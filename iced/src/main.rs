//! The Harken library, in iced — on the desktop and in a browser.
//!
//! One of two clients, and the other is `mobile`. They run the same
//! `apply`: this one links it, the phone loads it as a module. Neither of them
//! contains a line of domain logic.
//!
//!   terminal 1:  nix run .#serve
//!   terminal 2:  nix run .#iced alice     # this, on the desktop
//!   terminal 3:  nix run .#iced bob       # …and again, as someone else
//!   browser:     nix run .#web            # this, at http://localhost:8080
//!   a phone:     the Expo app, same server
//!
//! All of them are peers of one server, so a song favourited in the browser
//! hearts itself on the phone. Press the offline button in any of them, mutate
//! on both sides, come back online, and watch the rebase: your favourite lands
//! after whatever arrived while you were away, because "add to favourites"
//! reads the end of the playlist rather than naming a position.
//!
//! Who you are is what the server says. Signing in is `petros_auth`'s flow —
//! open one URL, get one code back — and looks different on the two targets
//! only in where the code comes back to: a loopback port the desktop listens
//! on, or this page's own address. `nix run .#iced alice` never opens a
//! browser because `nix run .#serve` is a dev server, which hands out a login
//! for a name; against a real one the browser opens and the name is ignored.
//!
//! The engine does not know which of these it is running in. What differs is
//! two lines: where the database lives, and which transport carries the bytes.

mod heart;

use std::time::Duration;

use harken::{self as mutators, HarkenApp, Item};
use heart::Heart;
use iced::widget::{button, canvas, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task};
use petros::{AutoCtx, Changes, Client};

/// A library item's id. The message carries what it identifies, so a playlist's
/// id cannot be dropped into one of these by mistake.
type Id = harken::Id<harken::tables::Media>;
use petros_auth::Login;

#[cfg(target_arch = "wasm32")]
use petros::transport::web::Link;
#[cfg(not(target_arch = "wasm32"))]
use petros::transport::ws::Link;

#[cfg(not(feature = "demo"))]
const DEFAULT_SERVER: &str = "http://127.0.0.1:8787";

/// Where the server is, and a name to offer a dev server. On the desktop,
/// flags; in a browser, the query string, with the server defaulting to
/// wherever this page came from — which is the server, when it serves it.
#[cfg(not(feature = "demo"))]
fn config() -> (String, Option<String>) {
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
            flag("--server").unwrap_or_else(|| DEFAULT_SERVER.into()),
            flag("--user"),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let location = web_sys::window().map(|w| w.location());
        let query = location
            .as_ref()
            .and_then(|l| l.search().ok())
            .unwrap_or_default();
        let origin = location
            .and_then(|l| l.origin().ok())
            .unwrap_or_else(|| DEFAULT_SERVER.into());
        (
            petros_auth::query_value(&query, "server").unwrap_or(origin),
            petros_auth::query_value(&query, "user"),
        )
    }
}

/// Where the database lives is the only storage difference between the targets.
#[cfg(not(target_arch = "wasm32"))]
fn open(user: &str) -> petros::Result<petros::Connection> {
    let safe: String = user
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    petros::open_path(std::env::temp_dir().join(format!("petros-demo-{safe}.db")))
}

/// In the browser, in memory: `sqlite-wasm-rs` registers a memory VFS by
/// default. It can also persist to OPFS, which needs an async handshake before
/// the first query — worth doing, and not what this example is for.
#[cfg(target_arch = "wasm32")]
fn open(_user: &str) -> petros::Result<petros::Connection> {
    petros::open_memory()
}

// ------------------------------------------------------------- remembering

/// The logins this machine has, one per server, in a JSON file under the
/// config directory. The token is a secret and the file is the user's own.
#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(feature = "demo", allow(dead_code))]
mod remembered {
    use super::Login;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn file() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("harken").join("logins.json"))
    }

    fn all() -> BTreeMap<String, Login> {
        file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write(all: &BTreeMap<String, Login>) {
        let Some(file) = file() else { return };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(all) {
            let _ = std::fs::write(file, json);
        }
    }

    pub fn recall(server: &str) -> Option<Login> {
        all().remove(server)
    }

    pub fn remember(server: &str, login: &Login) {
        let mut all = all();
        all.insert(server.to_string(), login.clone());
        write(&all);
    }

    pub fn forget(server: &str) {
        let mut all = all();
        all.remove(server);
        write(&all);
    }
}

