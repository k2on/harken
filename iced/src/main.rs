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

mod icon;
mod player;
/// The demo's library. Compiled only into the demo, so the client that
/// talks to a real server carries none of it.
#[cfg(feature = "demo")]
mod seed;
mod vim;

use std::time::Duration;

use harken::{self as mutators, HarkenApp, Item};
use iced::widget::{
    button, column, container, mouse_area, row, rule, scrollable, slider, text, Row,
};
use iced::{Element, Length, Subscription, Task};
use petros::{AutoCtx, Changes, Client};
use player::{Player, Track};

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

/// What the main list is showing, which is what the sidebar picks.
///
/// Each variant carries the name it was selected by rather than an id to look
/// up: the header draws it, and a library that changed underneath the
/// selection should not make the header go blank.
#[derive(Debug, Clone, PartialEq)]
enum Source {
    /// Everything, in library order. The only one served by the maintained
    /// view rather than by a query — see `Peer::reload_shown`.
    Library,
    Playlist(harken::Id<harken::tables::Playlist>, String),
    Album(String),
    Artist(String),
}

impl Source {
    /// The heading this source belongs under, or `None` for the one line that
    /// needs no heading. What makes the sidebar a flat list of choices with
    /// headings *derived* rather than interleaved — so a cursor can address
    /// line N without counting past decoration.
    fn heading(&self) -> Option<&'static str> {
        match self {
            Source::Library => None,
            Source::Playlist(..) => Some("Playlists"),
            Source::Album(_) => Some("Albums"),
            Source::Artist(_) => Some("Artists"),
        }
    }

    fn title(&self) -> &str {
        match self {
            Source::Library => "Library",
            Source::Playlist(_, name) | Source::Album(name) | Source::Artist(name) => name,
        }
    }
}

/// Which part of the window the cursor is in.
///
/// The panes and what leaving one means are the application's business, not
/// the grammar's: `vim` knows a motion was refused, and this decides that a
/// refused `l` in the sidebar means the track list.
///
/// The now-playing bar is deliberately not one of them. Everything it does has
/// a key of its own — `p`, `{`, `}` — so making it a third place the cursor
/// can be would only add a stop to `<Tab>` that nobody needs to pass through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Sidebar,
    Tracks,
}

impl Pane {
    /// Where a motion this pane refused should take the cursor, if anywhere.
    ///
    /// Only ever called with a motion the pane's own shape returned `None`
    /// for, which is what "not mine" was for.
    fn beyond(self, motion: vim::Motion) -> Option<Pane> {
        use vim::Motion::{Left, Right};
        match (self, motion) {
            (Pane::Sidebar, Right(_)) => Some(Pane::Tracks),
            (Pane::Tracks, Left(_)) => Some(Pane::Sidebar),
            _ => None,
        }
    }

    /// `<Tab>` order.
    fn next(self) -> Pane {
        match self {
            Pane::Sidebar => Pane::Tracks,
            Pane::Tracks => Pane::Sidebar,
        }
    }
}

/// Where a track's bytes are.
///
/// `file` is one of two things, and which one is not a mode: the demo's
/// recordings are whole URLs into Wikimedia, and a scanned track's is the path
/// the scanner wrote, relative to the media root — which is exactly what
/// `/media/` serves back. Joining it to the server *here* is what keeps that
/// one string one string: the log carries no machine's address, and a client
/// plays what the scanner wrote without either of them knowing where the
/// directory is.
///
/// Handing the relative one straight to an `<audio>` element is what this
/// exists to stop. The browser resolves it against the page, which drops the
/// `/media/` prefix, and a server with a single-page fallback answers the
/// wrong path with `index.html` and a 200 — so the element is handed HTML and
/// reports only that the resource is "not suitable".
fn media_url(server: &str, file: &str) -> String {
    if file.is_empty() || file.starts_with("http://") || file.starts_with("https://") {
        return file.to_string();
    }
    let mut url = format!("{}/media", server.trim_end_matches('/'));
    for part in file.split('/') {
        url.push('/');
        encode(part, &mut url);
    }
    url
}

/// Percent-encode one path segment.
///
/// Real libraries are full of spaces, ampersands and the occasional `#`, and a
/// `#` is the one that is silently destructive: everything after it is a
/// fragment, so the request goes out for a path that stops mid-filename and
/// the server answers 404. Everything outside RFC 3986's unreserved set is
/// escaped, which covers those and every non-ASCII byte.
fn encode(part: &str, out: &mut String) {
    for b in part.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
}

