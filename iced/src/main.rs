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
//! All of them are peers of one server, so a song the scanner finds is in the
//! library on the phone. Press the offline button in any of them, mutate on
//! both sides, come back online, and watch the rebase: what you did lands
//! after whatever arrived while you were away, because a playlist mutation
//! reads the end of the list rather than naming a position.
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

mod art;
mod covers;
mod icon;
mod listening;
mod palette;
mod player;
mod route;
/// The demo's library. Compiled only into the demo, so the client that
/// talks to a real server carries none of it.
#[cfg(feature = "demo")]
mod seed;
mod style;
mod vim;

use std::time::Duration;

use harken::{self as mutators, HarkenApp, Item};
use iced::widget::{
    button, column, container, image, mouse_area, pin, row, rule, scrollable, slider, stack, text,
    text_input, Row,
};
use iced::{Element, Length, Subscription, Task};
use petros::{AutoCtx, Changes, Client};
use player::{Player, Track};
// The tab's title and the platform's media controller are the browser's, the
// way the `<audio>` element is; the desktop build has neither.
#[cfg(target_arch = "wasm32")]
use player::Remote;

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
    /// Every album, and everyone who made something: the two index pages.
    ///
    /// These used to be *headings* over a sidebar row per album and per
    /// artist, which is fine for the twenty a demo has and wrong for a library
    /// — a sidebar that grows with the collection is a sidebar you scroll to
    /// find the thing you scroll. They are pages now, and the sidebar is the
    /// four fixed places plus whatever lists you made.
    Albums,
    Artists,
    /// Everyone this library has a work *by*, which is not the same list as
    /// `Artists`: that one is every `media.creator` for every kind, and this
    /// one is exactly the people some `work` is by. A library of pop and
    /// podcasts has none of them, and the sidebar then draws no such line.
    Composers,
    Playlist(harken::Id<harken::tables::Playlist>, String),
    Album(String),
    Artist(String),
    /// One composer's works. Reached from a card on `Composers`.
    Works(String),
    /// One work, and the performances of it this library holds. The first is
    /// the derived key `work_key` made and the second is the title to draw —
    /// carried together because a page needs both and the key is not a name.
    Work(String, String),
    /// One performance, in the order the work goes. The label is who played
    /// it, which is the only thing that tells two of them apart.
    Recording(String, String),
}

impl Source {
    /// How the address bar spells it. Names rather than ids: see `route.rs`.
    fn route(&self) -> route::Route {
        match self {
            Source::Library => route::Route::Library,
            Source::Albums => route::Route::Albums,
            Source::Artists => route::Route::Artists,
            Source::Composers => route::Route::Composers,
            Source::Playlist(_, name) => route::Route::Playlist(name.clone()),
            Source::Album(name) => route::Route::Album(name.clone()),
            Source::Artist(name) => route::Route::Artist(name.clone()),
            Source::Works(name) => route::Route::Composer(name.clone()),
            // The key, not the title — and it is still a name in the sense
            // `route.rs` means, because `work_key` builds it out of the
            // composer and the catalogue number:
            // `#work/johann-sebastian-bach/bwv-988` is a link somebody can
            // read and type. What that rule forbids is a uuid.
            Source::Work(id, _) => route::Route::Work(id.clone()),
            Source::Recording(id, _) => route::Route::Recording(id.clone()),
        }
    }

    /// The heading this source belongs under, or `None` for the one line that
    /// needs no heading. What makes the sidebar a flat list of choices with
    /// headings *derived* rather than interleaved — so a cursor can address
    /// line N without counting past decoration.
    fn heading(&self) -> Option<&'static str> {
        match self {
            Source::Library | Source::Albums | Source::Artists | Source::Composers => Some("Music"),
            Source::Playlist(..) => Some("Playlists"),
            // Reached from a page rather than from the sidebar, so there is no
            // line to put a heading over. `Choice::heading` is only ever asked
            // about a line the sidebar actually draws.
            Source::Album(_)
            | Source::Artist(_)
            | Source::Works(_)
            | Source::Work(..)
            | Source::Recording(..) => None,
        }
    }

    fn title(&self) -> &str {
        match self {
            Source::Library => "Songs",
            Source::Albums => "Albums",
            Source::Artists => "Artists",
            Source::Composers => "Composers",
            Source::Playlist(_, name)
            | Source::Album(name)
            | Source::Artist(name)
            | Source::Works(name)
            | Source::Work(_, name)
            | Source::Recording(_, name) => name,
        }
    }

    /// What a page of this kind is called, above its title.
    ///
    /// The one word that says which of the five things you are looking at,
    /// because "Goldberg Variations" is a work and a record and neither is
    /// obvious from the name alone.
    fn kind(&self) -> &'static str {
        match self {
            Source::Album(_) => "Album",
            Source::Artist(_) => "Artist",
            Source::Works(_) => "Composer",
            Source::Work(..) => "Work",
            Source::Recording(..) => "Recording",
            _ => "",
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
/// a key of its own — `<Space>`, `{`, `}` — so making it a third place the
/// cursor can be would only add a stop to `<Tab>` nobody needs to pass through.
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
    // The rule is the domain's, because `file`'s meaning is: a path every
    // device joins to its own server. Three things do it now — this, the
    // phone, and a speaker with no client at all — so it is written where the
    // column is described rather than here.
    harken::listening::url(server, file)
}

/// Percent-encode one path segment.
///
/// Real libraries are full of spaces, ampersands and the occasional `#`, and a
/// `#` is the one that is silently destructive: everything after it is a
/// fragment, so the request goes out for a path that stops mid-filename and
/// the server answers 404. Everything outside RFC 3986's unreserved set is
/// escaped, which covers those and every non-ASCII byte.
pub(crate) fn encode(part: &str, out: &mut String) {
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
    /// Whether this row is on the playlist the view was read against.
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
    /// Where the pointer is, in the window. See `App::cursor`.
    Hover(iced::Point),
    /// Open a row's menu, at the pointer: the three dots, or a right click.
    RowMenu(Id),
    CloseMenu,
    /// Open the playlist picker over the track under the cursor.
    OpenPicker,
    /// Move its cursor, by click rather than by `j`.
    PickerAt(usize),
    /// The same, for the row menu.
    MenuAt(usize),
    /// Run the entry it is on.
    MenuActivate,
    /// Toggle the row it is on — or, on the last row, start naming a new one.
    PickerActivate,
    /// What is being typed into the new-playlist box.
    PickerName(String),
    /// Make it. A blank name is refused by `apply`, not here.
    PickerCreate,
    ClosePicker,
    /// The pointer moved onto a row or a card.
    ///
    /// Content panes only. In the sidebar the cursor *is* the selection —
    /// moving onto a line shows it — so hovering there would navigate on the
    /// way past, and a highlight that did not navigate would be a second
    /// meaning for the one highlight the sidebar has.
    HoverAt(usize),
    /// A cover arrived, or did not.
    Cover(covers::Loaded),
    /// A key nothing on screen wanted. See `subscription`.
    Key(iced::keyboard::Key, iced::keyboard::Modifiers),
    /// The window changed size. Only the menu cares.
    Resized(iced::Size),
    /// Which device is making the sound: open the picker, walk it, pick.
    OpenDevices,
    DeviceAt(usize),
    /// `None` is the last row: stop it everywhere.
    PickDevice(Option<listening::DeviceId>),
    CloseDevices,
    /// Pump the transport. Nothing else drives a sans-io client.
    Tick,
}

/// What the now-playing bar draws, from wherever it came.
///
/// A struct rather than the player, because the bar has two sources — this
/// device's element, and whatever the session says another device is doing —
/// and a view that branched on that twice would eventually branch on it
/// differently in the two places.
struct Bar {
    title: String,
    creator: String,
    playing: bool,
    position: f64,
    duration: f64,
}

/// What `view_devices` draws a row at, and `fit` has to assume.
const DEVICE_ROW: f32 = 208.0;
const DEVICES_WIDTH: f32 = 216.0;

/// How the next change to what is shown should reach the history.
///
/// Everything is a place you went, except walking the sidebar's cursor: that
/// is one navigation however many rows it passes through, and recording each
/// would leave a back button that needs forty presses to undo one scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nav {
    Push,
    Replace,
}

/// A row's menu: what the three dots open, and what a right click opens.
///
/// Held by id rather than by index, because the list under it can move while
/// it is up — an entry arriving from the server re-splices the maintained view
/// and the row that was third is not third any more.
struct RowMenu {
    media: Id,
    title: String,
    album: String,
    artist: String,
    /// Where it was opened, in the window. Pinned there rather than centred,
    /// because a menu that appears somewhere else is a menu you have to look
    /// for after asking for it.
    origin: iced::Point,
    /// Which entry the keyboard is on. A menu you can only reach with the
    /// pointer is a menu that is missing from half this window's controls —
    /// everything else here answers to `j` and `k`.
    at: usize,
}

impl RowMenu {
    /// What it offers, in one place: the view draws these and `<Enter>` runs
    /// them, so the two cannot come to disagree about what the third entry is.
    ///
    /// Every one is something this window could already do. What the menu adds
    /// is asking for it *about a row*, which neither a key nor a click on the
    /// row itself could say.
    fn entries(&self) -> Vec<(String, Message)> {
        let mut out = vec![
            ("Play".to_string(), Message::PlayItem(self.media)),
            ("Add to playlist\u{2026}".to_string(), Message::OpenPicker),
        ];
        if !self.album.is_empty() {
            out.push((
                format!("Go to {}", self.album),
                Message::Select(Source::Album(self.album.clone())),
            ));
        }
        if !self.artist.is_empty() {
            out.push((
                format!("Go to {}", self.artist),
                Message::Select(Source::Artist(self.artist.clone())),
            ));
        }
        out
    }
}

/// The playlist picker, over the track it is for.
///
/// `a` on the track list opens it. A list rather than a menu, for the reason
/// the phone uses a sheet: however many playlists exist is however many rows
/// this has, and a menu that scrolls is a list pretending not to be one.
///
/// It is a **toggle**. A list of every playlist with nothing marked is a list
/// you can put the same track on twice and never take it off, so it asks
/// `playlists_of` when it opens and every row says which it is.
struct Picker {
    media: Id,
    /// The track's title, drawn above the list. Held rather than looked up:
    /// the list underneath can change while this is open.
    title: String,
    lists: Vec<(harken::Id<harken::tables::Playlist>, String, bool)>,
    /// Where the cursor is, over the playlists and then the row that makes
    /// one — which is why the new-playlist row is a row and not a key. One
    /// list, one cursor, and nothing extra to learn.
    at: usize,
    /// Where to pin it, when it was opened *from* the row menu — a submenu
    /// beside its parent, which stays up behind it the way a submenu's parent
    /// does. `None` is `a` on the track list, which has no parent and no
    /// pointer to sit under, so it is centred.
    origin: Option<iced::Point>,
    /// What is being typed into the new-playlist box, if it is open.
    ///
    /// While this is `Some` a `text_input` has the focus and takes its own
    /// keys, so none of them reach the vim layer — which is what makes a
    /// modeless keymap safe beside a box you can type a name into.
    naming: Option<String>,
}

/// One line of the sidebar: somewhere the cursor can be and something it can
/// open.
struct Choice {
    source: Source,
    label: String,
    /// The tally drawn on the right, where there is one to draw.
    count: Option<i64>,
}

// The table's columns. Portions rather than pixels, so the text columns share
// whatever width is left after the transport, the track number and the duration,
// and none of them can push the others off the edge at a narrow window.
/// The column the playing row's transport sits in, and the gutter every
/// other row leaves empty. A fixed width, so the titles line up whatever is
/// or is not playing.
const TRANSPORT: Length = Length::Fixed(31.0);
/// iced's default scrollbar, which takes its width out of the content's.
///
/// It has to be in `columns_in`'s arithmetic: a page of cards is inside a
/// `scrollable`, so what a row of cards actually gets is this much narrower
/// than the pane it is in.
const SCROLLBAR: f32 = 10.0;
const TRACK: Length = Length::Fixed(30.0);
const NAME: Length = Length::FillPortion(5);
const ARTIST: Length = Length::FillPortion(3);
const ALBUM: Length = Length::FillPortion(4);
const TIME: Length = Length::Fixed(56.0);

/// Roughly how many characters fit in one portion of the table, at size 13.
///
/// An estimate on purpose. iced lays text out in pixels and this runs before
/// layout, so the exact answer is not available here — but the failure it
/// prevents is not subtle: `Wrapping::None` does not shorten a string, it
/// draws it at full length, and a long title runs straight under the next
/// column. Erring short costs an ellipsis nobody needed; erring long costs
/// two columns of text on top of each other.
const PER_PORTION: usize = 13;

/// Shorten to `max` characters, taking the middle out rather than the end.
///
/// The ends are what identify a track: a Bach movement is
/// `Prelude No. 14 in F-sharp minor, BWV 859` and the tail carries the
/// catalogue number that tells it from the other twenty-three preludes.
/// Clipping the end leaves a column of `Prelude No. 14 in F-shar…`, which is
/// the half that is the same in all of them.
fn middle(body: &str, max: usize) -> String {
    let chars: Vec<char> = body.chars().collect();
    if chars.len() <= max || max < 5 {
        return body.to_string();
    }
    // One for the ellipsis, and the remainder split with the bias forward:
    // the head is doing more work than the tail.
    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('\u{2026}');
    out.extend(chars[chars.len() - tail..].iter());
    out
}

/// One cell of the table: a single line, clipped rather than wrapped.
///
/// Every color is asked of the theme rather than written down, which is the
/// whole of what makes this work in dark mode — and on the cursor's own row,
/// which is painted in the accent color and needs text chosen against *that*
/// rather than against the window.
fn cell<'a>(
    body: String,
    width: Length,
    on_cursor: bool,
    accent: bool,
    dim: bool,
) -> Element<'a, Message> {
    // Shortened here rather than by the renderer, because `Wrapping::None`
    // clips nothing — it draws the whole string, over whatever is next to it.
    let body = match width {
        Length::FillPortion(n) => middle(&body, n as usize * PER_PORTION),
        _ => body,
    };
    text(body)
        .size(13)
        .width(width)
        .wrapping(text::Wrapping::None)
        .style(move |theme: &iced::Theme| {
            let palette = palette::of(theme);
            let color = if on_cursor {
                // The row is filled with the accent color, so there is exactly
                // one color text on it can be: the one that color was paired
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
            color: Some(palette::of(theme).background.base.text.scale_alpha(0.55)),
        })
        .into()
}

/// A section heading inside the table: the part of the work below it.
///
/// Drawn in the accent color rather than filled with it, the way the playing
/// track is, because a filled stripe is what the cursor means here and there
/// can only be one of those. It sits in the same grid as the rows and takes
/// the whole width, so a work with four suites reads as four blocks rather
/// than as one list with a repeated column.
fn section<'a>(label: String) -> Element<'a, Message> {
    container(
        text(label)
            .size(12)
            .wrapping(text::Wrapping::None)
            .style(|theme: &iced::Theme| text::Style {
                color: Some(palette::of(theme).primary.base.color),
            }),
    )
    .width(Length::Fill)
    // Indented to where the track numbers start, so the heading sits over the
    // column it heads rather than out in the transport's gutter.
    .padding(iced::Padding {
        top: 8.0,
        right: 4.0,
        bottom: 2.0,
        left: 35.0,
    })
    .into()
}