/// The same, in the browser's storage.
#[cfg(target_arch = "wasm32")]
#[cfg_attr(feature = "demo", allow(dead_code))]
mod remembered {
    use super::Login;

    fn key(server: &str) -> String {
        format!("harken.login.{server}")
    }

    pub fn recall(server: &str) -> Option<Login> {
        let json = petros_auth::web::storage()?.get_item(&key(server)).ok()??;
        serde_json::from_str(&json).ok()
    }

    pub fn remember(server: &str, login: &Login) {
        if let (Some(storage), Ok(json)) =
            (petros_auth::web::storage(), serde_json::to_string(login))
        {
            let _ = storage.set_item(&key(server), &json);
        }
    }

    pub fn forget(server: &str) {
        if let Some(storage) = petros_auth::web::storage() {
            let _ = storage.remove_item(&key(server));
        }
    }
}

// ------------------------------------------------------------------ the app

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
    SignIn,
    /// The sign-in came back, one way or the other.
    SignedIn(Result<Login, String>),
    SignOut,
    /// Pump the transport. Nothing else drives a sans-io client.
    Tick,
}

/// A database, a socket, and what is maintained over them. Exists once
/// somebody is signed in, because the database is theirs.
struct Peer {
    client: Client<HarkenApp>,
    link: Option<Link<harken::Payload>>,
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
    items: Vec<Item>,
    /// The playlist's size, maintained. The status line showed it by counting
    /// the list on every frame — twenty times a second, over every song.
    /// How many items are on the playlist the hearts stand for.
    on_playlist: harken::PlaylistCount,
    /// Which playlist a heart means. The domain has no favourites of its own.
    playlist: harken::Id<harken::tables::Playlist>,
    pending: usize,
}

struct App {
    server: String,
    /// A name to offer a dev server, so `nix run .#iced alice` needs no
    /// browser. Ignored by a real one.
    user: Option<String>,
    login: Option<Login>,
    /// A sign-in is in flight: a browser is open, or a code is being traded.
    signing_in: bool,
    peer: Option<Peer>,
    title: String,
    artist: String,
    note: String,
}

impl Peer {
    fn open(login: &Login) -> Peer {
        let mut client = Client::<HarkenApp>::open(
            open(&login.user.id).expect("open the database"),
            login.user.id.clone(),
            AutoCtx::system(),
        )
        .expect("open the petros client");
        client.set_session(Some(login.session.clone()));
        client.set_token(Some(login.token.clone()));

        // A heart means "on this playlist", so there has to be one. The first
        // run of a peer makes it; after that it is whichever came back first,
        // which is stable because playlists are ordered by when they were made.
        let playlist = {
            let existing = harken::playlists(&mut client.store()).unwrap_or_default();
            match existing.first() {
                Some(p) => p.id,
                None => {
                    let _ = client.mutate(mutators::create_playlist("Favourites".into()));
                    harken::playlists(&mut client.store())
                        .unwrap_or_default()
                        .first()
                        .map(|p| p.id)
                        .unwrap_or_default()
                }
            }
        };

        let mut peer = Peer {
            client,
            link: None,
            library: harken::library_view(playlist),
            on_playlist: harken::playlist_count(playlist),
            playlist,
            items: Vec::new(),
            pending: 0,
        };
        // The one full read of the list. Everything after this is maintained.
        {
            let mut store = peer.client.store();
            peer.library.hydrate(&mut store);
            peer.on_playlist.hydrate(&mut store);
        }
        peer.items = harken::items_of(&peer.library);
        let _ = peer.client.take_changes();
        peer.pending = peer.client.pending_len();
        peer
    }

    fn connect(&mut self, server: &str) -> String {
        match Link::connect(&petros_auth::socket_url(server)) {
            Ok(link) => {
                let _ = self.client.connected();
                self.link = Some(link);
                format!("connected to {server}")
            }
            Err(e) => {
                self.link = None;
                format!("cannot reach {server} ({e}) — working offline")
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
                let patches = {
                    let mut store = self.client.store();
                    self.on_playlist.apply(&mut store, &changes);
                    self.library.apply(&mut store, &changes)
                };
                // Splice rather than rebuild: the query is maintained, and so
                // is the decoded list. A tap costs the rows that moved.
                harken::patch(&mut self.items, &patches);
            }
            Changes::Rebuilt => {
                {
                    let mut store = self.client.store();
                    self.library.hydrate(&mut store);
                    self.on_playlist.hydrate(&mut store);
                }
                self.items = harken::items_of(&self.library);
            }
        }
        self.pending = self.client.pending_len();
    }