// The demo drives only a few of these: it has no accounts to sign in or out
// of. The variants stay so that the one `update` serves both builds.
#[cfg_attr(feature = "demo", allow(dead_code))]
#[derive(Debug, Clone)]
enum Message {
    /// The heart: on the playlist, or off it.
    ToggleFavorite(Id, bool),
    ToggleLink,
    /// The sidebar: show a playlist, an album, an artist, or everything.
    Select(Source),
    /// Start this one, and make what is on screen the queue it plays through.
    PlayItem(Id),
    PlayPause,
    Skip(i32),
    /// Dragging the bar's progress, in seconds.
    Seek(f32),
    SignIn,
    /// The sign-in came back, one way or the other.
    SignedIn(Result<Login, String>),
    SignOut,
    /// A key nothing on screen wanted. See `subscription`.
    Key(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// Pump the transport. Nothing else drives a sans-io client.
    Tick,
}

/// One line of the sidebar: somewhere the cursor can be and something it can
/// open.
struct Choice {
    source: Source,
    label: String,
    /// The tally drawn on the right, where there is one to draw.
    count: Option<i64>,
}

// The table's columns. Portions rather than pixels, so the three text columns
// share whatever width is left after the heart and the duration, and none of
// them can push the others off the edge at a narrow window.
const NAME: Length = Length::FillPortion(4);
const ARTIST: Length = Length::FillPortion(3);
const ALBUM: Length = Length::FillPortion(3);
const TIME: Length = Length::Fixed(56.0);

/// One cell of the table: a single line, clipped rather than wrapped.
///
/// Every colour is asked of the theme rather than written down, which is the
/// whole of what makes this work in dark mode — and on the cursor's own row,
/// which is painted in the accent colour and needs text chosen against *that*
/// rather than against the window.
fn cell<'a>(
    body: String,
    width: Length,
    on_cursor: bool,
    accent: bool,
    dim: bool,
) -> Element<'a, Message> {
    text(body)
        .size(13)
        .width(width)
        .wrapping(text::Wrapping::None)
        .style(move |theme: &iced::Theme| {
            let palette = theme.extended_palette();
            let color = if on_cursor {
                // The row is filled with the accent colour, so there is exactly
                // one colour text on it can be: the one that colour was paired
                // with. A dimmed column gets the same hue, not a grey.
                let text = palette.primary.base.text;
                if dim {
                    text.scale_alpha(0.75)
                } else {
                    text
                }
            } else if accent {
                palette.primary.base.color
            } else if dim {
                palette.background.base.text.scale_alpha(0.6)
            } else {
                palette.background.base.text
            };
            text::Style { color: Some(color) }
        })
        .into()
}

/// A column heading, in the same grid as the cells under it.
fn heading<'a>(label: &'a str, width: Length) -> Element<'a, Message> {
    text(label)
        .size(11)
        .width(width)
        .style(|theme: &iced::Theme| text::Style {
            color: Some(
                theme
                    .extended_palette()
                    .background
                    .base
                    .text
                    .scale_alpha(0.55),
            ),
        })
        .into()
}

/// What a table row is painted.
///
/// Three states, and they are deliberately not three shades of the same idea:
/// the cursor is the accent colour when its pane has the keyboard and a plain
/// strong grey when it does not — the way a native list dims its selection
/// when you click away — and everything else is the zebra, which is the
/// window's own background alternating with the faintest step up from it.
/// Nothing here is a literal colour, so a dark theme restates all of it.
fn row_style(theme: &iced::Theme, on_cursor: bool, focused: bool, odd: bool) -> container::Style {
    let palette = theme.extended_palette();
    let background = if on_cursor {
        Some(if focused {
            palette.primary.base.color
        } else {
            palette.background.strong.color
        })
    } else if odd {
        Some(palette.background.weak.color)
    } else {
        None
    };
    container::Style {
        background: background.map(iced::Background::Color),
        ..container::Style::default()
    }
}

/// Seconds as `m:ss`, which is how long a piece of music is written down.
fn clock(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return String::new();
    }
    let secs = secs as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
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
    ///
    /// Not the same thing as what the sidebar is *showing*: a heart always
    /// means favourites, so hearting something while browsing an album puts it
    /// where a heart has always put it.
    playlist: harken::Id<harken::tables::Playlist>,
    /// What the sidebar picked, and the list that answers it.
    ///
    /// `Source::Library` is `items` itself, maintained. Everything else is one
    /// query per change — the whole point of the maintained view is the list
    /// you are looking at most of the time, and re-reading a single album when
    /// something moves is a hundred rows, not the library.
    source: Source,
    shown: Vec<Item>,
    /// Which album each track is on.
    ///
    /// A map beside the list rather than a field on `Item`, because `album`
    /// belongs to the song kind and the library row is deliberately
    /// kind-neutral. Read when the library changes, like the sidebar, and
    /// joined in memory while drawing.
    albums: std::collections::HashMap<harken::Id<harken::tables::Media>, String>,
    /// The sidebar, rebuilt when the library changes rather than per frame.
    ///
    /// One flat list of the lines a cursor can sit on, in the order they are
    /// drawn. Headings are derived from it while drawing rather than stored in
    /// it, so "line 9" means the same thing to the keyboard and to the eye.
    choices: Vec<Choice>,
    pending: usize,
}