/// What a table row is painted.
///
/// Three states, and they are deliberately not three shades of the same idea:
/// the cursor is the accent color when its pane has the keyboard and a plain
/// strong grey when it does not — the way a native list dims its selection
/// when you click away — and everything else is the zebra, which is the
/// window's own background alternating with the faintest step up from it.
/// Nothing here is a literal color, so a dark theme restates all of it.
fn row_style(theme: &iced::Theme, on_cursor: bool, focused: bool, odd: bool) -> container::Style {
    let palette = palette::of(theme);
    let background = if on_cursor {
        Some(if focused {
            palette.primary.base.color
        } else {
            palette.background.strong.color
        })
    } else if odd {
        // A wash of the text color rather than `background.weak`: from a base
        // this dark iced's generated step is the only direction it can go and
        // it goes a long way, which reads as a striped table rather than as a
        // list you can follow across. Alpha, so the same number is a lift on
        // a dark theme and a shade on a light one.
        Some(palette.background.base.text.scale_alpha(0.045))
    } else {
        None
    };
    container::Style {
        background: background.map(iced::Background::Color),
        ..container::Style::default()
    }
}

/// Seconds as `m:ss`, which is how long a piece of music is written down.
/// Who played it and on what terms, as one line.
///
/// The two are separate facts on separate rows — `credit` and
/// `recording.licence` — and this is the only place they are put back together,
/// because a table has one column for "who". A recording that reserves nothing
/// is just the names; one that asks for attribution says so in the same breath.
fn credit(performer: &str, licence: &str) -> String {
    match (performer.is_empty(), licence.is_empty()) {
        (_, true) => performer.to_string(),
        (true, false) => format!("({licence})"),
        (false, false) => format!("{performer} ({licence})"),
    }
}

/// How many cards fit across an index page, at this window width.
///
/// A free function so it can be tested: `cards_never_overflow_their_row` walks
/// every width a window can be and checks the answer actually fits, which is
/// the only way to know, because getting it wrong is silent.
///
/// **And it was wrong.** The room a card has is the window less the sidebar,
/// less the page's padding — and less the *scrollbar*, which this did not
/// subtract. Ten pixels, and at the widths where N cards needed every one of
/// them the row came out over-full; iced clamps a `Fixed` child to the space
/// left, so the last card in the row was drawn smaller than the rest. One card
/// out of five at the wrong size reads as a rendering bug and is arithmetic.
fn columns_in(width: f32) -> usize {
    let room = (width - App::SIDEBAR_WIDTH - App::PAGE_PADDING * 2.0 - SCROLLBAR).max(0.0);
    (((room + App::CARD_GAP) / (App::CARD + App::CARD_GAP)) as usize).max(1)
}

/// "1685–1750", or "1685–" for somebody still alive, or nothing at all.
///
/// Empty rather than a dash where a date should be: a composer nobody has
/// described yet gets the tally under their name instead, which is a fact.
fn lifespan(born: i64, died: i64) -> String {
    match (born, died) {
        (0, 0) => String::new(),
        (0, d) => format!("–{d}"),
        (b, 0) => format!("{b}–"),
        (b, d) => format!("{b}–{d}"),
    }
}

fn clock(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return String::new();
    }
    let secs = secs as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// How long a whole record runs, in the units people say it in.
///
/// `clock` above is `m:ss` because that is how one piece is written down; a
/// two-and-a-half-hour oratorio in `m:ss` is `150:37`, which nobody reads as a
/// length. So a header says "2 hr 30 min" and a row still says "4:21".
fn spell(ms: i64) -> String {
    let minutes = ms / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} hr"),
        (h, m) => format!("{h} hr {m} min"),
    }
}