    /// Move messages between the client and the wire. While offline the outbox
    /// is drained and dropped: reconnecting re-offers everything still pending,
    /// and the server dedupes what it has already seen.
    ///
    /// Reports whether anything arrived, and what is worth saying about it —
    /// so that the twenty ticks a second that find an empty socket cost a
    /// `try_recv` rather than a re-query of the whole list.
    fn pump(&mut self) -> (bool, Option<String>, Option<String>) {
        let mut moved = false;
        let mut note = None;
        let mut denied = None;
        for msg in self.client.take_outgoing() {
            if let Some(link) = &self.link {
                link.send(msg);
            }
        }
        if let Some(link) = &self.link {
            while let Some(msg) = link.try_recv() {
                moved = true;
                if let Err(e) = self.client.recv(msg) {
                    note = Some(e.to_string());
                }
            }
            if let Some(reason) = self.client.take_denial() {
                // The server's last word on this token; the socket is gone
                // behind it. Not a dropped link, and not worth retrying with
                // the same token.
                denied = Some(reason);
                self.link = None;
            } else if !link.is_alive() {
                note = Some("the link dropped".into());
                self.link = None;
            }
        }
        for r in self.client.take_rejections() {
            note = Some(format!("the server refused a change: {}", r.reason));
            moved = true;
        }
        (moved, note, denied)
    }
}

impl App {
    /// The demo's whole identity: nobody, signed in nowhere.
    ///
    /// A `Login` is what `Peer::open` wants, and the demo has no server to get
    /// one from — so it makes one up, with an empty token and session. Nothing
    /// ever sends them, because the demo never opens a socket.
    #[cfg(feature = "demo")]
    fn demo_login() -> Login {
        Login {
            token: String::new(),
            session: String::new(),
            user: petros_auth::Account {
                id: "demo".into(),
                name: "Demo".into(),
                email: String::new(),
            },
            expires_ms: 0,
        }
    }

    /// Put something in an empty demo library, so the page has a list on it.
    ///
    /// Through `mutate`, not through SQL: the demo runs the same `apply` as
    /// every other peer, and seeding it any other way would be showing
    /// something the engine did not do.
    #[cfg(feature = "demo")]
    fn seed(peer: &mut Peer) {
        if !peer.items.is_empty() {
            return;
        }
        const LIBRARY: &[(&str, &str, &str, i64)] = &[
            ("Glue", "Bicep", "Bicep", 272_000),
            ("Opal", "Bicep", "Bicep", 318_000),
            ("Aura", "Bicep", "Isles", 289_000),
            ("Gosh", "Jamie xx", "In Colour", 296_000),
            ("Loud Places", "Jamie xx", "In Colour", 397_000),
            ("Nightmarket", "Four Tet", "Sixteen Oceans", 256_000),
            ("Baby", "Four Tet", "Sixteen Oceans", 191_000),
            ("Teardrop", "Massive Attack", "Mezzanine", 330_000),
        ];
        for (title, artist, album, ms) in LIBRARY {
            let _ = peer.client.mutate(mutators::add_song(
                (*title).into(),
                (*artist).into(),
                (*album).into(),
                *ms,
                String::new(),
            ));
        }
        peer.refresh();
        // A few of them hearted, so the playlist is not empty either.
        let hearted: Vec<harken::Id<harken::tables::Media>> = peer
            .items
            .iter()
            .filter(|i| matches!(i.title.as_str(), "Glue" | "Gosh" | "Teardrop"))
            .map(|i| i.id)
            .collect();
        for id in hearted {
            let _ = peer
                .client
                .mutate(mutators::add_to_playlist(peer.playlist, id));
        }
        peer.refresh();
    }