struct App {
    /// Which pane the cursor is in, and where it is in each of them.
    ///
    /// A cursor per pane rather than one shared one, so that leaving the
    /// sidebar and coming back does not lose your place — which is what a
    /// window manager does and what anyone who types `h` twice expects.
    pane: Pane,
    cursors: [usize; 2],
    /// Half-typed keys: a count, a `g`, a `/` search. See `vim.rs`.
    keys: vim::Keys,
    /// The last accepted `/` search, for `n` and `N`.
    search: String,
    /// `?` — a keymap nobody can guess is a keymap nobody uses.
    help: bool,
    /// What is playing, and whether this build can sound it. See `player.rs`.
    player: Player,
    /// What `Skip` moves through: the list as it stood when play was pressed.
    ///
    /// A snapshot rather than a reference to the shown list, so that changing
    /// the sidebar selection — or somebody else's edit arriving — does not
    /// silently redirect what plays next.
    queue: Vec<Item>,
    server: String,
    /// A name to offer a dev server, so `nix run .#iced alice` needs no
    /// browser. Ignored by a real one.
    user: Option<String>,
    login: Option<Login>,
    /// A sign-in is in flight: a browser is open, or a code is being traded.
    signing_in: bool,
    peer: Option<Peer>,
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
            source: Source::Library,
            shown: Vec::new(),
            albums: std::collections::HashMap::new(),
            choices: Vec::new(),
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
        peer.reload_sidebar();
        peer.reload_shown();
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
        self.reload_sidebar();
        self.reload_shown();
        self.pending = self.client.pending_len();
    }

    /// The sidebar's three lists, read back.
    ///
    /// Three queries, and they run when something changed rather than when
    /// something is drawn — `refresh` is only called for an edit or an arrival.
    /// Grouping happens in the domain, so both clients get the same lists in
    /// the same order rather than each inventing a way to fold the library.
    fn reload_sidebar(&mut self) {
        let mut store = self.client.store();
        let playlists = harken::playlists(&mut store).unwrap_or_default();
        let albums = harken::albums(&mut store).unwrap_or_default();
        let artists = harken::artists(&mut store).unwrap_or_default();
        drop(store);

        self.albums = harken::track_albums(&mut self.client.store())
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.media_id, t.album))
            .collect();

        let mut choices = vec![Choice {
            source: Source::Library,
            label: "Library".into(),
            count: Some(self.items.len() as i64),
        }];
        choices.extend(playlists.into_iter().map(|p| Choice {
            source: Source::Playlist(p.id, p.name.clone()),
            label: p.name,
            count: None,
        }));
        choices.extend(albums.into_iter().map(|a| Choice {
            source: Source::Album(a.name.clone()),
            label: a.name,
            count: Some(a.tracks),
        }));
        choices.extend(artists.into_iter().map(|a| Choice {
            source: Source::Artist(a.name.clone()),
            label: a.name,
            count: Some(a.tracks),
        }));
        self.choices = choices;
    }

    /// The album a track is on, or nothing if its kind has none.
    fn album_of(&self, id: harken::Id<harken::tables::Media>) -> String {
        self.albums.get(&id).cloned().unwrap_or_default()
    }

    /// The list under the header, for whatever the sidebar picked.
    ///
    /// The library is the maintained one and costs nothing here. The others are
    /// a query, because a filtered list is not what the view is maintaining —
    /// and a selection is a click, so paying for it there is the right place.
    fn reload_shown(&mut self) {
        self.shown = match self.source.clone() {
            Source::Library => return,
            Source::Playlist(id, _) => harken::playlist(&mut self.client.store(), id),
            Source::Album(name) => harken::album(&mut self.client.store(), self.playlist, name),
            Source::Artist(name) => harken::artist(&mut self.client.store(), self.playlist, name),
        }
        .unwrap_or_default();
    }

    /// What the main list is showing, whichever side it came from.
    fn rows(&self) -> &[Item] {
        match self.source {
            Source::Library => &self.items,
            _ => &self.shown,
        }
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

    fn boot() -> (Self, Task<Message>) {
        // The demo signs nobody in and talks to nothing: it opens a peer, seeds
        // it, and that is the whole application. Everything below about tokens
        // and browsers is compiled out.
        #[cfg(feature = "demo")]
        {
            let login = Self::demo_login();
            let mut peer = Peer::open(&login);
            seed::seed(&mut peer);
            let app = App {
                pane: Pane::Tracks,
                cursors: [0; 2],
                keys: vim::Keys::new(),
                search: String::new(),
                help: false,
                player: Player::new(),
                queue: Vec::new(),
                login: Some(login),
                server: String::new(),
                user: None,
                signing_in: false,
                peer: Some(peer),
                note: "a demo — nothing here leaves your browser".into(),
            };
            (app, Task::none())
        }

        #[cfg(not(feature = "demo"))]
        {
            let (server, user) = config();
            let mut app = App {
                pane: Pane::Tracks,
                cursors: [0; 2],
                keys: vim::Keys::new(),
                search: String::new(),
                help: false,
                player: Player::new(),
                queue: Vec::new(),
                login: remembered::recall(&server),
                server,
                user,
                signing_in: false,
                peer: None,
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

    // The two scrollables the cursor has to keep itself inside of.
    const SIDEBAR: &'static str = "sidebar";
    const TRACKS: &'static str = "tracks";

    /// Where the cursor is in a pane.
    fn at(&self, pane: Pane) -> usize {
        self.cursors[pane as usize]
    }

    /// What shape a pane is, which is all `vim` needs to know about it.
    ///
    /// The only place the application says "this one is a list and that one is
    /// a row". Everything else — counts, `gg`, whether `h` leaves the pane —
    /// falls out of the shape.
    fn shape(&self, pane: Pane) -> Box<dyn vim::Navigate> {
        let peer = self.peer.as_ref();
        match pane {
            Pane::Sidebar => Box::new(vim::List {
                cells: peer.map_or(0, |p| p.choices.len()),
            }),
            Pane::Tracks => Box::new(vim::List {
                cells: peer.map_or(0, |p| p.rows().len()),
            }),
        }
    }

    /// The text `/` searches, for whichever pane has the cursor.
    fn labels(&self, pane: Pane) -> Vec<String> {
        let Some(peer) = &self.peer else {
            return Vec::new();
        };
        match pane {
            Pane::Sidebar => peer.choices.iter().map(|c| c.label.clone()).collect(),
            // Both halves of the row, so `/bach` finds a Bach track whether the
            // word is in the title or the composer.
            Pane::Tracks => peer
                .rows()
                .iter()
                .map(|i| format!("{} {}", i.title, i.creator))
                .collect(),
        }
    }

    /// Do what a finished command asked for.
    ///
    /// The only place that knows both halves: `vim` produced the action from
    /// keys, and this is what the action means in a music library.
    fn act(&mut self, action: vim::Action) -> Task<Message> {
        match action {
            vim::Action::Move(motion) => self.travel(motion),
            vim::Action::Activate => self.activate(),
            vim::Action::Cycle => {
                self.pane = self.pane.next();
                self.reveal()
            }
            vim::Action::Toggle => {
                let at = self.at(self.pane);
                match self.pane {
                    // A heart, which is what a row's own toggle is here.
                    Pane::Tracks => {
                        let row = self
                            .peer
                            .as_ref()
                            .and_then(|p| p.rows().get(at))
                            .map(|i| (i.id, i.on_playlist()));
                        match row {
                            Some((id, on)) => self.update(Message::ToggleFavorite(id, on)),
                            None => Task::none(),
                        }
                    }
                    Pane::Sidebar => self.activate(),
                }
            }
            vim::Action::Search(query) => {
                self.search = query;
                // From one before the cursor, so that a search finds a match
                // on the line you are already on rather than skipping it.
                self.cursors[self.pane as usize] = self.at(self.pane).saturating_sub(1);
                self.seek(1)
            }
            vim::Action::Match(delta) => self.seek(delta),
            vim::Action::Cancel => {
                self.help = false;
                self.note.clear();
                Task::none()
            }
            // Keys this application binds for itself. Kept out of the grammar
            // so that a music player's conveniences cannot collide with a
            // motion by accident: anything unclaimed here simply does nothing.
            vim::Action::Key(c) => match c {
                '?' => {
                    self.help = !self.help;
                    Task::none()
                }
                'p' => self.update(Message::PlayPause),
                '}' => self.update(Message::Skip(1)),
                '{' => self.update(Message::Skip(-1)),
                _ => Task::none(),
            },
        }
    }

    /// Move the cursor, letting a refused motion change pane instead.
    ///
    /// This is the whole of what the application adds to `vim`: the grammar
    /// said which way, the shape said whether it could, and this decides that
    /// "it could not, and there was no axis for it" means the next pane along.
    fn travel(&mut self, motion: vim::Motion) -> Task<Message> {
        let pane = self.pane;
        match self.shape(pane).step(self.at(pane), motion) {
            Some(at) => return self.land(pane, at),
            None => match pane.beyond(motion) {
                Some(next) => self.pane = next,
                None => return Task::none(),
            },
        }
        self.reveal()
    }

    /// Put the cursor down somewhere, and do everything that follows from it.
    ///
    /// The one way to move a cursor, because there were two — a motion and a
    /// search — and only the motion remembered that landing in the sidebar
    /// also means showing what you landed on. `/bach<Enter>` moved the
    /// highlight and left the table on whatever was there before.
    fn land(&mut self, pane: Pane, at: usize) -> Task<Message> {
        self.cursors[pane as usize] = at;
        // In the sidebar the cursor *is* the selection: what it is on is what
        // the table shows, with no key in between.
        if pane == Pane::Sidebar {
            self.show_under_cursor();
        }
        self.reveal()
    }

    /// Point the table at whatever the sidebar's cursor is on.
    fn show_under_cursor(&mut self) {
        let at = self.at(Pane::Sidebar);
        let Some(source) = self
            .peer
            .as_ref()
            .and_then(|p| p.choices.get(at))
            .map(|c| c.source.clone())
        else {
            return;
        };
        if let Some(peer) = &mut self.peer {
            if peer.source == source {
                return;
            }
            peer.source = source;
            peer.reload_shown();
            // A different list is a different row one, so the cursor goes back
            // to the top rather than to wherever it happened to be.
            self.cursors[Pane::Tracks as usize] = 0;
        }
    }

    /// Keep the cursor on screen.
    ///
    /// A relative offset rather than a measured one: rows here are a uniform
    /// height, so cell `n` of `len` is `n / (len - 1)` down the scrollable, and
    /// that is close enough to keep it in view without the widget having to
    /// report its own geometry back.
    fn reveal(&self) -> Task<Message> {
        let (id, len) = match self.pane {
            Pane::Sidebar => (Self::SIDEBAR, self.shape(Pane::Sidebar).cells()),
            Pane::Tracks => (Self::TRACKS, self.shape(Pane::Tracks).cells()),
        };
        if len < 2 {
            return Task::none();
        }
        let y = self.at(self.pane) as f32 / (len - 1) as f32;
        // `advanced` is on for exactly this: keeping a keyboard cursor inside
        // its scrollable is a widget operation, and there is no other way to
        // ask a scrollable to move.
        iced::advanced::widget::operate(iced::advanced::widget::operation::scrollable::snap_to(
            iced::advanced::widget::Id::new(id),
            scrollable::RelativeOffset {
                x: Some(0.0),
                y: Some(y.clamp(0.0, 1.0)),
            },
        ))
    }

    /// Jump to the first label matching the last search, `delta` matches on
    /// from where the cursor is. `n` and `N`, and the `/` that started it.
    fn seek(&mut self, delta: isize) -> Task<Message> {
        if self.search.is_empty() {
            return Task::none();
        }
        let pane = self.pane;
        let needle = self.search.to_lowercase();
        let labels = self.labels(pane);
        let hits: Vec<usize> = labels
            .iter()
            .enumerate()
            .filter(|(_, l)| l.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect();
        if hits.is_empty() {
            self.note = format!("no match for {}", self.search);
            return Task::none();
        }
        let at = self.at(pane);
        // Wrap, the way vim does, and say so rather than stopping silently.
        let next = match delta {
            d if d >= 0 => hits.iter().find(|&&h| h > at).copied(),
            _ => hits.iter().rev().find(|&&h| h < at).copied(),
        };
        let next = next.unwrap_or_else(|| {
            if delta >= 0 {
                hits[0]
            } else {
                hits[hits.len() - 1]
            }
        });
        self.land(pane, next)
    }

    /// Open what the cursor is on.
    fn activate(&mut self) -> Task<Message> {
        let at = self.at(self.pane);
        match self.pane {
            // The cursor already chose it on the way past, so opening means
            // "and now I want to be in it".
            Pane::Sidebar => {
                self.show_under_cursor();
                self.pane = Pane::Tracks;
                self.reveal()
            }
            Pane::Tracks => {
                let id = self
                    .peer
                    .as_ref()
                    .and_then(|p| p.rows().get(at))
                    .map(|i| i.id);
                if let Some(id) = id {
                    return self.update(Message::PlayItem(id));
                }
                Task::none()
            }
        }
    }

    /// Move through the queue, and stop at either end rather than wrapping —
    /// a list that loops silently is hard to tell from one that is stuck.
    fn skip(&mut self, delta: i32) {
        let Some(current) = self.player.track().map(|t| t.id) else {
            return;
        };
        let Some(at) = self.queue.iter().position(|i| i.id == current) else {
            return;
        };
        let next = at as i32 + delta;
        if next < 0 || next as usize >= self.queue.len() {
            return;
        }
        let item = self.queue[next as usize].clone();
        self.player.play(
            Track {
                id: item.id,
                title: item.title.clone(),
                creator: item.creator.clone(),
                ms: item.duration_ms,
            },
            &media_url(&self.server, &item.file),
        );
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Typing, ticking and pulling the plug all leave the list alone.
        let edited = matches!(message, Message::ToggleFavorite(..));
        let outcome: Result<(), String> = match message {
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
            Message::Key(key, mods) => {
                let Some(action) = self.keys.press(&key, mods) else {
                    // Still typing: a count, a `g`, a search. Drawn in the
                    // status line so it is never a mystery what was swallowed.
                    return Task::none();
                };
                return self.act(action);
            }
            Message::PlayPause => {
                self.player.toggle();
                Ok(())
            }
            Message::Seek(secs) => {
                self.player.seek(secs as f64);
                Ok(())
            }
            Message::Skip(delta) => {
                self.skip(delta);
                Ok(())
            }
            // The element runs on its own clock, so the end of a track arrives
            // as "the tick that noticed" rather than as an event. Twenty times
            // a second is plenty to move on by.
            Message::Tick => {
                if self.player.ended() {
                    self.skip(1);
                }
                Ok(())
            }
            other => match &mut self.peer {
                None => Ok(()),
                Some(peer) => match other {
                    // A click has to move the cursor as well, or the keyboard
                    // would carry on from wherever it was and the highlight
                    // would be somewhere the table is not.
                    Message::Select(source) => {
                        let at = peer.choices.iter().position(|c| c.source == source);
                        peer.source = source;
                        peer.reload_shown();
                        if let Some(at) = at {
                            self.cursors[Pane::Sidebar as usize] = at;
                            self.cursors[Pane::Tracks as usize] = 0;
                        }
                        Ok(())
                    }
                    Message::PlayItem(id) => {
                        // The queue is what is on screen, taken now: skipping
                        // follows the list you pressed play in, even after the
                        // sidebar moves somewhere else.
                        self.queue = peer.rows().to_vec();
                        if let Some(item) = self.queue.iter().find(|i| i.id == id) {
                            self.player.play(
                                Track {
                                    id: item.id,
                                    title: item.title.clone(),
                                    creator: item.creator.clone(),
                                    ms: item.duration_ms,
                                },
                                &media_url(&self.server, &item.file),
                            );
                            if item.file.is_empty() {
                                self.note = "nothing to stream — this one has no file".into();
                            }
                        }
                        Ok(())
                    }
                    Message::ToggleFavorite(id, favorited) => {
                        let m = if favorited {
                            mutators::remove_from_playlist(peer.playlist, id)
                        } else {
                            mutators::add_to_playlist(peer.playlist, id)
                        };
                        peer.client.mutate(m).map(|_| ())
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

        container(
            column![
                // The sidebar and the list share the height that is left once
                // the bar has taken its own — so the bar stays at the bottom
                // however long the list is, rather than being pushed off it.
                row![
                    self.view_sidebar(peer),
                    rule::vertical(1),
                    self.view_list(peer),
                ]
                .spacing(16)
                .height(Length::Fill),
                rule::horizontal(1),
                self.view_bar(),
            ]
            .spacing(12),
        )
        .padding(16)
        .into()
    }

    /// Playlists, then albums, then artists — each read back by the domain, so
    /// both clients would fold the library the same way.
    fn view_sidebar(&self, peer: &'_ Peer) -> Element<'_, Message> {
        let cursor = self.at(Pane::Sidebar);
        let focused = self.pane == Pane::Sidebar;
        let mut side = column![].spacing(0).padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 0.0,
        });

        let mut under: Option<&str> = None;
        for (i, choice) in peer.choices.iter().enumerate() {
            // The heading is emitted when the kind changes rather than stored
            // as a line, which is what keeps the cursor's idea of line N and
            // the reader's the same.
            let heading = choice.source.heading();
            if heading.is_some() && heading != under {
                side = side.push(
                    container(text(heading.unwrap_or_default()).size(10).style(
                        |theme: &iced::Theme| {
                            text::Style {
                                color: Some(
                                    theme
                                        .extended_palette()
                                        .background
                                        .base
                                        .text
                                        .scale_alpha(0.5),
                                ),
                            }
                        },
                    ))
                    .padding([8, 10]),
                );
            }
            under = heading;

            // The sidebar has one highlight, not two: its cursor *is* what the
            // table is showing, so a separate "selected" colour would be a
            // second name for the same row.
            let on_cursor = i == cursor;
            side = side.push(
                mouse_area(
                    container(
                        row![
                            text(choice.label.clone())
                                .size(13)
                                .width(Length::Fill)
                                .wrapping(text::Wrapping::None)
                                .style(move |theme: &iced::Theme| text::Style {
                                    color: Some(if on_cursor && focused {
                                        theme.extended_palette().primary.base.text
                                    } else {
                                        theme.extended_palette().background.base.text
                                    }),
                                }),
                            text(choice.count.map(|n| n.to_string()).unwrap_or_default())
                                .size(10)
                                .style(move |theme: &iced::Theme| {
                                    let palette = theme.extended_palette();
                                    text::Style {
                                        color: Some(if on_cursor && focused {
                                            palette.primary.base.text.scale_alpha(0.7)
                                        } else {
                                            palette.background.base.text.scale_alpha(0.5)
                                        }),
                                    }
                                }),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center),
                    )
                    .width(Length::Fill)
                    .padding([4, 10])
                    .style(move |theme: &iced::Theme| row_style(theme, on_cursor, focused, false)),
                )
                .on_press(Message::Select(choice.source.clone())),
            );
        }

        container(scrollable(side).id(Self::SIDEBAR))
            .width(Length::Fixed(200.0))
            .height(Length::Fill)
            .into()
    }

    /// What the sidebar picked.
    ///
    /// The demo is a listening UI and nothing else: no sign-in (it has no
    /// accounts and no server), no typing a song in, no bulk favouriting and
    /// no per-row remove. The real client keeps all four, because against a
    /// server they are the only way to sign in, add anything, or take it back
    /// out again — so they are compiled out here rather than deleted.
    fn view_list(&self, peer: &'_ Peer) -> Element<'_, Message> {
        let playing = self.player.track().map(|t| t.id);
        // Only drawn while this pane has the keyboard. A dimmed cursor here
        // would sit one shade away from the zebra and mean something entirely
        // different from it, which is a worse thing to show than nothing: the
        // sidebar's highlight is a *selection* and has to persist, but this
        // one only ever means "where the next `j` goes".
        let focused = self.pane == Pane::Tracks;
        let cursor = focused.then_some(self.at(Pane::Tracks));

        let rows = peer
            .rows()
            .iter()
            .enumerate()
            .fold(column![].spacing(0), |col, (i, item)| {
                let on_cursor = cursor == Some(i);
                let album = peer.album_of(item.id);
                #[cfg_attr(feature = "demo", allow(unused_mut))]
                let line = Row::new()
                    .spacing(0)
                    .align_y(iced::Alignment::Center)
                    .push(
                        button(icon::heart(item.on_playlist(), on_cursor))
                            .style(button::text)
                            .padding([0, 8])
                            .on_press(Message::ToggleFavorite(item.id, item.on_playlist())),
                    )
                    .push(cell(
                        item.title.clone(),
                        NAME,
                        on_cursor,
                        // The one playing is the only thing in the table drawn
                        // in the accent colour, so it is findable at a glance
                        // in a list of twenty near-identical rows.
                        playing == Some(item.id),
                        false,
                    ))
                    .push(cell(item.creator.clone(), ARTIST, on_cursor, false, true))
                    .push(cell(album, ALBUM, on_cursor, false, true))
                    .push(cell(
                        clock(item.duration_ms as f64 / 1000.0),
                        TIME,
                        on_cursor,
                        false,
                        true,
                    ));
                col.push(
                    // The background belongs to a container spanning the whole
                    // width, not to a button around the title: a stripe that
                    // stops where the text does is not a row.
                    mouse_area(container(line).width(Length::Fill).padding([3, 4]).style(
                        move |theme: &iced::Theme| row_style(theme, on_cursor, focused, i % 2 == 1),
                    ))
                    .on_press(Message::PlayItem(item.id)),
                )
            });

        let head = container(
            Row::new()
                .spacing(0)
                .align_y(iced::Alignment::Center)
                .push(container(text("")).width(Length::Fixed(31.0)))
                .push(heading("Name", NAME))
                .push(heading("Artist", ARTIST))
                .push(heading("Album", ALBUM))
                .push(heading("Time", TIME)),
        )
        .width(Length::Fill)
        .padding([2, 4]);

        #[cfg_attr(feature = "demo", allow(unused_mut))]
        let mut main = column![row![
            text(peer.source.title().to_string()).size(22),
            text(format!("{} tracks", peer.rows().len()))
                .size(12)
                .style(text::secondary),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)]
        .spacing(10)
        .width(Length::Fill);

        #[cfg(not(feature = "demo"))]
        {
            // Turned away by the server: the one button that helps is sign in.
            let signed_out = self.login.as_ref().is_some_and(|l| l.token.is_empty());
            let actions = row![
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
            main = main.push(actions);
        }

        if self.help {
            // The status line stays: which pane has the cursor is exactly
            // what somebody reading the keymap is trying to work out.
            return main
                .push(Self::view_help())
                .push(self.view_status(peer))
                .into();
        }

        main.push(head)
            .push(rule::horizontal(1))
            .push(scrollable(rows).id(Self::TRACKS).height(Length::Fill))
            .push(self.view_status(peer))
            .into()
    }

    /// The keymap, because one that has to be read in the source is one nobody
    /// will find. `?` opens it and `?` or `<Esc>` closes it.
    fn view_help() -> Element<'static, Message> {
        const KEYS: &[(&str, &str)] = &[
            ("j  k", "down, up"),
            (
                "h  l",
                "left, right \u{2014} and out of a list, the next pane",
            ),
            ("{n}j", "a count: 5j is five down"),
            ("gg  G", "first, last. 7G is the seventh"),
            ("^d  ^u", "a page down, a page up"),
            ("<Tab>", "swap between the sidebar and the table"),
            (
                "<Enter>  o",
                "in the sidebar, step into it; in the table, play",
            ),
            ("<Space>", "heart the track under the cursor"),
            ("/", "search this pane; <Enter> accepts, <Esc> drops it"),
            ("n  N", "the next match, the one before"),
            ("p", "play or pause"),
            ("{  }", "the previous track, the next one"),
            ("?", "this"),
        ];
        let rows = KEYS.iter().fold(column![].spacing(6), |col, (keys, what)| {
            col.push(
                row![
                    text(*keys).size(13).width(Length::Fixed(110.0)),
                    text(*what).size(13).style(text::secondary),
                ]
                .spacing(12),
            )
        });
        container(column![text("keys").size(16), rows].spacing(12))
            .padding(12)
            .height(Length::Fill)
            .into()
    }

    /// The engine showing through: `cursor` is how much of the server's log has
    /// been applied, `pending` is what this peer has done that no server has
    /// confirmed yet. Kept in the demo — it is most of what the demo is for.
    fn view_status(&self, peer: &'_ Peer) -> Element<'_, Message> {
        let favorites = peer.on_playlist.get();
        let mut line = format!(
            "{} songs, {favorites} favourited · cursor {} · {} pending",
            peer.items.len(),
            peer.client.cursor(),
            peer.pending,
        );
        #[cfg(not(feature = "demo"))]
        {
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
            line = format!(
                "{who} · {} · {line}",
                if peer.link.is_some() {
                    "online"
                } else {
                    "offline"
                }
            );
        }
        if !self.note.is_empty() {
            line = format!("{line}  ·  {}", self.note);
        }
        // Which pane has the cursor, and anything half-typed. Without this a
        // swallowed `5`, or a `g` still waiting for its pair, is invisible —
        // which is the one thing that makes a modal keymap feel broken.
        let mode = match self.pane {
            Pane::Sidebar => "browse",
            Pane::Tracks => "tracks",
        };
        row![
            text(line)
                .size(12)
                .style(text::secondary)
                .width(Length::Fill),
            // A search shows a caret, so a half-typed query does not look
            // like a finished one that matched nothing.
            text(match self.keys.mode() {
                vim::Mode::Search(_) => format!("{}\u{2582}", self.keys.pending()),
                vim::Mode::Normal => self.keys.pending(),
            })
            .size(12)
            .style(text::primary),
            text(mode).size(12).style(text::secondary),
        ]
        .spacing(12)
        .into()
    }

    /// The now-playing bar. Pinned to the bottom, and honest about silence:
    /// on a build with no audio device it says so rather than drawing a
    /// transport that does nothing when pressed.
    fn view_bar(&self) -> Element<'_, Message> {
        let Some(track) = self.player.track() else {
            return container(
                text(if Player::AUDIBLE {
                    "nothing playing — pick a track"
                } else {
                    "nothing playing — pick a track (the desktop build has no audio device; \
                     the browser one streams)"
                })
                .size(12)
                .style(text::secondary),
            )
            .padding([8, 4])
            .into();
        };

        let position = self.player.position();
        let duration = self.player.duration().max(0.1);
        // Nothing here takes the cursor: `p`, `{` and `}` do all three, so a
        // pane for them would be a stop on `<Tab>` that nobody needs.
        let transport = row![
            button(icon::previous())
                .style(button::text)
                .on_press(Message::Skip(-1)),
            button(if self.player.is_playing() {
                icon::pause()
            } else {
                icon::play()
            })
            .style(button::text)
            .on_press_maybe(Player::AUDIBLE.then_some(Message::PlayPause)),
            button(icon::next())
                .style(button::text)
                .on_press(Message::Skip(1)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        container(
            row![
                transport,
                column![
                    text(track.title.clone()).size(14),
                    text(track.creator.clone()).size(12).style(text::secondary),
                ]
                .spacing(2)
                .width(Length::Fixed(260.0)),
                text(clock(position)).size(11).style(text::secondary),
                // Seeking is the element's job in a browser, and there is
                // nothing to seek without one — so the slider only moves where
                // a track can actually be moved to.
                slider(0.0..=duration as f32, position as f32, Message::Seek).width(Length::Fill),
                text(clock(duration)).size(11).style(text::secondary),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 4])
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

    /// A sans-io client has to be pumped by someone. This is that someone —
    /// and beside it, the keyboard.
    ///
    /// `keyboard::listen` reports only the presses **no widget took**, which is
    /// what makes a modeless vim layer safe here: while a text input has the
    /// focus it consumes its own keys and none of them reach this, so typing a
    /// song title cannot also walk the cursor down the list. There is no
    /// insert mode to get stuck in because there is nothing to get stuck in.
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick),
            iced::keyboard::listen().map(|event| match event {
                iced::keyboard::Event::KeyPressed { key, modifiers, .. } => {
                    Message::Key(key, modifiers)
                }
                // A release or a modifier change is not a command. `Tick` is
                // the harmless message: it pumps the transport and nothing else.
                _ => Message::Tick,
            }),
        ])
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

#[cfg(test)]
mod tests {
    use super::media_url;

    /// The join, and the two things that make it more than a `format!`.
    #[test]
    fn a_relative_file_is_joined_to_the_media_route() {
        assert_eq!(
            media_url("https://harken.example.com", "music/Bach/air.mp3"),
            "https://harken.example.com/media/music/Bach/air.mp3",
            "the scanner's path is what /media/ serves, so this is the join"
        );
        // Handed straight to an <audio> element the relative path resolves
        // against the page, loses the /media/ prefix, and a single-page
        // fallback answers it with index.html and a 200 — which reaches the
        // element as "the media resource was not suitable".
        assert!(media_url("https://harken.example.com", "music/a.mp3").contains("/media/"));

        // A whole URL is already an answer. The demo's library is Wikimedia
        // links, so this is not a corner case, it is the other half.
        let wiki = "https://upload.wikimedia.org/x.mp3";
        assert_eq!(media_url("https://harken.example.com", wiki), wiki);

        // Nothing to stream stays nothing, rather than becoming a URL that
        // resolves to the media root and 404s.
        assert_eq!(media_url("https://harken.example.com", ""), "");

        // A trailing slash on the server does not double up.
        assert_eq!(
            media_url("https://harken.example.com/", "music/a.mp3"),
            "https://harken.example.com/media/music/a.mp3"
        );
    }

    /// Real libraries are full of these, and `#` is the destructive one.
    #[test]
    fn the_awkward_characters_in_a_filename_are_escaped() {
        assert_eq!(
            media_url("https://h.example", "music/Ravel/Boléro #1 & 2.mp3"),
            "https://h.example/media/music/Ravel/Bol%C3%A9ro%20%231%20%26%202.mp3",
            "a raw # makes the request stop mid-filename and 404"
        );
        // The separators stay separators: escaping them would ask for one file
        // with slashes in its name.
        assert_eq!(
            media_url("", "a/b/c.mp3").matches('/').count(),
            4,
            "/media + three segments"
        );
    }
}