/// `1 track`, `2 tracks`. English, and only for the two words this uses.
fn plural(n: i64, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

/// One cell of an index page.
///
/// Built from an `Album` or an `Artist` at the point of drawing rather than
/// being a third thing the domain returns: what a card needs is a name, a line
/// under it, a number and somewhere to go, and those are the same four for
/// both — which is why there is one grid and not two.
struct Card {
    /// What the square is derived from. The name, for both, so the square an
    /// album has on this page is the square it has on the phone.
    seed: String,
    title: String,
    /// The line under the title, or empty to fall back to the tally.
    under: String,
    tally: i64,
    open: Source,
    /// A person is a circle and a record is a square.
    round: bool,
    /// The row's `art`, if anything — `media.file`'s spelling, joined to
    /// this peer's server when it is drawn. Empty is the normal case and means
    /// the derived square.
    art: String,
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
    /// The playlist the library view is read against, which is what gives
    /// every row its `on_playlist`. The domain has no favorites of its own —
    /// a playlist named Favorites is just the first one.
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
    details: std::collections::HashMap<harken::Id<harken::tables::Media>, harken::TrackDetail>,
    /// The sidebar, rebuilt when the library changes rather than per frame.
    ///
    /// One flat list of the lines a cursor can sit on, in the order they are
    /// drawn. Headings are derived from it while drawing rather than stored in
    /// it, so "line 9" means the same thing to the keyboard and to the eye.
    choices: Vec<Choice>,
    /// The two index pages' contents, read when the library changes.
    ///
    /// They used to be sidebar rows and are lists in their own right now. Held
    /// rather than re-queried per frame for the reason the sidebar is: `view`
    /// takes `&self`, and a grid of two hundred albums is not a thing to
    /// rebuild sixty times a second. They are also what `source_of` resolves a
    /// `#album/…` link against, now that no sidebar line carries one.
    albums: Vec<harken::Album>,
    artists: Vec<harken::Artist>,
    /// Everyone this library has a *work* by, which is not `artists`: that one
    /// is every `media.creator` for every kind and this one is exactly the
    /// people some `work` is by. Empty for a library of pop and podcasts, and
    /// the sidebar then draws no Composers line at all.
    composers: Vec<harken::Composer>,
    /// What the current page shows when it is not a list of tracks: one
    /// composer's works, or one work's recordings. Read by `reload_shown`
    /// beside `shown`, for the same reason and at the same moment.
    works: Vec<harken::Work>,
    recordings: Vec<harken::Recording>,
    /// Bumped whenever the two lists above are rebuilt.
    ///
    /// The App watches it to know when to ask for covers. It used to ask when
    /// the *wire* brought something, which is not the same question and was
    /// wrong in both directions: the demo has no server, so nothing ever
    /// arrived and no cover was ever fetched; and a real client's first
    /// library comes from opening the database, before any sync lands.
    art_gen: u64,
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
    /// `a` — which playlists the track under the cursor is on. A popup over
    /// the list rather than in place of it: it is about a row, and a panel
    /// that replaces the rows hides the one it is about.
    picker: Option<Picker>,
    /// The three dots, or a right click.
    menu: Option<RowMenu>,
    /// How big the window is, so a menu opened near an edge can open the
    /// other way. Seeded with what `main` asks for and kept in step by
    /// `window::resize_events`.
    window: iced::Size,
    /// Whether the next reconciliation adds to the history or rewrites it.
    /// Reset to [`Nav::Push`] every time, so only the step that meant
    /// otherwise is the one that gets it.
    nav: Nav,
    /// The last fragment this window acted on.
    ///
    /// Without it the tick would re-resolve the same route twenty times a
    /// second — and a route naming an album this peer has not got resolves to
    /// the library every time, so it would also re-read the list twenty times
    /// a second for as long as the URL said so.
    routed: Option<route::Route>,
    /// Where the pointer is, so a menu can open where it was asked for.
    ///
    /// Tracked on the root, so the coordinates and the `pin` that places the
    /// menu share an origin. It costs a message per mouse move — the view is
    /// already rebuilt twenty times a second by the tick, and this is the only
    /// way to put a menu under the pointer without one.
    cursor: iced::Point,
    /// What is playing, and whether this build can sound it. See `player.rs`.
    player: Player,
    /// What `Skip` moves through: the list as it stood when play was pressed.
    ///
    /// A snapshot rather than a reference to the shown list, so that changing
    /// the sidebar selection — or somebody else's edit arriving — does not
    /// silently redirect what plays next.
    ///
    /// [`listening::Track`] rather than `Item`, because this queue is also
    /// what a hand-off carries: the other device gets the list, not a
    /// reference into a library it may not have yet. One type, so reporting
    /// costs a clone rather than a conversion per frame.
    queue: Vec<listening::Track>,
    /// This account's listening session: every device signed in as this
    /// person, which one is making the sound, and what it is playing.
    listening: listening::Remote,
    /// Covers, fetched once and kept: a directory on a desktop, the browser's
    /// Cache API in a tab. Empty until something asks, which is what makes an
    /// album with no artwork cost nothing.
    covers: covers::Covers,
    /// The `Peer::art_gen` the covers were last asked for.
    art_seen: u64,
    /// The device picker, when it is up. `at` walks it like every other
    /// overlay; the last row is "nowhere".
    devices: Option<usize>,
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

        // The view is read against a playlist, so there has to be one. The first
        // run of a peer makes it; after that it is whichever came back first,
        // which is stable because playlists are ordered by when they were made.
        //
        // Made *before* anything has synced, necessarily — the list is empty
        // because the log has not arrived, not because nobody has one. Which
        // is why `create_playlist` refuses a name this person already has:
        // three devices would otherwise author three of these. The one that
        // loses is dropped on the rebase, and `reload_sidebar` re-points this
        // when that happens.
        let playlist = {
            let existing = harken::playlists(&mut client.store()).unwrap_or_default();
            match existing.first() {
                Some(p) => p.id,
                None => {
                    let _ = client.mutate(mutators::create_playlist("Favorites".into()));
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
            playlist,
            source: Source::Library,
            shown: Vec::new(),
            details: std::collections::HashMap::new(),
            choices: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            composers: Vec::new(),
            works: Vec::new(),
            recordings: Vec::new(),
            art_gen: 0,
            items: Vec::new(),
            pending: 0,
        };
        // The one full read of the list. Everything after this is maintained.
        {
            let mut store = peer.client.store();
            peer.library.hydrate(&mut store);
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
        let composers = harken::composers(&mut store).unwrap_or_default();
        drop(store);

        // The playlist the view is read against can stop existing, and now
        // routinely does: `create_playlist` refuses a name this person already
        // has, so the default this peer authored on its first run is dropped
        // on the rebase when another device's turns out to have been first.
        // Holding the id it chose would leave the view reading against a row
        // nobody has.
        //
        // Re-hydrating is what re-pointing costs — one full read of the list,
        // which is what `library_view` is built from — so it happens only when
        // the id actually moved, not on every refresh.
        let first = playlists.first().map(|p| p.id).unwrap_or_default();
        if !playlists.iter().any(|p| p.id == self.playlist) && self.playlist != first {
            self.playlist = first;
            self.library = harken::library_view(first);
            let mut store = self.client.store();
            self.library.hydrate(&mut store);
            drop(store);
            self.items = harken::items_of(&self.library);
        }

        self.details = harken::track_details(&mut self.client.store())
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.media_id, t))
            .collect();

        // Music, then the lists somebody made. The sidebar is a fixed height
        // now: it was one line per album and one per artist, which is readable
        // at twenty albums and is a second scrolling list at two hundred.
        // Where those went is a page, which is what the first three lines are.
        //
        // **A line is drawn only when it has rows behind it**, which is what
        // lets one schema serve every genre without every library growing
        // pages about nothing. A pop library has no works, so it has no
        // Composers line; a library of podcasts has no albums either. Songs is
        // unconditional because it is the library, and an empty library is
        // still the thing you are looking at.
        let mut choices = vec![Choice {
            source: Source::Library,
            label: "Songs".into(),
            count: Some(self.items.len() as i64),
        }];
        for (source, label, count) in [
            (Source::Albums, "Albums", albums.len()),
            (Source::Artists, "Artists", artists.len()),
            (Source::Composers, "Composers", composers.len()),
        ] {
            if count > 0 {
                choices.push(Choice {
                    source,
                    label: label.into(),
                    count: Some(count as i64),
                });
            }
        }
        choices.extend(playlists.into_iter().map(|p| Choice {
            source: Source::Playlist(p.id, p.name.clone()),
            label: p.name,
            count: None,
        }));
        self.choices = choices;
        self.albums = albums;
        self.artists = artists;
        self.composers = composers;
        self.art_gen = self.art_gen.wrapping_add(1);
    }

    /// What is true of a track as a song, or nothing if its kind has none.
    fn detail_of(&self, id: harken::Id<harken::tables::Media>) -> harken::TrackDetail {
        self.details
            .get(&id)
            .cloned()
            .unwrap_or(harken::TrackDetail {
                media_id: id,
                album: String::new(),
                track: 0,
                part: String::new(),
                catalogue: String::new(),
                performer: String::new(),
                licence: String::new(),
                bpm: 0,
            })
    }

    /// The list under the header, for whatever the sidebar picked.
    ///
    /// The library is the maintained one and costs nothing here. The others are
    /// a query, because a filtered list is not what the view is maintaining —
    /// and a selection is a click, so paying for it there is the right place.
    fn reload_shown(&mut self) {
        // The two lists a page of cards or of recordings draws. Cleared first,
        // so a page that is neither cannot show the last one's — the same rule
        // `shown` follows and for the same reason.
        self.works.clear();
        self.recordings.clear();
        self.shown = match self.source.clone() {
            Source::Library => return,
            // An index page draws a grid, not a list of tracks. Cleared rather
            // than left alone so that `rows()` cannot hand the transport the
            // previous page's queue.
            Source::Albums | Source::Artists | Source::Composers => {
                self.shown.clear();
                return;
            }
            Source::Works(composer) => {
                self.works = harken::works(&mut self.client.store(), composer).unwrap_or_default();
                self.shown.clear();
                return;
            }
            // Both halves: the work itself, for the header, and its
            // performances, for the page. `works` is one composer's list and
            // this page may have been reached without opening a composer at
            // all — a link somebody sent — so the row is read by key.
            Source::Work(id, _) => {
                let mut store = self.client.store();
                self.works = harken::work(&mut store, id.clone()).unwrap_or_default();
                self.recordings = harken::recordings(&mut store, id).unwrap_or_default();
                self.shown.clear();
                return;
            }
            Source::Playlist(id, _) => harken::playlist(&mut self.client.store(), id),
            Source::Album(name) => harken::album(&mut self.client.store(), self.playlist, name),
            Source::Artist(name) => harken::artist(&mut self.client.store(), self.playlist, name),
            // The one list in the domain ordered by the *work* rather than by
            // the release: a compilation puts the Moonlight's first movement
            // at track nine and a page about the sonata has to put it first.
            Source::Recording(id, _) => {
                // Every recording of the same work, so the header can find
                // this one — `recording_key` puts the work's key before the
                // `@`, which is what makes that askable from the id alone.
                if let Some((work, _)) = id.split_once('@') {
                    let mut store = self.client.store();
                    self.works = harken::work(&mut store, work.to_string()).unwrap_or_default();
                    self.recordings =
                        harken::recordings(&mut store, work.to_string()).unwrap_or_default();
                }
                // A link arriving cold carries the key and no label, so the
                // page would be titled with an empty string. The row knows who
                // played it; this is where the two meet.
                if let Source::Recording(id, who) = &mut self.source {
                    if who.is_empty() {
                        if let Some(take) = self.recordings.iter().find(|r| r.id == *id) {
                            who.clone_from(&take.performers);
                        }
                    }
                }
                let id = match &self.source {
                    Source::Recording(id, _) => id.clone(),
                    _ => unreachable!("matched just above"),
                };
                harken::recording(&mut self.client.store(), self.playlist, id)
            }
        }
        .unwrap_or_default();
    }

    /// What the main list is showing, whichever side it came from.
    /// The title of a work, by its key, or `None` if this peer has no such
    /// work.
    ///
    /// A read rather than a lookup in `works`, because that list is one
    /// composer's and a `#work/…` link can arrive before any composer has been
    /// opened — which is exactly what a link somebody sent does.
    fn work_named(&mut self, id: &str) -> Option<String> {
        harken::work(&mut self.client.store(), id.to_string())
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|w| w.title)
    }

    /// The source a route names, against what this peer actually has.
    ///
    /// A link to a playlist that has since been renamed, or to an album this
    /// peer has not received yet, lands on the library — which is a page, and
    /// better than a heading with nothing under it.
    fn source_of(&mut self, route: &route::Route) -> Source {
        // An album and an artist are no longer sidebar lines, so the lists
        // they *are* have to be what a link resolves against. Same rule as
        // before — a name this peer does not have lands on the library, which
        // is a page rather than a heading with nothing under it.
        match route {
            route::Route::Album(name) => {
                return match self.albums.iter().any(|a| a.name == *name) {
                    true => Source::Album(name.clone()),
                    false => Source::Library,
                }
            }
            route::Route::Artist(name) => {
                return match self.artists.iter().any(|a| a.name == *name) {
                    true => Source::Artist(name.clone()),
                    false => Source::Library,
                }
            }
            route::Route::Composer(name) => {
                return match self.composers.iter().any(|c| c.name == *name) {
                    true => Source::Works(name.clone()),
                    false => Source::Library,
                }
            }
            // A work and a recording are resolved by *querying*, not against a
            // list this peer holds: the lists here are the current page's, and
            // a link arriving cold has no page yet. The key is enough to ask
            // with, which is the other half of what makes it a good route.
            route::Route::Work(id) => {
                return match self.work_named(id) {
                    Some(title) => Source::Work(id.clone(), title),
                    None => Source::Library,
                }
            }
            route::Route::Recording(id) => {
                // The work's key is everything before the `@` — see
                // `recording_key`. A recording nobody has is the library, the
                // same as a renamed playlist.
                let Some((work, _)) = id.split_once('@') else {
                    return Source::Library;
                };
                return match self.work_named(work) {
                    Some(_) => Source::Recording(id.clone(), String::new()),
                    None => Source::Library,
                };
            }
            _ => {}
        }
        self.choices
            .iter()
            .find(|c| c.source.route() == *route)
            .map(|c| c.source.clone())
            .unwrap_or(Source::Library)
    }

    fn rows(&self) -> &[Item] {
        match self.source {
            Source::Library => &self.items,
            _ => &self.shown,
        }
    }

    /// Every album this artist made something on.
    ///
    /// Derived from the list already read rather than from a query of its own:
    /// `Album::creator` is whoever made its first track, which is the same
    /// fact an artist page is selecting on.
    fn albums_by(&self, artist: &str) -> Vec<&harken::Album> {
        self.albums.iter().filter(|a| a.creator == artist).collect()
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
    /// The same login against a database of its own, which is what a test
    /// wants: the path is `petros-demo-{id}.db` and cargo runs a binary's
    /// tests on several threads, so two of them sharing one name fail only
    /// when run *together*.
    #[cfg(all(test, feature = "demo"))]
    fn demo_login_for(who: &str) -> Login {
        let mut login = Self::demo_login();
        login.user.id = who.to_string();
        login
    }

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
                picker: None,
                menu: None,
                nav: Nav::Push,
                routed: None,
                cursor: iced::Point::ORIGIN,
                window: iced::Size::new(860.0, 600.0),
                player: Player::new(),
                queue: Vec::new(),
                listening: listening::Remote::new(),
                covers: covers::Covers::default(),
                art_seen: 0,
                devices: None,
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
                picker: None,
                menu: None,
                nav: Nav::Push,
                routed: None,
                cursor: iced::Point::ORIGIN,
                window: iced::Size::new(860.0, 600.0),
                player: Player::new(),
                queue: Vec::new(),
                listening: listening::Remote::new(),
                covers: covers::Covers::default(),
                art_seen: 0,
                devices: None,
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
                app.listening
                    .open(&app.server, &login.token, &login.session);
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
        // A device is a login, so a new login is a new device — which is
        // what makes signing out and back in honestly a different row in
        // somebody else's picker, and a reloaded tab the same one.
        self.listening
            .open(&self.server, &login.token, &login.session);
        self.login = Some(login);
    }

    /// The index pages' geometry, shared by the view that draws the cards and
    /// the arithmetic that decides where `l` lands.
    ///
    /// Written once because the two have to agree: a grid drawn four across
    /// and walked as though it were five puts the cursor on a card nobody can
    /// see, and it would look like the keymap skipping rows at random.
    const SIDEBAR_WIDTH: f32 = 200.0;
    const PAGE_PADDING: f32 = 16.0;
    const CARD: f32 = 132.0;
    const CARD_GAP: f32 = 16.0;

    // The two scrollables the cursor has to keep itself inside of.
    const SIDEBAR: &'static str = "sidebar";
    const TRACKS: &'static str = "tracks";
    /// The new-playlist box, so opening it can put the keyboard in it.
    const NAMING: &'static str = "naming";
    /// What `view_menu` draws and `fit` has to assume: 190 of entry inside 4
    /// of padding either side.
    const MENU_WIDTH: f32 = 198.0;
    /// …and the picker's, which is a panel and also a submenu.
    const PICKER_WIDTH: f32 = 340.0;
    const PICKER_MAX_HEIGHT: f32 = 420.0;

    /// Where the cursor is in a pane.
    fn at(&self, pane: Pane) -> usize {
        self.cursors[pane as usize]
    }

    /// What shape a pane is, which is all `vim` needs to know about it.
    ///
    /// The only place the application says "this one is a list and that one is
    /// a row". Everything else — counts, `gg`, whether `h` leaves the pane —
    /// falls out of the shape.
    fn shape(&self, pane: Pane) -> vim::Grid {
        let peer = self.peer.as_ref();
        match pane {
            Pane::Sidebar => vim::Grid::column(peer.map_or(0, |p| p.choices.len())),
            // The only pane that is ever more than one column: an index page
            // is laid out in rows of cards, and saying so is the whole of what
            // the keyboard needs to be told. `Grid::column` below is a list,
            // and refuses `h` and `l` because every cell in one column is on
            // both of its horizontal edges — not because anything here says
            // "this one is a list".
            Pane::Tracks => match peer.map(|p| (&p.source, p)) {
                Some((Source::Albums, p)) => vim::Grid {
                    cells: p.albums.len(),
                    columns: self.columns(),
                },
                Some((Source::Artists, p)) => vim::Grid {
                    cells: p.artists.len(),
                    columns: self.columns(),
                },
                Some((Source::Composers, p)) => vim::Grid {
                    cells: p.composers.len(),
                    columns: self.columns(),
                },
                Some((Source::Works(_), p)) => vim::Grid {
                    cells: p.works.len(),
                    columns: self.columns(),
                },
                // A column, because a work's recordings are rows: `h` and `l`
                // refuse there and hand the cursor back to the sidebar, which
                // is what one column has always meant.
                Some((Source::Work(..), p)) => vim::Grid::column(p.recordings.len()),
                _ => vim::Grid::column(peer.map_or(0, |p| p.rows().len())),
            },
        }
    }

    /// How many cards fit across an index page.
    ///
    /// Arithmetic rather than a measurement, for the reason `menu_origin` is:
    /// iced lays out after `view` and the keyboard has to know where `l` lands
    /// before that. So this is the same division the view does, from the same
    /// window size — and the two agreeing is what keeps the cursor under the
    /// card it is drawn on.
    fn columns(&self) -> usize {
        columns_in(self.window.width)
    }

    /// The text `/` searches, for whichever pane has the cursor.
    fn labels(&self, pane: Pane) -> Vec<String> {
        let Some(peer) = &self.peer else {
            return Vec::new();
        };
        match pane {
            Pane::Sidebar => peer.choices.iter().map(|c| c.label.clone()).collect(),
            // Both halves of the row, so `/bach` finds a Bach track whether the
            // word is in the title or the composer — and on an index page the
            // cards, because `/` should search what is on screen rather than
            // what used to be.
            Pane::Tracks => match peer.source {
                Source::Albums => peer
                    .albums
                    .iter()
                    .map(|a| format!("{} {}", a.name, a.creator))
                    .collect(),
                Source::Artists => peer.artists.iter().map(|a| a.name.clone()).collect(),
                _ => peer
                    .rows()
                    .iter()
                    .map(|i| format!("{} {}", i.title, i.creator))
                    .collect(),
            },
        }
    }

    /// Do what a finished command asked for.
    ///
    /// The only place that knows both halves: `vim` produced the action from
    /// keys, and this is what the action means in a music library.
    fn act(&mut self, action: vim::Action) -> Task<Message> {
        // Whatever is over the list takes the motions while it is up — the
        // picker first, because it is the layer above. `<Space>` is
        // deliberately not one of them in either: the transport should not
        // stop working because a panel is.
        if self.menu.is_some() && self.picker.is_none() {
            match action {
                vim::Action::Move(motion) => return self.menu_travel(motion),
                vim::Action::Activate => return self.update(Message::MenuActivate),
                vim::Action::Cancel => return self.update(Message::CloseMenu),
                vim::Action::Toggle => return self.update(Message::PlayPause),
                _ => return Task::none(),
            }
        }
        if self.picker.is_some() {
            match action {
                vim::Action::Move(motion) => return self.picker_travel(motion),
                vim::Action::Activate => return self.update(Message::PickerActivate),
                vim::Action::Cancel => return self.update(Message::ClosePicker),
                vim::Action::Toggle => return self.update(Message::PlayPause),
                _ => return Task::none(),
            }
        }
        if self.devices.is_some() {
            match action {
                vim::Action::Move(motion) => return self.devices_travel(motion),
                vim::Action::Activate => return self.pick_device(),
                vim::Action::Cancel => return self.update(Message::CloseDevices),
                vim::Action::Toggle => return self.update(Message::PlayPause),
                _ => return Task::none(),
            }
        }
        match action {
            vim::Action::Move(motion) => self.travel(motion),
            vim::Action::Activate => self.activate(),
            vim::Action::Cycle => {
                self.pane = self.pane.next();
                self.reveal()
            }
            // The same in both panes, because it is not about what the
            // cursor is on: `p` used to do this and a music player's most
            // pressed key should be the biggest one on the keyboard.
            vim::Action::Toggle => self.update(Message::PlayPause),
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
                // Not a motion and not a mode: `a` on a track asks the one
                // question this client had no way to ask after the hearts
                // went, which is "which lists is this on".
                'a' => self.update(Message::OpenPicker),
                // The same menu the dots and a right click open. Asked for by
                // key it has no pointer to sit under, so it opens at the top
                // left of the list and is walked with `j` like everything
                // else — which is the whole of what "keyboard accessible"
                // means here.
                'm' => {
                    let id = self
                        .peer
                        .as_ref()
                        .and_then(|p| p.rows().get(self.at(Pane::Tracks)))
                        .map(|i| i.id);
                    match id {
                        Some(id) => {
                            self.cursor = iced::Point::new(260.0, 120.0);
                            self.update(Message::RowMenu(id))
                        }
                        None => Task::none(),
                    }
                }
                // Where the sound is. A letter rather than a motion for the
                // same reason `a` and `m` are: it asks a question about the
                // session rather than moving a cursor through a list.
                'd' => self.update(Message::OpenDevices),
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
            // Moving onto a row *is* showing it here — there is no `<Enter>`
            // in between — so the cursor walking down the sidebar changes what
            // is shown on every step. One navigation, not one per row.
            self.nav = Nav::Replace;
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
        let id = match self.pane {
            Pane::Sidebar => Self::SIDEBAR,
            Pane::Tracks => Self::TRACKS,
        };
        // The shape answers, because only it knows whether its cells are
        // stacked one per row or six.
        let y = self.shape(self.pane).progress(self.at(self.pane));
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

    /// Move inside the row menu, through the same shape everything else here
    /// uses. Four entries at most, so it is the same one-column grid a hundred
    /// rows get.
    fn menu_travel(&mut self, motion: vim::Motion) -> Task<Message> {
        let Some(menu) = &mut self.menu else {
            return Task::none();
        };
        let shape = vim::Grid::column(menu.entries().len());
        if let Some(at) = shape.step(menu.at, motion) {
            menu.at = at;
        }
        Task::none()
    }

    /// Move inside the picker, through the same shape the panes use.
    ///
    /// One column, one cell longer than the playlists, because the row that
    /// makes a new one is a row: `j` walks onto it like anything else and
    /// `<Enter>` there starts naming. One shape, one cursor, nothing extra to
    /// learn.
    fn picker_travel(&mut self, motion: vim::Motion) -> Task<Message> {
        let Some(picker) = &mut self.picker else {
            return Task::none();
        };
        let shape = vim::Grid::column(picker.lists.len() + 1);
        if let Some(at) = shape.step(picker.at, motion) {
            picker.at = at;
        }
        Task::none()
    }

    /// What `a` reads: every playlist, with the ones this track is already on
    /// marked. One query for one track, asked when the picker opens — the same
    /// trade the phone makes, and for the same reason.
    /// Walk the device picker. One more cell than there are devices, because
    /// the last row is "stop everywhere" — the same shape the playlist picker
    /// has, so `j` reaches it without a second key to learn.
    fn devices_travel(&mut self, motion: vim::Motion) -> Task<Message> {
        let Some(at) = self.devices else {
            return Task::none();
        };
        let cells = self.listening.devices().len() + 1;
        if let Some(next) = vim::Grid::column(cells).step(at, motion) {
            self.devices = Some(next);
        }
        Task::none()
    }

    /// Run the row the device cursor is on.
    ///
    /// A device that cannot be heard is not a target here either, exactly as
    /// it is not a click target: `<Enter>` on it does nothing rather than
    /// asking for a transfer the server will refuse.
    fn pick_device(&mut self) -> Task<Message> {
        let Some(at) = self.devices else {
            return Task::none();
        };
        let to = match self.listening.devices().get(at) {
            Some(device) if !device.audible => return Task::none(),
            Some(device) => Some(device.id.clone()),
            None => None,
        };
        self.update(Message::PickDevice(to))
    }

    fn open_picker(&mut self) -> Task<Message> {
        // Whatever the menu was opened for, or the track under the cursor.
        // The menu stays up: this is its submenu, and a submenu that closes
        // its parent is a second menu wearing the name.
        let wanted = self.menu.as_ref().map(|m| m.media);
        let origin = self.menu.as_ref().map(|m| {
            // Beside the parent, overlapping its border by a hair so the two
            // read as one panel rather than as two that happen to touch.
            Self::fit(
                iced::Point::new(m.origin.x + Self::MENU_WIDTH - 2.0, m.origin.y),
                self.window,
                Self::PICKER_WIDTH,
                Self::PICKER_MAX_HEIGHT,
            )
        });
        let at = self.at(Pane::Tracks);
        let Some(peer) = &mut self.peer else {
            return Task::none();
        };
        let found = match wanted {
            Some(id) => peer.rows().iter().find(|i| i.id == id).cloned(),
            None => peer.rows().get(at).cloned(),
        };
        let Some(item) = found else {
            return Task::none();
        };
        let mut store = peer.client.store();
        let on: std::collections::BTreeSet<_> = harken::playlists_of(&mut store, item.id)
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.id)
            .collect();
        let lists = harken::playlists(&mut store)
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.id, p.name, on.contains(&p.id)))
            .collect();
        drop(store);
        self.picker = Some(Picker {
            media: item.id,
            title: item.title,
            lists,
            at: 0,
            origin,
            naming: None,
        });
        Task::none()
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
            // On an index page the cursor is on a card, and a card is a place
            // rather than a track: `<Enter>` opens it, which is the same thing
            // clicking it does.
            Pane::Tracks => {
                if let Some(source) = self.card_under_cursor(at) {
                    return self.update(Message::Select(source));
                }
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

    /// A cover's URL, from what the log carries.
    ///
    /// `album.art` is spelt exactly as `media.file` is, so this is the same
    /// join the player already does — which is the point of spelling it that
    /// way rather than inventing a second rule for pictures.
    fn art_url(&self, art: &str) -> String {
        if art.is_empty() {
            return String::new();
        }
        media_url(&self.server, art)
    }

    /// Ask for every cover the two index pages would draw.
    ///
    /// All of them rather than the ones on screen, because scrolling does not
    /// go through `update` and a cover that only starts loading once it is
    /// visible is a cover that is never there when you look at it. `want` is
    /// idempotent, so this is cheap to call whenever the library moves — the
    /// second call for a URL returns `None` and nothing goes out.
    fn want_covers(&mut self) -> Task<Message> {
        let Some(peer) = &self.peer else {
            return Task::none();
        };
        let urls: Vec<String> = peer
            .albums
            .iter()
            .map(|a| a.art.clone())
            .chain(peer.artists.iter().map(|a| a.art.clone()))
            .filter(|art| !art.is_empty())
            .map(|art| media_url(&self.server, &art))
            .collect();
        let tasks: Vec<Task<Message>> = urls
            .into_iter()
            .filter_map(|url| self.covers.want(url))
            .map(|task| task.map(Message::Cover))
            .collect();
        Task::batch(tasks)
    }

    /// The album or artist card at this position, if the page has cards.
    ///
    /// `None` on every other page, which is what lets one `<Enter>` mean "open
    /// this" here and "play this" everywhere else without a mode.
    fn card_under_cursor(&self, at: usize) -> Option<Source> {
        let peer = self.peer.as_ref()?;
        match peer.source {
            Source::Albums => peer.albums.get(at).map(|a| Source::Album(a.name.clone())),
            Source::Artists => peer.artists.get(at).map(|a| Source::Artist(a.name.clone())),
            Source::Composers => peer
                .composers
                .get(at)
                .map(|c| Source::Works(c.name.clone())),
            Source::Works(_) => peer
                .works
                .get(at)
                .map(|w| Source::Work(w.id.clone(), w.title.clone())),
            // Not a card, and it opens all the same: a work page is a list of
            // recordings and `<Enter>` on one has to mean the obvious thing.
            // Which is why this is `card_under_cursor` in name only — what it
            // answers is "what does the cursor open", and a page where the
            // answer is `None` is a page where `<Enter>` plays.
            Source::Work(..) => peer
                .recordings
                .get(at)
                .map(|r| Source::Recording(r.id.clone(), r.performers.clone())),
            _ => None,
        }
    }

    /// Move through the queue, and stop at either end rather than wrapping —
    /// a list that loops silently is hard to tell from one that is stuck.
    fn skip(&mut self, delta: i32) {
        let next = self.playing_at() as i32 + delta;
        if self.player.track().is_none() || next < 0 || next as usize >= self.queue.len() {
            return;
        }
        self.start_at(next as usize, 0, true);
    }

    /// Where in the queue the player is. Zero when nothing is playing, which
    /// is also what a report of an empty session should say.
    fn playing_at(&self) -> usize {
        let Some(id) = self.player.track().map(|t| t.id) else {
            return 0;
        };
        self.queue.iter().position(|t| t.id == id).unwrap_or(0)
    }

    /// Put this device's player on `at` in the queue.
    ///
    /// The one place a [`listening::Track`] becomes something that makes a
    /// sound, so a hand-off and a click on a row land in the same code: both
    /// are a queue, a place in it, a point in the track and whether it was
    /// playing.
    ///
    /// The seek is issued before the element has read any of the stream. A
    /// browser queues it against `loadedmetadata` rather than refusing it, so
    /// a track handed over mid-way starts where it left off — and the worst
    /// case is a second of the beginning, not a seek that goes nowhere.
    fn start_at(&mut self, at: usize, position_ms: i64, playing: bool) {
        let Some(track) = self.queue.get(at).cloned() else {
            return;
        };
        self.player.play(
            Track {
                id: track.id,
                title: track.title.clone(),
                creator: track.creator.clone(),
                album: track.album.clone(),
                ms: track.duration_ms,
            },
            &media_url(&self.server, &track.file),
        );
        if position_ms > 0 {
            self.player.seek(position_ms as f64 / 1000.0);
        }
        if !playing {
            self.player.pause();
        }
        if track.file.is_empty() {
            self.note = "nothing to stream — this one has no file".into();
        }
    }

    /// Where a transport button goes, and the only place that is decided.
    ///
    /// If the sound is on another of this account's devices, the button is a
    /// *message*: pressing pause here pauses the laptop. Otherwise it is an
    /// instruction — either this device is the output, or nothing is, and the
    /// report that follows the tick claims it. That third case is why this is
    /// not `if outputs_here()`: a session with no output yet is the common one
    /// at the start of a day, and needing a device picked before any music can
    /// start is a setup step.
    fn ask(&mut self, command: listening::Command) -> Task<Message> {
        if self.listening.elsewhere() {
            self.listening.ask(command);
            return Task::none();
        }
        self.obey(command)
    }

    /// Do it here. Reached from [`App::ask`] and from the server, which only
    /// ever sends a command to the device that is the output.
    fn obey(&mut self, command: listening::Command) -> Task<Message> {
        match command {
            listening::Command::Play => self.player.resume(),
            listening::Command::Pause => self.player.pause(),
            listening::Command::Next => self.skip(1),
            listening::Command::Previous => self.skip(-1),
            listening::Command::Seek { position_ms } => {
                self.player.seek(position_ms as f64 / 1000.0)
            }
            listening::Command::Start {
                queue,
                at,
                position_ms,
                playing,
            } => {
                self.queue = queue;
                self.start_at(at, position_ms, playing);
            }
        }
        Task::none()
    }

    /// Do the thing, then make the address bar agree with what is on screen.
    ///
    /// The agreeing is *here* and not in the places that change what is shown,
    /// which is the whole architecture: there are four of them — a click on
    /// the sidebar, `j` in it, the row menu's "Go to", and the back button
    /// itself — and the first version pushed the route from one of them. The
    /// one it missed was the sidebar's own cursor, which is the way this
    /// window is actually driven. A rule that every call site has to remember
    /// is a rule that is already broken; this one cannot be missed, because
    /// nothing has to remember it.
    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.step(message);
        self.sync_route();
        // Covers follow the *lists*, not the wire. `want_covers` is idempotent
        // but it walks every album and artist to find that out, and this runs
        // twenty times a second — so a generation the sidebar bumps decides,
        // and the usual answer is that nothing happened.
        Task::batch([task, self.want_covers_if_moved()])
    }

    /// Ask for covers when the library has been rebuilt since the last ask.
    fn want_covers_if_moved(&mut self) -> Task<Message> {
        let gen = match &self.peer {
            Some(peer) => peer.art_gen,
            None => return Task::none(),
        };
        if gen == self.art_seen {
            return Task::none();
        }
        self.art_seen = gen;
        self.want_covers()
    }

    /// Where the address bar is made to agree. Nothing else writes it.
    fn sync_route(&mut self) {
        let Some(peer) = &self.peer else {
            return;
        };
        let want = peer.source.route();
        if self.routed.as_ref() == Some(&want) {
            return;
        }
        // Skipped when the bar already says it, which is exactly the case
        // where the route *came* from the back button: writing there would
        // make one place two history entries.
        if route::read().as_ref() != Some(&want) {
            // The sidebar's cursor is a scrub, not forty places visited.
            // Holding `j` through forty albums should leave the back button
            // where it was, and leave the URL right the whole way down.
            route::write(&want, self.nav == Nav::Push);
        }
        self.routed = Some(want);
        self.nav = Nav::Push;
    }

    fn step(&mut self, message: Message) -> Task<Message> {
        // An entry was picked: the menu has said everything it had to say.
        // `OpenPicker` is not here because it reads the menu on its way out.
        if matches!(message, Message::PlayItem(_) | Message::Select(_)) {
            self.menu = None;
        }
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
                // A device is a login, so signing out is this device leaving
                // the session rather than going quiet inside it — the picker
                // on everything else should stop offering it.
                self.listening.close();
                self.devices = None;
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
            Message::Resized(size) => {
                self.window = size;
                Ok(())
            }
            Message::HoverAt(at) => {
                // Takes the keyboard as well as the highlight, because the
                // cursor is only *drawn* in the pane that has it — a hover
                // that moved an undrawn cursor would look like nothing
                // happening, and then the next `j` would jump from wherever
                // the mouse had been.
                self.pane = Pane::Tracks;
                self.cursors[Pane::Tracks as usize] = at;
                Ok(())
            }
            Message::Cover(done) => {
                self.covers.loaded(done);
                Ok(())
            }
            Message::Hover(at) => {
                self.cursor = at;
                Ok(())
            }
            Message::MenuAt(at) => {
                if let Some(menu) = &mut self.menu {
                    menu.at = at.min(menu.entries().len().saturating_sub(1));
                }
                Ok(())
            }
            Message::MenuActivate => {
                let picked = self
                    .menu
                    .as_ref()
                    .and_then(|m| m.entries().get(m.at).map(|(_, msg)| msg.clone()));
                return match picked {
                    Some(msg) => self.update(msg),
                    None => Task::none(),
                };
            }
            Message::CloseMenu => {
                self.menu = None;
                Ok(())
            }
            Message::RowMenu(id) => {
                if let Some(peer) = &self.peer {
                    if let Some(item) = peer.rows().iter().find(|i| i.id == id) {
                        let album = peer.detail_of(id).album;
                        let artist = item.creator.clone();
                        // Two entries always, and one more for each of the
                        // album and the artist when the track has one — which
                        // is what decides how tall it is, and so which way it
                        // has room to open.
                        let entries =
                            2 + usize::from(!album.is_empty()) + usize::from(!artist.is_empty());
                        self.menu = Some(RowMenu {
                            media: id,
                            title: item.title.clone(),
                            album,
                            artist,
                            origin: Self::menu_origin(self.cursor, self.window, entries),
                            at: 0,
                        });
                    }
                }
                Ok(())
            }
            Message::OpenPicker => return self.open_picker(),
            Message::ClosePicker => {
                self.picker = None;
                Ok(())
            }
            Message::PickerAt(at) => {
                if let Some(picker) = &mut self.picker {
                    picker.at = at.min(picker.lists.len());
                }
                Ok(())
            }
            Message::PickerName(name) => {
                if let Some(picker) = &mut self.picker {
                    picker.naming = Some(name);
                }
                Ok(())
            }
            Message::PickerActivate => {
                // The last row makes a playlist; every other row toggles one.
                let Some(picker) = &mut self.picker else {
                    return Task::none();
                };
                let Some(&(list, _, on)) = picker.lists.get(picker.at) else {
                    picker.naming = Some(String::new());
                    // Put the keyboard in the box rather than making somebody
                    // reach for the mouse to finish what a key started.
                    return iced::widget::operation::focus(Self::NAMING);
                };
                let media = picker.media;
                // Marked here rather than by re-reading: the answer is known
                // and a query per tap is a query per tap.
                picker.lists[picker.at].2 = !on;
                match &mut self.peer {
                    Some(peer) => {
                        let m = if on {
                            mutators::remove_from_playlist(list, media)
                        } else {
                            mutators::add_to_playlist(list, media)
                        };
                        peer.client.mutate(m).map(|_| ()).map_err(|e| e.to_string())
                    }
                    None => Ok(()),
                }
            }
            Message::PickerCreate => {
                let name = match &self.picker {
                    Some(p) => p.naming.clone().unwrap_or_default(),
                    None => return Task::none(),
                };
                let made = match &mut self.peer {
                    Some(peer) => peer
                        .client
                        .mutate(mutators::create_playlist(name))
                        .map(|_| ())
                        .map_err(|e: petros::Error| e.to_string()),
                    None => Ok(()),
                };
                // Re-read rather than guessing the new row: `create_playlist`
                // chooses the id inside `apply`, so the only way to know it is
                // to ask. The track is not added to it here for the same
                // reason — one tap away, on a row that now exists.
                if made.is_ok() {
                    if let Some(peer) = &mut self.peer {
                        peer.refresh();
                    }
                    let at = self.picker.as_ref().map(|p| p.at);
                    // It only reads, so the task it hands back is empty.
                    let _ = self.open_picker();
                    if let (Some(picker), Some(at)) = (&mut self.picker, at) {
                        picker.at = at;
                    }
                }
                made
            }
            // Play and pause are two verbs rather than one toggle, and the
            // moment the sound might be on another device that stops being a
            // detail: "the other one of whatever you are" is not something a
            // phone can mean about a laptop. So which it is gets decided here,
            // against whichever of the two is actually making the sound.
            Message::PlayPause => {
                let playing = if self.listening.elsewhere() {
                    self.listening.playing()
                } else {
                    self.player.is_playing()
                };
                return self.ask(if playing {
                    listening::Command::Pause
                } else {
                    listening::Command::Play
                });
            }
            Message::Seek(secs) => {
                return self.ask(listening::Command::Seek {
                    position_ms: (secs as f64 * 1000.0) as i64,
                })
            }
            Message::Skip(delta) => {
                return self.ask(if delta > 0 {
                    listening::Command::Next
                } else {
                    listening::Command::Previous
                })
            }
            // The queue is what is on screen, taken now: skipping follows the
            // list you pressed play in, even after the sidebar moves somewhere
            // else. It is also what a hand-off carries, which is why the rows
            // are copied rather than referred to — the device that receives
            // them may not have that album in its replica yet.
            Message::PlayItem(id) => {
                let Some(peer) = &self.peer else {
                    return Task::none();
                };
                self.queue = peer
                    .rows()
                    .iter()
                    .map(|i| listening::Track {
                        id: i.id,
                        title: i.title.clone(),
                        creator: i.creator.clone(),
                        album: peer.detail_of(i.id).album,
                        duration_ms: i.duration_ms,
                        // The path as the log carries it. Each device joins it
                        // to *its* server, which is what `media_url` is for.
                        file: i.file.clone(),
                    })
                    .collect();
                let at = self.queue.iter().position(|t| t.id == id).unwrap_or(0);
                let queue = self.queue.clone();
                return self.ask(listening::Command::Start {
                    queue,
                    at,
                    position_ms: 0,
                    playing: true,
                });
            }
            Message::OpenDevices => {
                // Where the cursor starts is the device that has the sound,
                // so `<Enter>` straight away changes nothing.
                let output = self.listening.session().and_then(|s| s.output.clone());
                let at = output
                    .and_then(|id| self.listening.devices().iter().position(|d| d.id == id))
                    .unwrap_or(self.listening.devices().len());
                self.devices = Some(at);
                Ok(())
            }
            Message::DeviceAt(at) => {
                self.devices = Some(at);
                Ok(())
            }
            Message::PickDevice(to) => {
                self.listening.transfer(to);
                self.devices = None;
                Ok(())
            }
            Message::CloseDevices => {
                self.devices = None;
                Ok(())
            }
            // The element runs on its own clock, so the end of a track arrives
            // as "the tick that noticed" rather than as an event. Twenty times
            // a second is plenty to move on by.
            Message::Tick => {
                if self.player.ended() {
                    self.skip(1);
                }
                // The listening session: first what this device has been told
                // to do — the server only ever tells the output — and then
                // what it is doing.
                for command in self.listening.poll() {
                    let _ = self.obey(command);
                }
                // Silent unless it is the output. One rule, enforced here
                // rather than remembered at each button, because losing the
                // sound is not something this device does: it is told, by a
                // broadcast, and this is the only place that can notice.
                if self.listening.elsewhere() && self.player.is_playing() {
                    self.player.pause();
                }
                // A device that has never played anything says nothing, so
                // opening a second tab does not quietly claim the sound from
                // the one that is using it.
                if self.player.track().is_some() {
                    let at = self.playing_at();
                    let playing = self.player.is_playing();
                    let position_ms = (self.player.position() * 1000.0) as i64;
                    let queue = std::mem::take(&mut self.queue);
                    self.listening.report(&queue, at, playing, position_ms);
                    self.queue = queue;
                }
                // The back button, and a link somebody was sent. Read rather
                // than listened for: `popstate` would mean a closure kept
                // alive for the life of the page publishing into a channel,
                // and this tick already runs for the transport. A route is
                // only consumed once the sidebar has something to resolve it
                // against, so a deep link that arrives before the database
                // has opened is still waiting when it does.
                let fragment = route::read();
                let ready = self
                    .peer
                    .as_ref()
                    .is_some_and(|peer| !peer.choices.is_empty());
                if ready && fragment != self.routed {
                    self.routed = fragment.clone();
                    if let (Some(route), Some(peer)) = (fragment, &mut self.peer) {
                        let wanted = peer.source_of(&route);
                        if wanted != peer.source {
                            return self.update(Message::Select(wanted));
                        }
                    }
                }
                // A lock-screen button leaves a note rather than calling in,
                // because its handler runs on the browser's stack and the
                // state it wants to move is behind this loop. The tick that
                // already watches for the end of a track collects it.
                #[cfg(target_arch = "wasm32")]
                {
                    let remote = self.player.take_remote();
                    self.player.announce();
                    // A media key is a transport button like any other, so it
                    // goes where they go: to whichever device is making the
                    // sound. Which is also why the controller this page put on
                    // the lock screen is still worth having after the sound
                    // moves — it is a remote for the session, not for the tab.
                    let command = match remote {
                        Some(Remote::Play) => Some(listening::Command::Play),
                        Some(Remote::Pause) => Some(listening::Command::Pause),
                        Some(Remote::Next) => Some(listening::Command::Next),
                        Some(Remote::Previous) => Some(listening::Command::Previous),
                        Some(Remote::Seek(secs)) => Some(listening::Command::Seek {
                            position_ms: (secs * 1000.0) as i64,
                        }),
                        None => None,
                    };
                    if let Some(command) = command {
                        return self.ask(command);
                    }
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
                        }
                        // Always, not only when the sidebar moved: an album
                        // opened from a card is not a sidebar line, and
                        // landing on row seventeen of a record you just opened
                        // is a cursor that remembers the wrong list.
                        self.cursors[Pane::Tracks as usize] = 0;
                        Ok(())
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
            // Nothing this client does writes to the library any more — the
            // scanner authors it and the playlists are read here — so the
            // list moves only when something arrives from the server.
            if arrived {
                peer.refresh();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let Some(peer) = &self.peer else {
            return self.view_signed_out();
        };

        let base = container(
            column![
                // The sidebar and the list share the height that is left once
                // the bar has taken its own — so the bar stays at the bottom
                // however long the list is, rather than being pushed off it.
                row![
                    self.view_sidebar(peer),
                    rule::vertical(1),
                    self.view_page(peer),
                ]
                .spacing(16)
                .height(Length::Fill),
                rule::horizontal(1),
                self.view_bar(),
            ]
            .spacing(12),
        )
        .padding(16)
        // The window's own background is the one thing `palette::of` in a
        // style closure cannot reach: with no theme of our own, iced paints
        // the page from *its* Dark, and every row that draws no background
        // shows it through. So the page is painted here, and the near-black
        // in `branding/` is what you actually see.
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(
                palette::of(theme).background.base.color,
            )),
            text_color: Some(palette::of(theme).background.base.text),
            ..container::Style::default()
        });

        // Everything above the page is a layer of one stack, and the pointer
        // is tracked on the root so that `pin` below shares its origin.
        let mut layers = stack![mouse_area(base).on_move(Message::Hover)];

        if let Some(menu) = &self.menu {
            // A backdrop under it, because a menu that only closes on the key
            // that opened it is a menu people click around.
            layers = layers
                .push(
                    mouse_area(container(text("")).width(Length::Fill).height(Length::Fill))
                        .on_press(Message::CloseMenu)
                        .on_right_press(Message::CloseMenu),
                )
                .push(pin(Self::view_menu(menu)).x(menu.origin.x).y(menu.origin.y));
        }

        if let Some(picker) = &self.picker {
            // Dimmed only when it is centred. A submenu that dims its parent
            // has hidden the thing it is a submenu of.
            let dimmed = picker.origin.is_none();
            // One panel, two places. Opened from the row menu it is that
            // menu's submenu and sits beside it, with the parent still up;
            // opened with `a` it has no parent and no pointer, so it is
            // centred and the page behind it is dimmed. The *component* is the
            // same either way, which is the point — two ways to reach one
            // question should not be two panels to keep in step.
            let backdrop = mouse_area(
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |theme: &iced::Theme| container::Style {
                        background: dimmed.then(|| {
                            iced::Background::Color(
                                palette::of(theme).background.base.color.scale_alpha(0.72),
                            )
                        }),
                        ..container::Style::default()
                    }),
            )
            .on_press(Message::ClosePicker);
            layers = layers.push(backdrop).push(match picker.origin {
                Some(at) => Element::from(pin(Self::view_picker(picker)).x(at.x).y(at.y)),
                None => Element::from(
                    container(Self::view_picker(picker))
                        .center_x(Length::Fill)
                        .center_y(Length::Fill),
                ),
            });
        }

        if let Some(at) = self.devices {
            // Placed by the same rule the row menu is, which is the point of
            // `fit` being a function: this one is always asked for from the
            // bottom of the window, so it always opens upwards — and it does
            // that because of where it was asked from rather than because it
            // was told to.
            let rows = self.listening.devices().len() + 1;
            let origin = Self::fit(
                self.cursor,
                self.window,
                DEVICES_WIDTH,
                8.0 + 27.0 + 27.0 * rows as f32,
            );
            layers = layers
                .push(
                    mouse_area(container(text("")).width(Length::Fill).height(Length::Fill))
                        .on_press(Message::CloseDevices)
                        .on_right_press(Message::CloseDevices),
                )
                .push(pin(self.view_devices(at)).x(origin.x).y(origin.y));
        }

        layers.into()
    }

    /// Where to pin a menu asked for at `cursor`.
    ///
    /// A menu opened near the bottom of the window would otherwise run off it
    /// — and `pin` clips rather than scrolls, so the entries nearest the edge
    /// would simply not be there. Down and to the right when there is room,
    /// and back the other way when there is not, which is what every menu on
    /// every desktop does and nobody notices until it does not.
    ///
    /// The size is computed rather than measured: iced lays out after `view`
    /// and this has to decide before it. Both numbers are the panel's own —
    /// `MENU_WIDTH` is what `view_menu` sets, and the height is its padding,
    /// its title line and `entries` rows of text.
    fn menu_origin(cursor: iced::Point, window: iced::Size, entries: usize) -> iced::Point {
        const TITLE: f32 = 27.0;
        const ENTRY: f32 = 27.0;
        const PADDING: f32 = 8.0;
        Self::fit(
            cursor,
            window,
            Self::MENU_WIDTH,
            PADDING + TITLE + ENTRY * entries as f32,
        )
    }

    /// Put a panel of that size at `at`, or back the other way when it would
    /// not fit. The one rule both the menu and its submenu follow.
    fn fit(at: iced::Point, window: iced::Size, w: f32, h: f32) -> iced::Point {
        // A margin, so a panel that only just fits does not sit flush against
        // the glass.
        let edge = 8.0;
        let x = if at.x + w + edge > window.width {
            (at.x - w).max(edge)
        } else {
            at.x
        };
        let y = if at.y + h + edge > window.height {
            (at.y - h).max(edge)
        } else {
            at.y
        };
        iced::Point::new(x, y)
    }

    /// A row's menu: what to do with the track it was opened on.
    ///
    /// Four things, and each is something this window could already do — the
    /// menu is a way to ask for them *about a row you are pointing at*, which
    /// is the one thing the keyboard could not express.
    fn view_menu(menu: &RowMenu) -> Element<'_, Message> {
        let rows = menu.entries().into_iter().enumerate().fold(
            column![].spacing(0),
            |col, (i, (label, _))| {
                let on_cursor = menu.at == i;
                col.push(
                    mouse_area(
                        container(text(middle(&label, 26)).size(13).style(
                            move |theme: &iced::Theme| text::Style {
                                color: Some(if on_cursor {
                                    palette::of(theme).primary.base.text
                                } else {
                                    palette::of(theme).background.base.text
                                }),
                            },
                        ))
                        .width(Length::Fixed(190.0))
                        .padding([5, 10])
                        .style(move |theme: &iced::Theme| container::Style {
                            background: on_cursor.then(|| {
                                iced::Background::Color(palette::of(theme).primary.base.color)
                            }),
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..iced::Border::default()
                            },
                            ..container::Style::default()
                        }),
                    )
                    // Moving onto a row and running it are the same two
                    // messages a click is: the pointer lands the cursor where
                    // the keyboard would have walked it, so whichever you used
                    // last, the other carries on from there.
                    .on_press(Message::MenuAt(i))
                    .on_release(Message::MenuActivate),
                )
            },
        );
        container(
            column![
                // Which track this is about. A menu opened by a right click can
                // land a row away from where the eye was, and a menu that does not
                // say what it is for is a menu you close to check.
                container(text(middle(&menu.title, 26)).size(11).style(style::dim))
                    .padding([4, 10]),
                rows,
            ]
            .spacing(0),
        )
        .padding(4)
        .style(|theme: &iced::Theme| {
            let palette = palette::of(theme);
            container::Style {
                background: Some(iced::Background::Color(palette.background.weak.color)),
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            }
        })
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
                        |theme: &iced::Theme| text::Style {
                            color: Some(palette::of(theme).background.base.text.scale_alpha(0.5)),
                        },
                    ))
                    .padding([8, 10]),
                );
            }
            under = heading;

            // The sidebar has one highlight, not two: its cursor *is* what the
            // table is showing, so a separate "selected" color would be a
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
                                        palette::of(theme).primary.base.text
                                    } else {
                                        palette::of(theme).background.base.text
                                    }),
                                }),
                            text(choice.count.map(|n| n.to_string()).unwrap_or_default())
                                .size(10)
                                .style(move |theme: &iced::Theme| {
                                    let palette = palette::of(theme);
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

        container(scrollable(side).id(Self::SIDEBAR).style(style::bars))
            .width(Length::Fixed(Self::SIDEBAR_WIDTH))
            .height(Length::Fill)
            .into()
    }

    /// What the sidebar picked.
    ///
    /// A listening UI and nothing else: the library is what the scanner
    /// found, and this reads it. The one thing the demo drops is signing in,
    /// because it has no accounts and no server to sign in to — compiled out
    /// rather than deleted, since the real client's is the only way in.
    /// Which page the sidebar's selection is, which is four shapes now rather
    /// than one.
    ///
    /// The table was the whole of the right-hand side while every source was a
    /// list of tracks. An album is not only a list of tracks, though — it is a
    /// *thing*, with a cover and a performer and a length — and a page that
    /// opens on the first row of a table says none of that. So a record gets a
    /// header above its table, and the two indexes get a grid of cards, and a
    /// playlist and the library stay exactly what they were.
    fn view_page(&self, peer: &'_ Peer) -> Element<'_, Message> {
        match &peer.source {
            Source::Albums => self.view_cards(
                peer.albums
                    .iter()
                    .map(|a| Card {
                        seed: a.name.clone(),
                        title: a.name.clone(),
                        under: a.creator.clone(),
                        tally: a.tracks,
                        open: Source::Album(a.name.clone()),
                        round: false,
                        art: a.art.clone(),
                    })
                    .collect(),
                "No albums yet.",
            ),
            Source::Artists => self.view_cards(
                peer.artists
                    .iter()
                    .map(|a| Card {
                        seed: a.name.clone(),
                        title: a.name.clone(),
                        under: String::new(),
                        tally: a.tracks,
                        open: Source::Artist(a.name.clone()),
                        // A person is a circle and a record is a square, which
                        // is the one thing every music app agrees about and
                        // the only thing telling the two grids apart at a
                        // glance.
                        round: true,
                        art: a.art.clone(),
                    })
                    .collect(),
                "Nobody yet.",
            ),
            Source::Album(name) => {
                let album = peer.albums.iter().find(|a| a.name == *name);
                let header = self.view_record(
                    name,
                    album.map(|a| a.creator.as_str()).unwrap_or_default(),
                    peer.source.kind(),
                    peer.rows(),
                    None,
                    album.map(|a| a.art.as_str()).unwrap_or_default(),
                );
                column![header, self.view_list(peer)]
                    .spacing(0)
                    .width(Length::Fill)
                    .into()
            }
            // Everyone this library has a work by. Circles, because these are
            // people and a person is a circle wherever one is drawn.
            Source::Composers => self.view_cards(
                peer.composers
                    .iter()
                    .map(|c| Card {
                        seed: c.name.clone(),
                        title: c.name.clone(),
                        // The years, which is what makes a composer index
                        // readable at a glance — and the tally when nobody has
                        // said, rather than a dash where a date should be.
                        under: lifespan(c.born, c.died),
                        tally: c.works,
                        open: Source::Works(c.name.clone()),
                        round: true,
                        art: c.art.clone(),
                    })
                    .collect(),
                "Nobody yet.",
            ),
            // One composer's works.
            Source::Works(name) => {
                let composer = peer.composers.iter().find(|c| c.name == *name);
                let works = peer.works.len();
                let tracks: i64 = peer.works.iter().map(|w| w.tracks).sum();
                let facts = format!(
                    "{works} {}  ·  {tracks} {}",
                    plural(works as i64, "work"),
                    plural(tracks, "track")
                );
                let header = self.view_header(
                    name,
                    &lifespan(
                        composer.map(|c| c.born).unwrap_or_default(),
                        composer.map(|c| c.died).unwrap_or_default(),
                    ),
                    peer.source.kind(),
                    &facts,
                    composer.map(|c| c.art.as_str()).unwrap_or_default(),
                    true,
                );
                let cards = peer
                    .works
                    .iter()
                    .map(|w| Card {
                        seed: w.title.clone(),
                        title: w.title.clone(),
                        // The catalogue number, which is the one name a work
                        // has that survives translation — and the thing that
                        // tells two "Ballades" apart.
                        under: w.catalogue.clone(),
                        tally: w.tracks,
                        open: Source::Work(w.id.clone(), w.title.clone()),
                        round: false,
                        art: w.art.clone(),
                    })
                    .collect();
                column![header, self.view_cards(cards, "Nothing by them yet.")]
                    .spacing(0)
                    .width(Length::Fill)
                    .into()
            }
            // One work, and every performance of it this library holds. The
            // page this whole shape exists for: the rows are the same music
            // and what tells them apart is who played it.
            Source::Work(id, title) => {
                let work = peer.works.iter().find(|w| w.id == *id);
                let takes = peer.recordings.len();
                let mut facts = match work.map(|w| w.catalogue.clone()).unwrap_or_default() {
                    c if c.is_empty() => String::new(),
                    c => format!("{c}  ·  "),
                };
                facts.push_str(&format!("{takes} {}", plural(takes as i64, "recording")));
                if let Some(w) = work {
                    for fact in [&w.form, &w.period] {
                        if !fact.is_empty() {
                            facts.push_str("  ·  ");
                            facts.push_str(fact);
                        }
                    }
                }
                let header = self.view_header(
                    title,
                    work.map(|w| w.composer.as_str()).unwrap_or_default(),
                    peer.source.kind(),
                    &facts,
                    work.map(|w| w.art.as_str()).unwrap_or_default(),
                    false,
                );
                column![header, self.view_takes(peer)]
                    .spacing(0)
                    .width(Length::Fill)
                    .into()
            }
            // One performance, in the order the work goes.
            Source::Recording(id, who) => {
                let take = peer.recordings.iter().find(|r| r.id == *id);
                let rows = peer.rows();
                let ms: i64 = rows.iter().map(|i| i.duration_ms).sum();
                let mut facts = format!("{} {}", rows.len(), plural(rows.len() as i64, "track"));
                if ms > 0 {
                    facts.push_str("  ·  ");
                    facts.push_str(&spell(ms));
                }
                if let Some(r) = take {
                    for fact in [
                        (r.recorded > 0).then(|| r.recorded.to_string()),
                        (!r.label.is_empty()).then(|| r.label.clone()),
                        (!r.licence.is_empty()).then(|| r.licence.clone()),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        facts.push_str("  ·  ");
                        facts.push_str(&fact);
                    }
                }
                let header = self.view_header(
                    who,
                    "",
                    peer.source.kind(),
                    &facts,
                    take.map(|r| r.art.as_str()).unwrap_or_default(),
                    false,
                );
                column![header, self.view_list(peer)]
                    .spacing(0)
                    .width(Length::Fill)
                    .into()
            }
            Source::Artist(name) => {
                let by = peer.albums_by(name);
                let art = peer
                    .artists
                    .iter()
                    .find(|a| a.name == *name)
                    .map(|a| a.art.as_str())
                    .unwrap_or_default();
                let header = self.view_record(
                    name,
                    "",
                    peer.source.kind(),
                    peer.rows(),
                    Some(by.len()),
                    art,
                );
                column![header, self.view_list(peer)]
                    .spacing(0)
                    .width(Length::Fill)
                    .into()
            }
            _ => self.view_list(peer),
        }
    }

    /// The recordings of one work, as rows.
    ///
    /// Rows rather than cards, and that is the one place this page departs
    /// from the two index pages: every recording of a work has the same
    /// picture and the same title, so a grid of them would be a grid of
    /// identical squares. What tells them apart is text — who played it, when,
    /// on what terms — so text is what the page is.
    fn view_takes(&self, peer: &'_ Peer) -> Element<'_, Message> {
        if peer.recordings.is_empty() {
            return container(text("No recordings of it yet.").size(13).style(
                |theme: &iced::Theme| text::Style {
                    color: Some(palette::of(theme).background.base.text.scale_alpha(0.5)),
                },
            ))
            .padding(Self::PAGE_PADDING)
            .into();
        }
        let focused = self.pane == Pane::Tracks;
        let cursor = focused.then_some(self.at(Pane::Tracks));
        let mut list = column![];
        for (at, take) in peer.recordings.iter().enumerate() {
            let on_cursor = Some(at) == cursor;
            let who = match take.performers.is_empty() {
                // A recording nobody is credited on, which the demo has: the
                // honest answer, rather than a guess at who played it.
                true => "Performer not named".to_string(),
                false => take.performers.clone(),
            };
            let mut facts = format!("{} {}", take.tracks, plural(take.tracks, "track"));
            for fact in [
                (take.recorded > 0).then(|| take.recorded.to_string()),
                (!take.label.is_empty()).then(|| take.label.clone()),
                (!take.licence.is_empty()).then(|| take.licence.clone()),
            ]
            .into_iter()
            .flatten()
            {
                facts.push_str("  ·  ");
                facts.push_str(&fact);
            }
            list = list.push(
                mouse_area(
                    container(
                        column![
                            text(who).size(13).style(move |theme: &iced::Theme| {
                                text::Style {
                                    color: Some(if on_cursor {
                                        palette::of(theme).primary.base.text
                                    } else {
                                        palette::of(theme).background.base.text
                                    }),
                                }
                            }),
                            text(facts).size(11).style(move |theme: &iced::Theme| {
                                let palette = palette::of(theme);
                                text::Style {
                                    color: Some(match on_cursor {
                                        true => palette.primary.base.text.scale_alpha(0.7),
                                        false => palette.background.base.text.scale_alpha(0.5),
                                    }),
                                }
                            }),
                        ]
                        .spacing(2),
                    )
                    .padding(iced::Padding {
                        top: 8.0,
                        right: Self::PAGE_PADDING,
                        bottom: 8.0,
                        left: Self::PAGE_PADDING,
                    })
                    .width(Length::Fill)
                    .style(move |theme: &iced::Theme| {
                        row_style(theme, on_cursor, focused, at % 2 == 1)
                    }),
                )
                .on_enter(Message::HoverAt(at))
                .on_press(Message::Select(Source::Recording(
                    take.id.clone(),
                    take.performers.clone(),
                ))),
            );
        }
        container(scrollable(list).id(Self::TRACKS))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// One index page: a grid of cards, and the cursor on one of them.
    ///
    /// Laid out as rows of `columns()` rather than by a wrapping widget,
    /// because the keyboard has to agree about where `l` lands and iced lays
    /// out after `view` — the same reason `menu_origin` does arithmetic. So
    /// both halves divide the same width by the same card, once, from
    /// `App::columns`.
    fn view_cards(&self, cards: Vec<Card>, empty: &'static str) -> Element<'_, Message> {
        if cards.is_empty() {
            return container(
                text(empty)
                    .size(13)
                    .style(|theme: &iced::Theme| text::Style {
                        color: Some(palette::of(theme).background.base.text.scale_alpha(0.5)),
                    }),
            )
            .padding(Self::PAGE_PADDING)
            .into();
        }
        let focused = self.pane == Pane::Tracks;
        let cursor = focused.then_some(self.at(Pane::Tracks));
        let columns = self.columns();

        let mut page = column![].spacing(Self::CARD_GAP);
        for (r, chunk) in cards.chunks(columns).enumerate() {
            // Top-aligned, because a title that wrapped to two lines makes
            // its card taller and centring would then float the short ones.
            let mut line = row![]
                .spacing(Self::CARD_GAP)
                .align_y(iced::Alignment::Start);
            for (c, card) in chunk.iter().enumerate() {
                let at = r * columns + c;
                line = line.push(self.view_card(card, at, Some(at) == cursor));
            }
            page = page.push(line);
        }
        container(scrollable(container(page).padding(Self::PAGE_PADDING)).id(Self::TRACKS))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// The cover if one has arrived, and the derived square until then.
    ///
    /// One function, because every caller wants the same fallback and a caller
    /// that forgot it would draw a hole. The square is not a placeholder to be
    /// replaced by a spinner: it is right on its own, so a cover that never
    /// comes costs nothing and one that does simply appears.
    fn picture(&self, seed: &str, art: &str, side: f32, corner: f32) -> Element<'_, Message> {
        let url = self.art_url(art);
        match self.covers.handle(&url) {
            Some(handle) => image(handle.clone())
                .width(Length::Fixed(side))
                .height(Length::Fixed(side))
                // Cropped to the square rather than letterboxed: a cover is
                // square and a photograph of a person is not, and a grid of
                // cards with grey bars down the sides of half of them is a
                // grid that looks broken.
                .content_fit(iced::ContentFit::Cover)
                .border_radius(corner)
                .into(),
            None => art::square::<Message>(seed, side, corner),
        }
    }

    /// One card: the square, the name, and what is under it.
    fn view_card(&self, card: &Card, at: usize, on_cursor: bool) -> Element<'_, Message> {
        let corner = if card.round { Self::CARD / 2.0 } else { 6.0 };
        let under = if card.under.is_empty() {
            format!("{} {}", card.tally, plural(card.tally, "track"))
        } else {
            card.under.clone()
        };
        mouse_area(
            container(
                column![
                    self.picture(&card.seed, &card.art, Self::CARD, corner),
                    // Bounded, and allowed to wrap. Unbounded with
                    // `Wrapping::None` the layout node is as wide as the text,
                    // so "Goldberg Variations, BWV 988" ran straight through
                    // the card beside it — two titles overlapping, which is
                    // worse than either being cut.
                    text(card.title.clone())
                        .size(13)
                        .width(Length::Fixed(Self::CARD))
                        .style(move |theme: &iced::Theme| text::Style {
                            color: Some(if on_cursor {
                                palette::of(theme).primary.base.color
                            } else {
                                palette::of(theme).background.base.text
                            }),
                        }),
                    text(under).size(11).width(Length::Fixed(Self::CARD)).style(
                        |theme: &iced::Theme| text::Style {
                            color: Some(palette::of(theme).background.base.text.scale_alpha(0.5)),
                        }
                    ),
                ]
                .spacing(4)
                .width(Length::Fixed(Self::CARD)),
            )
            // The cursor is the *title* going gold rather than a filled
            // rectangle. A card is mostly picture, and a fill behind one is a
            // border around an image — it reads as a selected file, not as
            // where the next `l` goes.
            .padding(0),
        )
        .on_enter(Message::HoverAt(at))
        .on_press(Message::Select(card.open.clone()))
        .into()
    }

    /// The header an album or an artist page opens with.
    ///
    /// The big square, what it is, its name, and the two or three numbers that
    /// are true of the whole of it. `albums` is `Some` on an artist page,
    /// because "nine albums" is the fact that differs there — an album page
    /// would be saying "1".
    fn view_record(
        &self,
        name: &str,
        under: &str,
        kind: &'static str,
        rows: &[Item],
        albums: Option<usize>,
        art: &str,
    ) -> Element<'_, Message> {
        let tracks = rows.len() as i64;
        let ms: i64 = rows.iter().map(|i| i.duration_ms).sum();
        let mut facts = match albums {
            Some(n) => format!("{n} {}", plural(n as i64, "album")),
            None => String::new(),
        };
        if !facts.is_empty() {
            facts.push_str("  ·  ");
        }
        facts.push_str(&format!("{tracks} {}", plural(tracks, "track")));
        if ms > 0 {
            facts.push_str("  ·  ");
            facts.push_str(&spell(ms));
        }

        self.view_header(name, under, kind, &facts, art, albums.is_some())
    }

    /// The header every page that is *about* something opens with: the
    /// square, what kind of thing it is, its name, one line of whatever is
    /// under it, and the numbers true of the whole of it.
    ///
    /// Shared, because there are five such pages now — an album, an artist, a
    /// composer, a work and a recording — and five copies of this would be
    /// five chances for one of them to draw the square at a different size.
    #[allow(clippy::too_many_arguments)]
    fn view_header(
        &self,
        name: &str,
        under: &str,
        kind: &'static str,
        facts: &str,
        art: &str,
        round: bool,
    ) -> Element<'_, Message> {
        let facts = facts.to_string();
        let side = 116.0;
        container(
            row![
                self.picture(name, art, side, if round { side / 2.0 } else { 8.0 }),
                column![
                    text(kind)
                        .size(10)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(palette::of(theme).background.base.text.scale_alpha(0.5)),
                        }),
                    text(name.to_string())
                        .size(26)
                        .wrapping(text::Wrapping::None),
                    // The performer is the one thing on this page that is also
                    // a *place*, so it is one: same accent, and a click goes
                    // there.
                    mouse_area(
                        text(under.to_string())
                            .size(13)
                            .style(|theme: &iced::Theme| text::Style {
                                color: Some(palette::of(theme).primary.base.color),
                            },)
                    )
                    .on_press(Message::Select(Source::Artist(under.to_string()))),
                    text(facts)
                        .size(11)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(palette::of(theme).background.base.text.scale_alpha(0.5)),
                        }),
                ]
                .spacing(4),
            ]
            .spacing(16)
            .align_y(iced::Alignment::Center),
        )
        .padding(iced::Padding {
            top: Self::PAGE_PADDING,
            right: Self::PAGE_PADDING,
            bottom: Self::PAGE_PADDING,
            left: 0.0,
        })
        .width(Length::Fill)
        .into()
    }

    fn view_list(&self, peer: &'_ Peer) -> Element<'_, Message> {
        let playing = self.player.track().map(|t| t.id);
        let sounding = self.player.is_playing();
        // Only drawn while this pane has the keyboard. A dimmed cursor here
        // would sit one shade away from the zebra and mean something entirely
        // different from it, which is a worse thing to show than nothing: the
        // sidebar's highlight is a *selection* and has to persist, but this
        // one only ever means "where the next `j` goes".
        let focused = self.pane == Pane::Tracks;
        let cursor = focused.then_some(self.at(Pane::Tracks));

        // An album is the one place a track number means anything. Everywhere
        // else the list is a library, a playlist or an artist — orderings that
        // have nothing to do with where a movement sits in its work — and a
        // column of numbers counting something else is worse than no column.
        let album_page = matches!(peer.source, Source::Album(_));
        // The part changes as the list is walked, and a heading is emitted
        // when it does. Derived rather than stored, like the sidebar's: the
        // rows are already in track order, so "the part changed" is the
        // whole of what a section boundary is.
        let mut part = String::new();

        let rows =
            peer.rows()
                .iter()
                .enumerate()
                .fold(column![].spacing(0), |mut col, (i, item)| {
                    let on_cursor = cursor == Some(i);
                    let detail = peer.detail_of(item.id);
                    if album_page && detail.part != part {
                        part = detail.part.clone();
                        if !part.is_empty() {
                            col = col.push(section(part.clone()));
                        }
                    }
                    // The one shape in the table, on the one row making a
                    // sound. Every other row leaves the column empty rather
                    // than drawing something greyed out: a mark that is
                    // always there is a mark that says nothing.
                    let here = playing == Some(item.id);
                    let mut line =
                        Row::new()
                            .spacing(0)
                            .align_y(iced::Alignment::Center)
                            .push(if here {
                                Element::from(
                                    button(icon::playing(!sounding, on_cursor))
                                        .style(button::text)
                                        .padding([0, 8])
                                        .on_press(Message::PlayPause),
                                )
                            } else {
                                // As tall as the button it stands in for, not as
                                // tall as an empty string. The heart used to set
                                // every row's height; an empty container sets
                                // none, so the rows that were not playing came out
                                // shorter than the one that was.
                                Element::from(
                                    container(text(""))
                                        .width(TRANSPORT)
                                        .height(Length::Fixed(icon::TRANSPORT)),
                                )
                            });
                    if album_page {
                        line = line.push(cell(
                            // 0 is "nobody said", and an empty cell says that
                            // better than a zero does.
                            if detail.track > 0 {
                                detail.track.to_string()
                            } else {
                                String::new()
                            },
                            TRACK,
                            on_cursor,
                            false,
                            true,
                        ));
                    }
                    let line = line
                        .push(cell(
                            item.title.clone(),
                            NAME,
                            on_cursor,
                            // The one playing is the only thing in the table drawn
                            // in the accent color, so it is findable at a glance
                            // in a list of twenty near-identical rows.
                            here,
                            false,
                        ))
                        .push(cell(item.creator.clone(), ARTIST, on_cursor, false, true))
                        .push(cell(
                            // On an album page every row's album is the one the
                            // heading already names, so the column is a wasted
                            // third of the width. Who played it is the fact
                            // that differs down the page — two recordings of
                            // one work are two performers, not two albums.
                            if album_page {
                                // …and the terms it was given on, beside
                                // whoever gave it. `recording.licence` is where
                                // that fact lives now; this is the one place it
                                // is drawn, and it has to *be* drawn — the
                                // demo's Brandenburgs are CC BY and CC BY-SA,
                                // and a credit nobody draws is a condition
                                // nobody met.
                                credit(&detail.performer, &detail.licence)
                            } else {
                                detail.album.clone()
                            },
                            ALBUM,
                            on_cursor,
                            false,
                            true,
                        ))
                        .push(cell(
                            clock(item.duration_ms as f64 / 1000.0),
                            TIME,
                            on_cursor,
                            false,
                            true,
                        ))
                        // The same menu a right click opens, for anyone who
                        // does not know a right click opens one.
                        .push(
                            button(icon::more(on_cursor))
                                .style(button::text)
                                .padding([0, 6])
                                .on_press(Message::RowMenu(item.id)),
                        );
                    col.push(
                        // The background belongs to a container spanning the whole
                        // width, not to a button around the title: a stripe that
                        // stops where the text does is not a row.
                        mouse_area(container(line).width(Length::Fill).padding([3, 4]).style(
                            move |theme: &iced::Theme| {
                                row_style(theme, on_cursor, focused, i % 2 == 1)
                            },
                        ))
                        .on_enter(Message::HoverAt(i))
                        .on_press(Message::PlayItem(item.id))
                        .on_right_press(Message::RowMenu(item.id)),
                    )
                });

        let mut headings = Row::new()
            .spacing(0)
            .align_y(iced::Alignment::Center)
            .push(container(text("")).width(TRANSPORT));
        if album_page {
            headings = headings.push(heading("#", TRACK));
        }
        let head = container(
            headings
                .push(heading("Name", NAME))
                .push(heading("Artist", ARTIST))
                .push(heading(
                    if album_page { "Performer" } else { "Album" },
                    ALBUM,
                ))
                .push(heading("Time", TIME)),
        )
        .width(Length::Fill)
        .padding([2, 4]);

        // An album or an artist page opens with a header that already says the
        // name and counts the tracks, so the table's own title would be the
        // same two facts twice, eighty pixels apart. Everywhere else — the
        // library, a playlist — there is no header and this *is* the title.
        let titled = !matches!(peer.source, Source::Album(_) | Source::Artist(_));
        #[cfg_attr(feature = "demo", allow(unused_mut))]
        let mut main = column![].spacing(10).width(Length::Fill);
        if titled {
            main = main.push(
                row![
                    text(peer.source.title().to_string()).size(22),
                    text(format!("{} tracks", peer.rows().len()))
                        .size(12)
                        .style(style::dim),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            );
        }

        #[cfg(not(feature = "demo"))]
        {
            // Turned away by the server: the one button that helps is sign in.
            let signed_out = self.login.as_ref().is_some_and(|l| l.token.is_empty());
            let actions = row![
                if signed_out {
                    button("sign in again")
                        .style(style::action)
                        .on_press_maybe((!self.signing_in).then_some(Message::SignIn))
                } else {
                    button(if peer.link.is_some() {
                        "go offline"
                    } else {
                        "go online"
                    })
                    .style(style::action)
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
            .push(
                scrollable(rows)
                    .id(Self::TRACKS)
                    .style(style::bars)
                    .height(Length::Fill),
            )
            .push(self.view_status(peer))
            .into()
    }

    /// Which playlists the track is on, and the row that makes another.
    ///
    /// Drawn in place of the table, the way the keymap is: the list underneath
    /// is what the question is about, and a panel over it would put the answer
    /// on top of the thing it describes.
    fn view_picker(picker: &Picker) -> Element<'_, Message> {
        let rows = picker.lists.iter().enumerate().fold(
            column![].spacing(0),
            |col, (i, (_, name, on))| {
                let on_cursor = picker.at == i;
                let line = Row::new()
                    .spacing(0)
                    .align_y(iced::Alignment::Center)
                    .push(
                        container(if *on {
                            Element::from(icon::tick(on_cursor))
                        } else {
                            Element::from(text(""))
                        })
                        .width(TRANSPORT)
                        .height(Length::Fixed(icon::TRANSPORT)),
                    )
                    .push(cell(name.clone(), NAME, on_cursor, *on, false));
                col.push(
                    mouse_area(container(line).width(Length::Fill).padding([3, 4]).style(
                        move |theme: &iced::Theme| row_style(theme, on_cursor, true, i % 2 == 1),
                    ))
                    .on_press(Message::PickerAt(i))
                    .on_release(Message::PickerActivate),
                )
            },
        );

        let last = picker.lists.len();
        let making: Element<'_, Message> = match &picker.naming {
            Some(name) => text_input("a name for it", name)
                .id(Self::NAMING)
                .on_input(Message::PickerName)
                .on_submit(Message::PickerCreate)
                .size(13)
                .padding([4, 6])
                .into(),
            None => {
                let on_cursor = picker.at == last;
                mouse_area(
                    container(cell(
                        "New playlist\u{2026}".into(),
                        NAME,
                        on_cursor,
                        false,
                        true,
                    ))
                    .width(Length::Fill)
                    .padding([3, 4])
                    .style(move |theme: &iced::Theme| {
                        row_style(theme, on_cursor, true, last % 2 == 1)
                    }),
                )
                .on_press(Message::PickerAt(last))
                .on_release(Message::PickerActivate)
                .into()
            }
        };

        // A panel rather than a page: it is about one row, and something the
        // size of the window is something you have to dismiss to see what you
        // were doing. Bounded in both directions so thirty playlists scroll
        // inside it instead of growing it off the screen.
        container(
            column![
                text(middle(&picker.title, 38)).size(15),
                text("j k move  \u{00b7}  <Enter> toggles  \u{00b7}  <Esc> back")
                    .size(11)
                    .style(style::dim),
                rule::horizontal(1),
                scrollable(rows.push(making))
                    .style(style::bars)
                    .height(Length::Shrink),
            ]
            .spacing(8),
        )
        .width(Length::Fixed(340.0))
        .max_height(420.0)
        .padding(12)
        .style(|theme: &iced::Theme| {
            let palette = palette::of(theme);
            container::Style {
                background: Some(iced::Background::Color(palette.background.weak.color)),
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..container::Style::default()
            }
        })
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
            ("<Space>", "play or pause"),
            (
                "a",
                "which playlists this track is on \u{2014} and make one",
            ),
            ("d", "which device is making the sound, and move it"),
            ("/", "search this pane; <Enter> accepts, <Esc> drops it"),
            ("n  N", "the next match, the one before"),
            ("{  }", "the previous track, the next one"),
            ("?", "this"),
        ];
        let rows = KEYS.iter().fold(column![].spacing(6), |col, (keys, what)| {
            col.push(
                row![
                    text(*keys).size(13).width(Length::Fixed(110.0)),
                    text(*what).size(13).style(style::dim),
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
        let mut line = format!(
            "{} songs · cursor {} · {} pending",
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
        // A session socket turned away is its own kind of offline, and it
        // does not look like the log's: the library keeps syncing and only
        // the devices go. Saying which is what stops "the picker is empty"
        // being a question.
        if let Some(reason) = self.listening.denied() {
            line = format!("{line}  ·  no listening session: {reason}");
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
            text(line).size(12).style(style::dim).width(Length::Fill),
            // A search shows a caret, so a half-typed query does not look
            // like a finished one that matched nothing.
            text(match self.keys.mode() {
                vim::Mode::Search(_) => format!("{}\u{2582}", self.keys.pending()),
                vim::Mode::Normal => self.keys.pending(),
            })
            .size(12)
            .style(text::primary),
            text(mode).size(12).style(style::dim),
        ]
        .spacing(12)
        .into()
    }

    /// The now-playing bar. Pinned to the bottom, and honest about silence:
    /// on a build with no audio device it says so rather than drawing a
    /// transport that does nothing when pressed.
    /// What the now-playing bar is drawing, whichever device is making it.
    ///
    /// The bar shows the *session*, not this window's player — so a laptop
    /// watching a phone draws the phone's track, its clock and its state, and
    /// the transport under it moves the phone. When the sound is here (or
    /// nowhere yet) it is the local player, because that answer is a frame
    /// fresher than anything a broadcast could be.
    fn bar(&self) -> Option<Bar> {
        if self.listening.elsewhere() {
            let track = self.listening.now()?;
            return Some(Bar {
                title: track.title.clone(),
                creator: track.creator.clone(),
                playing: self.listening.playing(),
                position: self.listening.position_ms() as f64 / 1000.0,
                duration: track.duration_ms as f64 / 1000.0,
            });
        }
        let track = self.player.track()?;
        Some(Bar {
            title: track.title.clone(),
            creator: track.creator.clone(),
            playing: self.player.is_playing(),
            position: self.player.position(),
            duration: self.player.duration(),
        })
    }

    /// Which device has the sound, as a button that opens the picker.
    ///
    /// Drawn only where there is a session to draw: a peer with no server, or
    /// one whose socket has not come up, has no devices to offer and a picker
    /// with one row in it that says "this one" is a control that does nothing.
    fn view_output(&self) -> Option<Element<'_, Message>> {
        if !self.listening.live() {
            return None;
        }
        let session = self.listening.session()?;
        let here = self.listening.outputs_here();
        let label = match session.output_device() {
            Some(_) if here => "this device".to_string(),
            Some(device) => middle(&device.name, 16),
            None => "no device".to_string(),
        };
        Some(
            button(
                row![
                    icon::devices(here),
                    text(label).size(12).style(move |theme: &iced::Theme| {
                        text::Style {
                            color: Some(if here {
                                palette::of(theme).primary.base.color
                            } else {
                                palette::of(theme).background.base.text.scale_alpha(0.75)
                            }),
                        }
                    }),
                ]
                .spacing(5)
                .align_y(iced::Alignment::Center),
            )
            .style(button::text)
            .on_press(Message::OpenDevices)
            .into(),
        )
    }

    fn view_bar(&self) -> Element<'_, Message> {
        let Some(bar) = self.bar() else {
            let idle = text(if Player::AUDIBLE {
                "nothing playing — pick a track"
            } else {
                "nothing playing — pick a track (the desktop build has no audio device; \
                 the browser one streams)"
            })
            .size(12)
            .style(style::dim);
            let mut line = row![idle].spacing(12).align_y(iced::Alignment::Center);
            if let Some(output) = self.view_output() {
                line = line.push(container(text("")).width(Length::Fill));
                line = line.push(output);
            }
            return container(line).padding([8, 4]).into();
        };

        let duration = bar.duration.max(0.1);
        // Nothing here takes the cursor: `<Space>`, `{`, `}` and `d` do all
        // four, so a pane for them would be a stop on `<Tab>` that nobody
        // needs to pass through.
        //
        // The transport is enabled whenever there is a session to send it to,
        // even in a build that cannot make a sound itself — being a remote
        // control is a use, and `AUDIBLE` only decides whether *this* device
        // can be the one playing.
        let workable = Player::AUDIBLE || self.listening.elsewhere();
        let transport = row![
            button(icon::previous())
                .style(button::text)
                .on_press(Message::Skip(-1)),
            button(if bar.playing {
                icon::pause()
            } else {
                icon::play()
            })
            .style(button::text)
            .on_press_maybe(workable.then_some(Message::PlayPause)),
            button(icon::next())
                .style(button::text)
                .on_press(Message::Skip(1)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        let mut line = row![
            transport,
            column![
                text(bar.title).size(14),
                text(bar.creator).size(12).style(style::dim),
            ]
            .spacing(2)
            .width(Length::Fixed(260.0)),
            text(clock(bar.position)).size(11).style(style::dim),
            // Seeking is the element's job in a browser, and there is
            // nothing to seek without one — so the slider only moves where
            // a track can actually be moved to.
            slider(0.0..=duration as f32, bar.position as f32, Message::Seek)
                .style(style::seek)
                .width(Length::Fill),
            text(clock(duration)).size(11).style(style::dim),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);
        if let Some(output) = self.view_output() {
            line = line.push(output);
        }

        container(line).padding([6, 4]).into()
    }

    /// Where the sound is, and everywhere it could be.
    ///
    /// One row per device of this account, plus a last row that stops it
    /// everywhere — which is the same shape the playlist picker has, and for
    /// the same reason: a list whose last row is the other thing you might
    /// want means `j` reaches it without a second key to learn.
    ///
    /// A device that cannot be heard is drawn and not selectable. Hiding it
    /// would be worse: a laptop that is *in* the session and controlling it
    /// should be able to see itself listed, and "this one has no audio device"
    /// is a different answer from "this one is not here".
    fn view_devices(&self, at: usize) -> Element<'_, Message> {
        let devices = self.listening.devices();
        let output = self
            .listening
            .session()
            .and_then(|s| s.output.as_deref())
            .unwrap_or("");
        let mut rows = column![].spacing(0);
        for (i, device) in devices.iter().enumerate() {
            let on_cursor = at == i;
            let is_output = device.id == output;
            let mine = device.id == self.listening.me();
            let audible = device.audible;
            let label = if mine {
                format!("{} (this one)", middle(&device.name, 14))
            } else {
                middle(&device.name, 24)
            };
            // A tick only where the sound is. The blank one is as tall as the
            // tick it stands in for, because an empty container has no height
            // and the rows without a tick would come out shorter than the one
            // with it — the same trap the table's transport column has.
            let mark: Element<'_, Message> = if is_output {
                icon::tick(on_cursor).into()
            } else {
                container(text(""))
                    .width(Length::Fixed(icon::TRANSPORT))
                    .height(Length::Fixed(icon::TRANSPORT))
                    .into()
            };
            let entry = container(
                row![
                    mark,
                    text(label)
                        .size(13)
                        .style(move |theme: &iced::Theme| text::Style {
                            color: Some(if on_cursor {
                                palette::of(theme).primary.base.text
                            } else if !audible {
                                palette::of(theme).background.base.text.scale_alpha(0.4)
                            } else {
                                palette::of(theme).background.base.text
                            }),
                        }),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fixed(DEVICE_ROW))
            .padding([5, 10])
            .style(move |theme: &iced::Theme| container::Style {
                background: on_cursor
                    .then(|| iced::Background::Color(palette::of(theme).primary.base.color)),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
            // A row for a device that cannot be heard is not a target: the
            // server would refuse the transfer anyway, and a control that
            // looks pressable and is not is worse than one that is plainly
            // not.
            let entry: Element<'_, Message> = if audible {
                mouse_area(entry)
                    .on_press(Message::DeviceAt(i))
                    .on_release(Message::PickDevice(Some(device.id.clone())))
                    .into()
            } else {
                entry.into()
            };
            rows = rows.push(entry);
        }

        let last = devices.len();
        let on_cursor = at == last;
        rows = rows.push(
            mouse_area(
                container(
                    text("Stop everywhere")
                        .size(13)
                        .style(move |theme: &iced::Theme| text::Style {
                            color: Some(if on_cursor {
                                palette::of(theme).primary.base.text
                            } else {
                                palette::of(theme).background.base.text.scale_alpha(0.7)
                            }),
                        }),
                )
                .width(Length::Fixed(DEVICE_ROW))
                .padding([5, 10])
                .style(move |theme: &iced::Theme| container::Style {
                    background: on_cursor
                        .then(|| iced::Background::Color(palette::of(theme).primary.base.color)),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
            )
            .on_press(Message::DeviceAt(last))
            .on_release(Message::PickDevice(None)),
        );

        container(
            column![
                container(text("Playing on").size(11).style(style::dim)).padding([4, 10]),
                rows,
            ]
            .spacing(0),
        )
        .padding(4)
        .style(|theme: &iced::Theme| {
            let palette = palette::of(theme);
            container::Style {
                background: Some(iced::Background::Color(palette.background.weak.color)),
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            }
        })
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
                button(label)
                    .style(style::action)
                    .on_press_maybe((!self.signing_in).then_some(Message::SignIn)),
                text(self.note.clone()).size(13).style(style::dim),
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
            iced::window::resize_events().map(|(_, size)| Message::Resized(size)),
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

/// The demo's covers, walked the way the demo walks them.
///
/// This has now been wrong twice — once because the ask was wired to the wire
/// and once because nobody had ever run the path — and both times what the
/// page showed was a grid of correct-looking derived squares, which is exactly
/// what it shows when everything works and eight of the twelve albums have no
/// picture. A silent fallback is the hardest kind of broken to see, so the
/// path gets a test rather than a reading.
#[cfg(all(test, feature = "demo"))]
mod demo_covers {
    use super::{covers, media_url, seed, App, Peer};

    /// A demo peer with a library in it, from nothing.
    ///
    /// `seed` returns early on a database that already has tracks, and the
    /// native demo's is a file in the temp directory that outlives the run — so
    /// this deletes it first.
    ///
    /// **`who` is a database name, and every caller needs its own.** The path
    /// is `petros-demo-{id}.db`, cargo runs a binary's tests on several threads
    /// at once, and two tests both deleting and reseeding one file fail only
    /// when they are run *together*: each passes alone and the suite fails,
    /// which is the worst way to find out. One name each and they cannot meet.
    pub fn fresh(who: &str) -> Peer {
        let mut peer = blank(who);
        seed::seed(&mut peer);
        peer
    }

    /// …and the same with nothing in it, for the tests about what a library
    /// with no music offers.
    pub fn blank(who: &str) -> Peer {
        let db = std::env::temp_dir().join(format!("petros-demo-{who}.db"));
        // `-intents` is the one that matters and is the one easily forgotten:
        // a client's pending mutations live in a file of their own, so
        // deleting the database alone reopens a peer that replays every song
        // the last run authored — and `seed` then returns early on a library
        // it did not make.
        for suffix in ["", "-wal", "-shm", "-intents"] {
            let _ = std::fs::remove_file(format!("{}{}", db.display(), suffix));
        }
        Peer::open(&App::demo_login_for(who))
    }

    #[test]
    fn the_seeded_art_reaches_a_fetch() {
        let peer = fresh("covers");

        let albums: Vec<&str> = peer
            .albums
            .iter()
            .filter(|a| !a.art.is_empty())
            .map(|a| a.name.as_str())
            .collect();
        let artists: Vec<&str> = peer
            .artists
            .iter()
            .filter(|a| !a.art.is_empty())
            .map(|a| a.name.as_str())
            .collect();

        assert!(
            albums.contains(&"Water Music"),
            "an album seeded with a cover has to come back carrying it; \
             albums with art: {albums:?}"
        );
        assert!(
            artists.contains(&"George Frideric Handel"),
            "…and so does a composer; artists with art: {artists:?}"
        );

        // The demo has no server, so the join is `media_url("", …)` — which is
        // the case a Wikimedia URL passes through unchanged. A relative path
        // here would resolve against the page and 200 with the index.
        let mut want = covers::Covers::default();
        let asked = peer
            .albums
            .iter()
            .map(|a| a.art.clone())
            .chain(peer.artists.iter().map(|a| a.art.clone()))
            .filter(|art| !art.is_empty())
            .filter(|art| want.want(media_url("", art)).is_some())
            .count();
        assert_eq!(
            asked,
            albums.len() + artists.len(),
            "every seeded cover is a fetch the client would make"
        );
    }
}

/// What the demo's own library comes out as, through the work chain.
///
/// Not one line of `seed.rs` moved for this: every track already carried a
/// catalogue number or a part, and `add_song` reads either as saying there is a
/// work whose name is the record's. So this is the replay rule for entries
/// written before any of it existed, checked against the only real library
/// there is rather than against a fixture — which is the half a unit test
/// cannot do.
#[cfg(all(test, feature = "demo"))]
mod demo_works {
    use super::demo_covers::fresh;

    #[test]
    fn the_demo_is_composers_with_works() {
        let mut peer = fresh("works");
        let store = &mut peer.client.store();

        let composers = harken::composers(store).unwrap();
        assert!(
            composers.len() >= 5,
            "the demo is seven composers; it has {}: {:?}",
            composers.len(),
            composers.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        let bach = composers
            .iter()
            .find(|c| c.name == "Johann Sebastian Bach")
            .expect("Bach is in the demo");
        assert!(
            bach.works >= 3,
            "Bach has the Goldbergs, the Brandenburgs and the Well-Tempered \
             Clavier at least; this says {}",
            bach.works
        );

        // The Goldbergs: one work, one recording, thirty-two movements — which
        // is the shape the old schema could not say, because thirty-two tracks
        // with one catalogue number between them was thirty-two catalogue
        // numbers.
        let works = harken::works(store, "Johann Sebastian Bach".into()).unwrap();
        let goldbergs = works
            .iter()
            .find(|w| w.catalogue == "BWV 988")
            .expect("BWV 988 is in the demo");
        assert_eq!(goldbergs.recordings, 1, "one performance of it is seeded");
        assert!(goldbergs.tracks > 20, "tracks: {}", goldbergs.tracks);

        let takes = harken::recordings(store, goldbergs.id.clone()).unwrap();
        assert_eq!(takes.len(), 1);
        assert_eq!(takes[0].performers, "Kimiko Ishizaka");
    }

    /// A work has a name of its own, not the record's.
    ///
    /// Twenty-four preludes and fugues on one record were twenty-four works all
    /// called "The Well-Tempered Clavier", because a work with no name falls
    /// back to the album it is on and this library puts a whole collection on
    /// one. `WORKS` is what says otherwise. Falsify it by dropping `work_title`
    /// from the seed's `add_song`: the set of titles collapses to one.
    #[test]
    fn a_work_is_named_for_itself() {
        let mut peer = fresh("named");
        let store = &mut peer.client.store();
        let works = harken::works(store, "Johann Sebastian Bach".into()).unwrap();

        let wtc: Vec<&str> = works
            .iter()
            .filter(|w| w.form == "Prelude and Fugue")
            .map(|w| w.title.as_str())
            .collect();
        assert_eq!(wtc.len(), 24, "Book I is twenty-four of them");
        let distinct: std::collections::BTreeSet<&&str> = wtc.iter().collect();
        assert_eq!(
            distinct.len(),
            24,
            "…and twenty-four names, not one name twenty-four times: {wtc:?}"
        );
        assert!(wtc.contains(&"Prelude and Fugue No. 1 in C Major"));

        // The six Brandenburgs, which had the same problem.
        let brandenburgs: Vec<&str> = works
            .iter()
            .filter(|w| w.catalogue.starts_with("BWV 10"))
            .map(|w| w.title.as_str())
            .collect();
        assert_eq!(brandenburgs.len(), 6);
        assert!(brandenburgs.contains(&"Brandenburg Concerto No. 4 in G Major"));
    }

    /// **Nobody is called `(CC BY-SA 3.0)`.**
    ///
    /// The wart stage one left standing: the seed wrote `"{performer}
    /// ({licence})"` into one column, so a recording nobody was credited on was
    /// credited to a person named after a licence. The terms are on
    /// `recording.licence` now. Falsify it by putting the licence back in the
    /// performer string — the name turns up in `artists()`.
    #[test]
    fn nobody_is_named_after_a_licence() {
        let mut peer = fresh("licence");
        let store = &mut peer.client.store();

        // Every credited name in the library. `artists()` is the wrong place to
        // look and this test asked it first: that one reads `media.creator`,
        // which is the *composer*, so the junk person sat in `credit` where it
        // could not see it — and the test passed with the bug reinstated.
        let mut credited: Vec<String> = Vec::new();
        for composer in harken::composers(store).unwrap() {
            for work in harken::works(store, composer.name.clone()).unwrap() {
                for take in harken::recordings(store, work.id.clone()).unwrap() {
                    for credit in harken::credits(store, take.id.clone()).unwrap() {
                        credited.push(credit.name);
                    }
                }
            }
        }
        assert!(
            credited.len() > 5,
            "this has to be looking at something: {credited:?}"
        );
        let wrong: Vec<&String> = credited
            .iter()
            .filter(|n| n.contains("CC BY") || n.contains('('))
            .collect();
        assert!(wrong.is_empty(), "a licence is not a person: {wrong:?}");
    }

    /// …and the licence is still on the screen, which is the other half.
    ///
    /// Moving it out of the performer column would have been a regression
    /// dressed as a cleanup: the demo's Brandenburgs are CC BY and CC BY-SA, and
    /// a credit nobody draws is a condition nobody met. `TrackDetail.licence`
    /// carries it and `credit` puts the two back together for the one column a
    /// table has. Falsify it by dropping the field from `track_details`.
    #[test]
    fn the_terms_are_still_drawn_beside_whoever_gave_them() {
        let mut peer = fresh("terms");
        let store = &mut peer.client.store();
        let details = harken::track_details(store).unwrap();

        let licensed: Vec<&harken::TrackDetail> =
            details.iter().filter(|d| !d.licence.is_empty()).collect();
        assert!(
            licensed.len() > 5,
            "the Brandenburgs alone are more than five licensed tracks; {} found",
            licensed.len()
        );
        let drawn = super::credit(&licensed[0].performer, &licensed[0].licence);
        assert!(
            drawn.contains("CC BY"),
            "the column a person reads has to carry the terms: {drawn:?}"
        );

        // And a recording that reserves nothing says only who played it.
        let free = details
            .iter()
            .find(|d| d.licence.is_empty() && !d.performer.is_empty())
            .expect("most of the demo reserves nothing");
        assert_eq!(
            super::credit(&free.performer, &free.licence),
            free.performer
        );
    }

    /// One lumped string becomes two people with two roles, in billing order.
    ///
    /// "London Symphony Orchestra, Hermann Scherchen" is all `add_song` gets and
    /// splitting it is a guess, so `CREDITS` is where the demo says which half
    /// is the orchestra. Falsify it by removing the Messiah rows from `CREDITS`:
    /// the fallback survives and the assertion still reads the same string —
    /// which is why this checks the *roles* rather than the joined line.
    #[test]
    fn the_messiah_has_an_orchestra_and_a_conductor() {
        let mut peer = fresh("credits");
        let store = &mut peer.client.store();
        let works = harken::works(store, "George Frideric Handel".into()).unwrap();
        let messiah = works
            .iter()
            .find(|w| w.catalogue == "HWV 56")
            .expect("the Messiah is in the demo");
        let takes = harken::recordings(store, messiah.id.clone()).unwrap();
        assert_eq!(
            takes[0].performers, "London Symphony Orchestra, Hermann Scherchen",
            "billing order: the orchestra is first on the record"
        );

        // The part that says they are two rows and not one string: two people,
        // holding the two roles a classical service browses by.
        let credits = harken::credits(store, takes[0].id.clone()).unwrap();
        assert_eq!(
            credits
                .iter()
                .map(|c| (c.name.as_str(), c.role.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("London Symphony Orchestra", "orchestra"),
                ("Hermann Scherchen", "conductor"),
            ],
            "two rows with two roles, in billing order — and the lumped \
             fallback displaced rather than left beside them"
        );
    }

    /// The whole path a person takes: Composers → a composer → a work → a
    /// recording → its tracks, with every page's own state loaded.
    ///
    /// Each of these pages reads a *different* list, and `reload_shown` clears
    /// three of them before filling the new one — so the way to get this wrong
    /// is for a page to draw the last page's rows, or none. That is invisible
    /// in a screenshot of any one page and obvious from a walk.
    ///
    /// It caught two. The work page looked its row up in `works`, which holds
    /// one *composer's* works and is empty on a page reached by key; the
    /// recording page did the same with `recordings`. Both drew a header with
    /// no catalogue, no period and no cover, which reads as missing data.
    #[test]
    fn the_whole_path_from_a_composer_to_a_movement() {
        let mut peer = fresh("walk");

        peer.source = super::Source::Composers;
        peer.reload_sidebar();
        peer.reload_shown();
        assert!(
            peer.composers
                .iter()
                .any(|c| c.name == "Johann Sebastian Bach"),
            "the composers page has to have composers on it"
        );

        peer.source = super::Source::Works("Johann Sebastian Bach".into());
        peer.reload_shown();
        let goldbergs = peer
            .works
            .iter()
            .find(|w| w.catalogue == "BWV 988")
            .expect("Bach's page lists the Goldbergs")
            .clone();
        assert_eq!(goldbergs.title, "Goldberg Variations");

        peer.source = super::Source::Work(goldbergs.id.clone(), goldbergs.title.clone());
        peer.reload_shown();
        assert_eq!(
            peer.works.len(),
            1,
            "the work's own row, for the header — not the composer's list, \
             which is empty on a page reached by key"
        );
        assert_eq!(peer.works[0].catalogue, "BWV 988");
        assert_eq!(peer.recordings.len(), 1);
        let take = peer.recordings[0].clone();
        assert_eq!(take.performers, "Kimiko Ishizaka");

        peer.source = super::Source::Recording(take.id.clone(), String::new());
        peer.reload_shown();
        assert!(peer.rows().len() > 20, "tracks: {}", peer.rows().len());
        assert_eq!(
            peer.recordings
                .iter()
                .find(|r| r.id == take.id)
                .map(|r| r.tracks),
            Some(peer.rows().len() as i64),
            "the recording's own row has to be loaded too, or its header has \
             no year, no label and no licence"
        );
        match &peer.source {
            super::Source::Recording(_, who) => {
                assert_eq!(who, "Kimiko Ishizaka", "a page titled with nothing")
            }
            _ => unreachable!(),
        }
    }

    /// A work link resolves by its key, and one naming nothing lands on the
    /// library rather than on an empty page.
    #[test]
    fn a_work_link_resolves_by_its_key() {
        let mut peer = fresh("links");
        peer.reload_sidebar();
        let id = harken::work_key("Johann Sebastian Bach", "BWV 988", "Goldberg Variations");

        // The key has a slash in it — composer then catalogue — which is why
        // `Route::parse` splits on the first separator only.
        assert!(id.contains('/'), "the key is composer/catalogue: {id}");
        let route = super::route::Route::parse(&super::route::Route::Work(id.clone()).fragment());
        assert_eq!(route, super::route::Route::Work(id.clone()), "round trip");

        assert_eq!(
            peer.source_of(&route),
            super::Source::Work(id, "Goldberg Variations".into())
        );
        assert_eq!(
            peer.source_of(&super::route::Route::Work("nobody/nothing".into())),
            super::Source::Library
        );
    }

    /// The sidebar draws a line only when there are rows behind it.
    ///
    /// The rule that lets one schema serve every genre: a library of pop has
    /// no works, so it has no Composers page — and a client that drew the line
    /// anyway would offer a page for a question the library cannot answer.
    #[test]
    fn the_sidebar_only_offers_what_there_is() {
        let mut peer = fresh("sidebar");
        peer.reload_sidebar();
        let lines: Vec<&str> = peer.choices.iter().map(|c| c.label.as_str()).collect();
        assert!(lines.contains(&"Composers"), "{lines:?}");
        assert!(lines.contains(&"Albums"), "{lines:?}");

        // …and an empty library offers the library and the playlist every peer
        // makes on its first run, and nothing else. No albums, no artists, no
        // composers — there is nothing to browse *by* until something is added,
        // and three empty pages is three ways to find out the hard way.
        let mut bare = super::demo_covers::blank("bare");
        bare.reload_sidebar();
        let lines: Vec<&str> = bare.choices.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(lines, vec!["Songs", "Favorites"], "an empty library");
    }

    /// `WORKS` is keyed by the catalogue number alone, so two of them sharing
    /// one would silently give a work somebody else's title.
    ///
    /// The log's key includes the composer; this table's does not, because
    /// nothing here needs it and repeating the composer on forty-three rows is
    /// forty-three chances to mistype one.
    #[test]
    fn the_catalogue_is_the_key() {
        let mut seen = std::collections::BTreeSet::new();
        for w in super::seed::WORKS {
            assert!(
                seen.insert(w.catalogue),
                "two works both claim {}: {}",
                w.catalogue,
                w.title
            );
        }
    }
}

/// The card grid's arithmetic, which nothing on screen can check for you.
#[cfg(test)]
mod cards {
    use super::{columns_in, App, SCROLLBAR};

    /// Every width a window can be, and the row it produces has to fit.
    ///
    /// The bug this is written for: `columns_in` did not subtract the
    /// scrollbar, so at the widths where N cards needed the whole pane the row
    /// came out ten pixels over. iced clamps a `Fixed` child to the space that
    /// is left, so the *last* card in the row was drawn narrower than the
    /// others — one card in five at the wrong size, which reads as a rendering
    /// fault and is an off-by-ten.
    ///
    /// Falsify it by removing `SCROLLBAR` from the subtraction: it fails at
    /// 964 and at every width where a row is exactly full.
    #[test]
    fn cards_never_overflow_their_row() {
        for width in 320..=4000 {
            let width = width as f32;
            let n = columns_in(width) as f32;
            let needed = n * App::CARD + (n - 1.0) * App::CARD_GAP;
            let room = width - App::SIDEBAR_WIDTH - App::PAGE_PADDING * 2.0 - SCROLLBAR;
            assert!(
                n == 1.0 || needed <= room,
                "at {width}px the page draws {n} cards needing {needed}px in {room}px — \
                 the last one is clamped and comes out a different size from the rest"
            );
        }
    }

    /// …and it must not be needlessly stingy either: whenever another card
    /// would fit, it is drawn. Without this half, "subtract more" passes the
    /// test above and wastes a column.
    #[test]
    fn a_card_that_fits_is_drawn() {
        for width in 320..=4000 {
            let width = width as f32;
            let n = columns_in(width) as f32;
            let one_more = (n + 1.0) * App::CARD + n * App::CARD_GAP;
            let room = width - App::SIDEBAR_WIDTH - App::PAGE_PADDING * 2.0 - SCROLLBAR;
            assert!(
                one_more > room,
                "at {width}px there is room for {} cards and only {n} are drawn",
                n + 1.0
            );
        }
    }
}
