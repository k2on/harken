//! The Harken library, in iced — on the desktop and in a browser.
//!
//! One of two clients, and the other is `clients/expo`. They run the same
//! `apply`: this one links it, the phone loads it as a module. Neither of them
//! contains a line of domain logic.
//!
//!   terminal 1:  just serve
//!   terminal 2:  just iced alice     # this, on the desktop
//!   terminal 3:  just iced bob       # …and again, as someone else
//!   browser:     just web            # this, at http://localhost:8080
//!   a phone:     the Expo app, same server
//!
//! All of them are peers of one server, so a song favourited in the browser
//! hearts itself on the phone. Press the offline button in any of them, mutate
//! on both sides, come back online, and watch the rebase: your favourite lands
//! after whatever arrived while you were away, because "add to favourites"
//! reads the end of the playlist rather than naming a position.
//!
//! The engine does not know which of these it is running in. What differs is
//! two lines: where the database lives, and which transport carries the bytes.

mod heart;

use std::time::Duration;

use harken::{self as mutators, HarkenApp, Song};
use heart::Heart;
use iced::widget::{button, canvas, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task};
use petros::{AutoCtx, Changes, Client, Id};

#[cfg(target_arch = "wasm32")]
use petros::transport::web::Link;
#[cfg(not(target_arch = "wasm32"))]
use petros::transport::ws::Link;

const DEFAULT_SERVER: &str = "127.0.0.1:8787";

/// Who we are and where the server is. On the desktop, flags; in a browser, the
/// query string, so two tabs can be two peers.
fn config() -> (String, String) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        let flag = |name: &str| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        (
            flag("--user").unwrap_or_else(|| "iced".into()),
            flag("--server").unwrap_or_else(|| DEFAULT_SERVER.into()),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let query = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default();
        let param = |name: &str| {
            query
                .trim_start_matches('?')
                .split('&')
                .find_map(|kv| kv.strip_prefix(&format!("{name}="))?.into())
                .map(|v: &str| v.to_string())
        };
        (
            param("user").unwrap_or_else(|| "browser".into()),
            param("server").unwrap_or_else(|| DEFAULT_SERVER.into()),
        )
    }
}

/// Where the database lives is the only storage difference between the targets.
#[cfg(not(target_arch = "wasm32"))]
fn open(user: &str) -> petros::Result<petros::Connection> {
    petros::open_path(std::env::temp_dir().join(format!("petros-demo-{user}.db")))
}

/// In the browser, in memory: `sqlite-wasm-rs` registers a memory VFS by
/// default. It can also persist to OPFS, which needs an async handshake before
/// the first query — worth doing, and not what this example is for.
#[cfg(target_arch = "wasm32")]
fn open(_user: &str) -> petros::Result<petros::Connection> {
    petros::open_memory()
}

#[derive(Debug, Clone)]
enum Message {
    TypedTitle(String),
    TypedArtist(String),
    AddSong,
    /// The heart: on the playlist, or off it.
    ToggleFavorite(Id, bool),
    FavoriteAll,
    RemoveSong(Id),
    ToggleLink,
    /// Pump the transport. Nothing else drives a sans-io client.
    Tick,
}

struct App {
    client: Client<HarkenApp>,
    link: Option<Link<harken::Payload>>,
    server: String,
    user: String,
    /// The library, maintained rather than re-read.
    ///
    /// It is hydrated once and then told what each mutation changed, so a tap
    /// costs the rows that moved instead of the whole list. `Changes::Rebuilt`
    /// is the exception and is not a special case so much as an honest one: a
    /// rebase rolls the optimistic view back, and no sequence of changes
    /// describes that.
    library: harken::LibraryView,
    /// What `view` draws. iced's `view` takes `&self` and decoding needs
    /// nothing mutable, but doing it once per change beats once per frame.
    songs: Vec<Song>,
    pending: usize,
    title: String,
    artist: String,
    note: String,
}

impl App {
    fn boot() -> Self {
        let (user, server) = config();
        let client = Client::<HarkenApp>::open(
            open(&user).expect("open the database"),
            user.clone(),
            AutoCtx::system(),
        )
        .expect("open the petros client");
        let mut app = App {
            client,
            link: None,
            server,
            user,
            library: harken::library_view(),
            songs: Vec::new(),
            pending: 0,
            title: String::new(),
            artist: String::new(),
            note: String::new(),
        };
        app.connect();
        // The one full read of the list. Everything after this is maintained.
        {
            let mut store = app.client.store();
            app.library.hydrate(&mut store);
        }
        app.songs = harken::songs_of(&app.library);
        let _ = app.client.take_changes();
        app.pending = app.client.pending_len();
        app
    }

    fn connect(&mut self) {
        match Link::connect(&format!("ws://{}", self.server)) {
            Ok(link) => {
                let _ = self.client.connected();
                self.link = Some(link);
                self.note = format!("connected to {}", self.server);
            }
            Err(e) => {
                self.link = None;
                self.note = format!("cannot reach {} ({e}) — working offline", self.server);
            }
        }
    }

    /// Bring the view up to date with whatever just happened.
    ///
    /// The whole list is never read here. `Changes::Applied` is the rows that
    /// moved — usually one — and `Changes::Rebuilt` is the rebase, which costs
    /// one query and happens only when the server speaks while something of
    /// ours is still pending.
    fn refresh(&mut self) {
        match self.client.take_changes() {
            Changes::Applied(changes) if changes.is_empty() => {}
            Changes::Applied(changes) => {
                let mut store = self.client.store();
                self.library.apply(&mut store, &changes);
                self.songs = harken::songs_of(&self.library);
            }
            Changes::Rebuilt => {
                let mut store = self.client.store();
                self.library.hydrate(&mut store);
                self.songs = harken::songs_of(&self.library);
            }
        }
        self.pending = self.client.pending_len();
    }