    fn boot() -> (Self, Task<Message>) {
        // The demo signs nobody in and talks to nothing: it opens a peer, seeds
        // it, and that is the whole application. Everything below about tokens
        // and browsers is compiled out.
        #[cfg(feature = "demo")]
        {
            let login = Self::demo_login();
            let mut peer = Peer::open(&login);
            Self::seed(&mut peer);
            let app = App {
                login: Some(login),
                server: String::new(),
                user: None,
                signing_in: false,
                peer: Some(peer),
                title: String::new(),
                artist: String::new(),
                note: "a demo — nothing here leaves your browser".into(),
            };
            (app, Task::none())
        }

        #[cfg(not(feature = "demo"))]
        {
            let (server, user) = config();
            let mut app = App {
                login: remembered::recall(&server),
                server,
                user,
                signing_in: false,
                peer: None,
                title: String::new(),
                artist: String::new(),
                note: String::new(),
            };
            if let Some(login) = &app.login {
                let mut peer = Peer::open(login);
                app.note = peer.connect(&app.server);
                app.peer = Some(peer);
                return (app, Task::none());
            }
            // Nobody yet. A page that just came back from signing in has the
            // code in its address; a desktop given a name can ask straight away.
            let task = app.sign_in();
            (app, task)
        }
    }