    /// Move messages between the client and the wire. While offline the outbox
    /// is drained and dropped: reconnecting re-offers everything still pending,
    /// and the server dedupes what it has already seen.
    ///
    /// Reports whether anything arrived, so that the twenty ticks a second that
    /// find an empty socket cost a `try_recv` rather than a re-query of the
    /// whole list.
    fn pump(&mut self) -> bool {
        let mut moved = false;
        for msg in self.client.take_outgoing() {
            if let Some(link) = &self.link {
                link.send(msg);
            }
        }
        if let Some(link) = &self.link {
            while let Some(msg) = link.try_recv() {
                moved = true;
                if let Err(e) = self.client.recv(msg) {
                    self.note = e.to_string();
                }
            }
            if !link.is_alive() {
                self.note = "the link dropped".into();
                self.link = None;
            }
        }
        for r in self.client.take_rejections() {
            self.note = format!("the server refused a change: {}", r.reason);
            moved = true;
        }
        moved
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Typing, ticking and pulling the plug all leave the list alone.
        let edited = !matches!(
            message,
            Message::TypedTitle(_) | Message::TypedArtist(_) | Message::ToggleLink | Message::Tick
        );
        let outcome = match message {
            Message::TypedTitle(text) => {
                self.title = text;
                Ok(())
            }
            Message::TypedArtist(text) => {
                self.artist = text;
                Ok(())
            }
            Message::AddSong => {
                let title = std::mem::take(&mut self.title);
                let artist = std::mem::take(&mut self.artist);
                self.client
                    .mutate(mutators::add_song(title, artist))
                    .map(|_| ())
            }
            Message::ToggleFavorite(id, favorited) => {
                let bytes = *id.as_uuid().as_bytes();
                let m = if favorited {
                    mutators::unfavorite(bytes.to_vec())
                } else {
                    mutators::favorite(bytes.to_vec())
                };
                self.client.mutate(m).map(|_| ())
            }
            Message::FavoriteAll => self.client.mutate(mutators::favorite_all()).map(|_| ()),
            Message::RemoveSong(id) => self
                .client
                .mutate(mutators::remove_song(id.as_uuid().as_bytes().to_vec()))
                .map(|_| ()),
            Message::ToggleLink => {
                match self.link {
                    Some(_) => {
                        self.link = None;
                        self.note = "gone offline — edits pile up locally".into();
                    }
                    None => self.connect(),
                }
                Ok(())
            }
            Message::Tick => Ok(()),
        };
        // A mutation the app itself refuses never reaches the pending queue.
        if let Err(e) = outcome {
            self.note = e.to_string();
        }
        // `refresh` reads the whole list back out of SQLite, so it waits for a
        // reason: either this message was an edit, or the wire brought one.
        let arrived = self.pump();
        if edited || arrived {
            self.refresh();
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let rows = self.songs.iter().fold(column![].spacing(6), |col, song| {
            let favorited = song.favorited();
            col.push(
                row![
                    // The heart is a button wrapping a canvas rather than a
                    // label: the font has no heart in it, so a character would
                    // draw nothing. See `heart.rs`.
                    button(
                        canvas(Heart { filled: favorited })
                            .width(Heart::SIZE)
                            .height(Heart::SIZE)
                    )
                    .style(button::text)
                    .padding(4)
                    .on_press(Message::ToggleFavorite(song.id, favorited)),
                    column![
                        text(song.title.clone()),
                        text(song.artist.clone()).size(12).style(text::secondary),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    // Where it sits in the playlist, which is the number that
                    // moves when someone else favourites something first.
                    text(match song.favorite_pos {
                        Some(pos) => format!("#{pos}"),
                        None => String::new(),
                    })
                    .size(12)
                    .style(text::secondary),
                    text(song.actor.clone()).size(12).style(text::secondary),
                    button("remove")
                        .style(button::text)
                        .on_press(Message::RemoveSong(song.id)),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            )
        });

        let entry = row![
            text_input("title…", &self.title)
                .on_input(Message::TypedTitle)
                .on_submit(Message::AddSong)
                .width(Length::Fill),
            text_input("artist…", &self.artist)
                .on_input(Message::TypedArtist)
                .on_submit(Message::AddSong)
                .width(Length::Fill),
            button("add").on_press(Message::AddSong),
        ]
        .spacing(12);

        let actions = row![
            button("favourite everything").on_press(Message::FavoriteAll),
            button(if self.link.is_some() {
                "go offline"
            } else {
                "go online"
            })
            .on_press(Message::ToggleLink),
        ]
        .spacing(12);

        // The engine showing through: `cursor` is how much of the server's log
        // has been applied, `pending` is what this peer has done that no server
        // has confirmed yet.
        let favorites = self.songs.iter().filter(|s| s.favorited()).count();
        let status = text(format!(
            "{} · {} · {} songs, {favorites} favourited · cursor {} · {} pending{}",
            self.user,
            if self.link.is_some() {
                "online"
            } else {
                "offline"
            },
            self.songs.len(),
            self.client.cursor(),
            self.pending,
            if self.note.is_empty() {
                String::new()
            } else {
                format!("  ·  {}", self.note)
            }
        ))
        .size(13);

        container(
            column![
                text("harken").size(26),
                entry,
                actions,
                scrollable(rows).height(Length::Fill),
                status,
            ]
            .spacing(16),
        )
        .padding(24)
        .into()
    }

    /// A sans-io client has to be pumped by someone. This is that someone.
    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick)
    }
}

pub fn main() -> iced::Result {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    iced::application(App::boot, App::update, App::view)
        .subscription(App::subscription)
        .title("harken")
        .window_size((860.0, 600.0))
        .run()
}