    /// Start a sign-in, if one can be started from here without a person.
    #[cfg(not(feature = "demo"))]
    fn sign_in(&mut self) -> Task<Message> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(code) = petros_auth::web::take_code() {
                self.signing_in = true;
                self.note = "signing in…".into();
                let server = self.server.clone();
                return Task::perform(
                    async move { petros_auth::web::exchange(&server, &code).await },
                    Message::SignedIn,
                );
            }
            Task::none()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.user.is_none() {
                return Task::none();
            }
            self.start_sign_in()
        }
    }

    /// Send the person to sign in.
    fn start_sign_in(&mut self) -> Task<Message> {
        self.signing_in = true;
        #[cfg(target_arch = "wasm32")]
        {
            // The page goes away and comes back with a code; `boot` finishes.
            petros_auth::web::go_sign_in(&self.server, self.user.as_deref());
            Task::none()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Blocking for as long as a person takes, so on a thread of its
            // own, with the answer handed back as a message.
            self.note = "signing in — look for a browser tab if one opened".into();
            let server = self.server.clone();
            let user = self.user.clone();
            let (tx, rx) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let outcome = petros_auth::client::login(
                    &server,
                    user.as_deref(),
                    petros_auth::client::open_browser,
                );
                let _ = tx.send(outcome);
            });
            Task::perform(
                async move {
                    rx.await
                        .unwrap_or_else(|_| Err("the sign-in thread went away".into()))
                },
                Message::SignedIn,
            )
        }
    }

    fn signed_in(&mut self, login: Login) {
        remembered::remember(&self.server, &login);
        // The same person again keeps their database and their pending
        // edits; someone else gets theirs.
        let same = self
            .login
            .as_ref()
            .is_some_and(|l| l.user.id == login.user.id);
        match (&mut self.peer, same) {
            (Some(peer), true) => {
                peer.client.set_session(Some(login.session.clone()));
                peer.client.set_token(Some(login.token.clone()));
                peer.link = None;
                self.note = peer.connect(&self.server);
            }
            _ => {
                let mut peer = Peer::open(&login);
                self.note = peer.connect(&self.server);
                self.peer = Some(peer);
            }
        }
        self.login = Some(login);
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Typing, ticking and pulling the plug all leave the list alone.
        let edited = matches!(
            message,
            Message::AddSong
                | Message::ToggleFavorite(..)
                | Message::FavoriteAll
                | Message::RemoveSong(_)
        );
        let outcome: Result<(), String> = match message {
            Message::TypedTitle(text) => {
                self.title = text;
                Ok(())
            }
            Message::TypedArtist(text) => {
                self.artist = text;
                Ok(())
            }
            Message::SignIn => return self.start_sign_in(),
            Message::SignedIn(outcome) => {
                self.signing_in = false;
                match outcome {
                    Ok(login) => self.signed_in(login),
                    Err(e) => self.note = format!("could not sign in: {e}"),
                }
                Ok(())
            }
            Message::SignOut => {
                if let Some(login) = self.login.take() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let server = self.server.clone();
                        std::thread::spawn(move || {
                            let _ = petros_auth::client::logout(&server, &login.token);
                        });
                    }
                    #[cfg(target_arch = "wasm32")]
                    let _ = login;
                }
                remembered::forget(&self.server);
                self.peer = None;
                self.note = "signed out".into();
                Ok(())
            }
            Message::Tick => Ok(()),
            other => match &mut self.peer {
                None => Ok(()),
                Some(peer) => match other {
                    Message::AddSong => {
                        let title = std::mem::take(&mut self.title);
                        let artist = std::mem::take(&mut self.artist);
                        peer.client
                            .mutate(mutators::add_song(
                                title,
                                artist,
                                String::new(),
                                0,
                                String::new(),
                            ))
                            .map(|_| ())
                    }
                    Message::ToggleFavorite(id, favorited) => {
                        let m = if favorited {
                            mutators::remove_from_playlist(peer.playlist, id)
                        } else {
                            mutators::add_to_playlist(peer.playlist, id)
                        };
                        peer.client.mutate(m).map(|_| ())
                    }
                    Message::FavoriteAll => peer
                        .client
                        .mutate(mutators::add_all_to_playlist(peer.playlist))
                        .map(|_| ()),
                    Message::RemoveSong(id) => {
                        peer.client.mutate(mutators::remove_media(id)).map(|_| ())
                    }
                    Message::ToggleLink => {
                        match peer.link {
                            Some(_) => {
                                peer.link = None;
                                self.note = "gone offline — edits pile up locally".into();
                            }
                            None => self.note = peer.connect(&self.server),
                        }
                        Ok(())
                    }
                    _ => Ok(()),
                },
            }
            .map_err(|e: petros::Error| e.to_string()),
        };
        // A mutation the app itself refuses never reaches the pending queue.
        if let Err(e) = outcome {
            self.note = e;
        }
        // `refresh` reads the rows that moved, so it waits for a reason:
        // either this message was an edit, or the wire brought one.
        if let Some(peer) = &mut self.peer {
            let (arrived, note, denied) = peer.pump();
            if let Some(note) = note {
                self.note = note;
            }
            if let Some(reason) = denied {
                // Signed out from the other end — an expired token, a revoked
                // session. The database and the pending edits stay; the next
                // sign-in as the same person offers them.
                self.note = format!("signed out by the server: {reason}");
                remembered::forget(&self.server);
                if let Some(login) = &mut self.login {
                    login.token.clear();
                }
            }
            if edited || arrived {
                peer.refresh();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let Some(peer) = &self.peer else {
            return self.view_signed_out();
        };
        let rows = peer.items.iter().fold(column![].spacing(6), |col, item| {
            let favorited = item.on_playlist();
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
                    .on_press(Message::ToggleFavorite(item.id, favorited)),
                    column![
                        text(item.title.clone()),
                        text(item.creator.clone()).size(12).style(text::secondary),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    // Where it sits in the playlist, which is the number that
                    // moves when someone else favourites something first.
                    text(match item.playlist_pos {
                        Some(pos) => format!("#{pos}"),
                        None => String::new(),
                    })
                    .size(12)
                    .style(text::secondary),
                    text(item.user_id.clone()).size(12).style(text::secondary),
                    button("remove")
                        .style(button::text)
                        .on_press(Message::RemoveSong(item.id)),
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

        // Turned away by the server: the one button that helps is sign in.
        let signed_out = self.login.as_ref().is_some_and(|l| l.token.is_empty());
        let actions = row![
            button("favourite everything").on_press(Message::FavoriteAll),
            if signed_out {
                button("sign in again")
                    .on_press_maybe((!self.signing_in).then_some(Message::SignIn))
            } else {
                button(if peer.link.is_some() {
                    "go offline"
                } else {
                    "go online"
                })
                .on_press(Message::ToggleLink)
            },
            button("sign out")
                .style(button::text)
                .on_press(Message::SignOut),
        ]
        .spacing(12);

        // The engine showing through: `cursor` is how much of the server's log
        // has been applied, `pending` is what this peer has done that no server
        // has confirmed yet.
        let favorites = peer.on_playlist.get();
        let who = self
            .login
            .as_ref()
            .map(|l| {
                if l.user.name.is_empty() {
                    l.user.id.clone()
                } else {
                    l.user.name.clone()
                }
            })
            .unwrap_or_default();
        let status = text(format!(
            "{who} · {} · {} songs, {favorites} favourited · cursor {} · {} pending{}",
            if peer.link.is_some() {
                "online"
            } else {
                "offline"
            },
            peer.items.len(),
            peer.client.cursor(),
            peer.pending,
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

    fn view_signed_out(&self) -> Element<'_, Message> {
        let label = if self.signing_in {
            "signing in…"
        } else {
            "sign in"
        };
        container(
            column![
                text("harken").size(26),
                text(format!("a peer of {}", self.server)).size(13),
                button(label).on_press_maybe((!self.signing_in).then_some(Message::SignIn)),
                text(self.note.clone()).size(13).style(text::secondary),
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
