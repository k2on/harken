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
mod player;

use std::time::Duration;

use harken::{self as mutators, HarkenApp, Item};
use iced::widget::{button, column, container, row, rule, scrollable, slider, text, Row};
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
    fn title(&self) -> &str {
        match self {
            Source::Library => "Library",
            Source::Playlist(_, name) | Source::Album(name) | Source::Artist(name) => name,
        }
    }
}

// The demo drives only a few of these: it has no accounts to sign in or out
// of, no entry box to type a song into, and no remove button. The variants
// stay so that the one `update` serves both builds.
#[cfg_attr(feature = "demo", allow(dead_code))]
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
    /// Pump the transport. Nothing else drives a sans-io client.
    Tick,
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
    /// The sidebar, rebuilt when the library changes rather than per frame.
    playlists: Vec<harken::Playlist>,
    albums: Vec<harken::Album>,
    artists: Vec<harken::Artist>,
    pending: usize,
}

struct App {
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
            source: Source::Library,
            shown: Vec::new(),
            playlists: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
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
        self.playlists = harken::playlists(&mut store).unwrap_or_default();
        self.albums = harken::albums(&mut store).unwrap_or_default();
        self.artists = harken::artists(&mut store).unwrap_or_default();
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
        // Public-domain recordings on Wikimedia Commons, by way of the mp3
        // Commons transcodes every audio file gets: a browser plays mp3
        // everywhere, and Vorbis in an `.ogg` does not play in Safari at all.
        //
        // The URL goes in `file`, which is what that column has always been
        // for — "the bytes travel over HTTP and only the name of them is
        // synced". Nothing about the log changes to carry a recording.
        //
        // Every one of these was checked: public domain by Commons' own
        // licence field, and a transcode that answers with `audio/mpeg` and a
        // range request. A dead link here is a silent demo, so they are not
        // taken on trust.
        const LIBRARY: &[(&str, &str, &str, i64, &str)] = &[
    (
        "Air on the G String",
        "Johann Sebastian Bach",
        "Orchestral Suite No. 3",
        260000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/1/1e/Air_%28Bach%29.ogg/Air_%28Bach%29.ogg.mp3",
    ),
    (
        "Toccata and Fugue in D minor, BWV 565",
        "Johann Sebastian Bach",
        "Organ Works",
        514000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/b/be/Toccata_et_Fugue_BWV565.ogg/Toccata_et_Fugue_BWV565.ogg.mp3",
    ),
    (
        "Für Elise",
        "Ludwig van Beethoven",
        "Bagatelles",
        177000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/7/7b/FurElise.ogg/FurElise.ogg.mp3",
    ),
    (
        "Moonlight Sonata - I. Adagio sostenuto",
        "Ludwig van Beethoven",
        "Piano Sonata No. 14",
        307000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/d/d0/Moonlight_Sonata.ogg/Moonlight_Sonata.ogg.mp3",
    ),
    (
        "Symphony No. 5 - I. Allegro con brio",
        "Ludwig van Beethoven",
        "Symphony No. 5",
        436000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/5/5b/Ludwig_van_Beethoven_-_Symphonie_5_c-moll_-_1._Allegro_con_brio.ogg/Ludwig_van_Beethoven_-_Symphonie_5_c-moll_-_1._Allegro_con_brio.ogg.mp3",
    ),
    (
        "Symphony No. 5 - III. Allegro",
        "Ludwig van Beethoven",
        "Symphony No. 5",
        336000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/5/5b/Ludwig_van_Beethoven_-_symphony_no._5_in_c_minor%2C_op._67_-_iii._allegro.ogg/Ludwig_van_Beethoven_-_symphony_no._5_in_c_minor%2C_op._67_-_iii._allegro.ogg.mp3",
    ),
    (
        "Eine kleine Nachtmusik - I. Allegro",
        "Wolfgang Amadeus Mozart",
        "Eine kleine Nachtmusik",
        253000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/6/68/Mozart_K525_Serenade_in_G_Major_1_-_Allegro.ogg/Mozart_K525_Serenade_in_G_Major_1_-_Allegro.ogg.mp3",
    ),
    (
        "Eine kleine Nachtmusik - III. Minuet",
        "Wolfgang Amadeus Mozart",
        "Eine kleine Nachtmusik",
        123000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/a/a0/Mozart_K525_Serenade_in_G_Major_3_-_Minuet.ogg/Mozart_K525_Serenade_in_G_Major_3_-_Minuet.ogg.mp3",
    ),
    (
        "Eine kleine Nachtmusik - IV. Rondo",
        "Wolfgang Amadeus Mozart",
        "Eine kleine Nachtmusik",
        194000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/3/3b/Mozart_K525_Serenade_in_G_Major_4_-_Rondo.ogg/Mozart_K525_Serenade_in_G_Major_4_-_Rondo.ogg.mp3",
    ),
    (
        "Ballade No. 1 in G minor, Op. 23",
        "Frédéric Chopin",
        "Ballades",
        679000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/3/33/Frederic_Chopin_-_ballade_no._1_in_g_minor%2C_op._23.ogg/Frederic_Chopin_-_ballade_no._1_in_g_minor%2C_op._23.ogg.mp3",
    ),
    (
        "Ballade No. 2 in F major, Op. 38",
        "Frédéric Chopin",
        "Ballades",
        420000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/c/cf/Frederic_Chopin_-_ballade_no._2_in_f_major%2C_op._38.ogg/Frederic_Chopin_-_ballade_no._2_in_f_major%2C_op._38.ogg.mp3",
    ),
    (
        "Swan Lake - Dance of the Swans",
        "Pyotr Ilyich Tchaikovsky",
        "Swan Lake",
        81000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/3/35/Tchaikovsky_Swan_Lake_Op.20_No.13._Danses_des_cygnes_IV.ogg/Tchaikovsky_Swan_Lake_Op.20_No.13._Danses_des_cygnes_IV.ogg.mp3",
    ),
    (
        "Swan Lake - Scene",
        "Pyotr Ilyich Tchaikovsky",
        "Swan Lake",
        150000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/1/1f/Tchaikovsky_Swan_Lake_Op.20_No.10._Sc%C3%A8ne.ogg/Tchaikovsky_Swan_Lake_Op.20_No.10._Sc%C3%A8ne.ogg.mp3",
    ),
    (
        "Winter - I. Allegro non molto",
        "Antonio Vivaldi",
        "The Four Seasons",
        198000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/0/04/Vivaldi_Winter_mvt_1_Allegro_non_molto_-_The_USAF_Concert.ogg/Vivaldi_Winter_mvt_1_Allegro_non_molto_-_The_USAF_Concert.ogg.mp3",
    ),
    (
        "Water Music - Allegro",
        "George Frideric Handel",
        "Water Music",
        125000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/d/de/Handel%27s_Water_Music_-_11._Allegro_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_11._Allegro_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
    ),
    (
        "Water Music - Bourrée",
        "George Frideric Handel",
        "Water Music",
        76000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/2/2d/Handel%27s_Water_Music_-_15._Bourree_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_15._Bourree_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
    ),
    (
        "Impromptu in G-flat major, D. 899",
        "Franz Schubert",
        "Impromptus",
        301000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/0/0b/Schubert_Gb_Impromptu_Andriy_Bondarenko_%28Live%29.ogg/Schubert_Gb_Impromptu_Andriy_Bondarenko_%28Live%29.ogg.mp3",
    ),
    (
        "Hungarian Dance No. 1",
        "Johannes Brahms",
        "Hungarian Dances",
        57000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/d/d6/Brahms_-_Hungarian_Dance_No._1_%28performed_by_the_composer%29.oga/Brahms_-_Hungarian_Dance_No._1_%28performed_by_the_composer%29.oga.mp3",
    ),
    (
        "Hungarian Dance No. 5",
        "Johannes Brahms",
        "Hungarian Dances",
        175000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/0/0a/Brahms_nikisch_hd5.ogg/Brahms_nikisch_hd5.ogg.mp3",
    ),
    (
        "Clair de lune",
        "Claude Debussy",
        "Suite bergamasque",
        304000,
        "https://upload.wikimedia.org/wikipedia/commons/transcoded/b/be/Clair_de_lune_%28Claude_Debussy%29_Suite_bergamasque.ogg/Clair_de_lune_%28Claude_Debussy%29_Suite_bergamasque.ogg.mp3",
    ),
        ];
        for (title, artist, album, ms, file) in LIBRARY {
            let _ = peer.client.mutate(mutators::add_song(
                (*title).into(),
                (*artist).into(),
                (*album).into(),
                *ms,
                (*file).into(),
            ));
        }
        peer.refresh();
        // A few of them hearted, so the playlist is not empty either.
        let hearted: Vec<harken::Id<harken::tables::Media>> = peer
            .items
            .iter()
            .filter(|i| {
                matches!(
                    i.title.as_str(),
                    "Clair de lune" | "Für Elise" | "Air on the G String"
                )
            })
            .map(|i| i.id)
            .collect();
        for id in hearted {
            let _ = peer
                .client
                .mutate(mutators::add_to_playlist(peer.playlist, id));
        }
        peer.refresh();

        // Two more playlists, so the sidebar shows what a playlist *is* here:
        // an ordered list somebody made, of which "Favourites" is one and not
        // a special case. Made after the hearts above so that the first
        // playlist — the one a heart means — stays Favourites.
        const SETS: &[(&str, &[&str])] = &[
            (
                "Piano",
                &[
                    "Für Elise",
                    "Moonlight Sonata - I. Adagio sostenuto",
                    "Clair de lune",
                    "Ballade No. 1 in G minor, Op. 23",
                    "Impromptu in G-flat major, D. 899",
                ],
            ),
            (
                "Strings",
                &[
                    "Air on the G String",
                    "Eine kleine Nachtmusik - I. Allegro",
                    "Winter - I. Allegro non molto",
                    "Swan Lake - Scene",
                ],
            ),
        ];
        for (name, titles) in SETS {
            let _ = peer
                .client
                .mutate(mutators::create_playlist((*name).into()));
            peer.refresh();
            let Some(list) = peer
                .playlists
                .iter()
                .find(|p| p.name == *name)
                .map(|p| p.id)
            else {
                continue;
            };
            let ids: Vec<harken::Id<harken::tables::Media>> = titles
                .iter()
                .filter_map(|t| peer.items.iter().find(|i| i.title == *t))
                .map(|i| i.id)
                .collect();
            for id in ids {
                let _ = peer.client.mutate(mutators::add_to_playlist(list, id));
            }
            peer.refresh();
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
            Self::seed(&mut peer);
            let app = App {
                player: Player::new(),
                queue: Vec::new(),
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
                player: Player::new(),
                queue: Vec::new(),
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
            &item.file,
        );
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
                    Message::Select(source) => {
                        peer.source = source;
                        peer.reload_shown();
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
                                &item.file,
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
        fn heading(label: &str) -> Element<'_, Message> {
            text(label).size(12).style(text::secondary).into()
        }

        let entry = |label: String, count: Option<i64>, source: Source, current: &Source| {
            let selected = *current == source;
            button(
                row![
                    text(label).width(Length::Fill).size(14),
                    text(count.map(|n| n.to_string()).unwrap_or_default())
                        .size(11)
                        .style(text::secondary),
                ]
                .spacing(6),
            )
            .style(if selected {
                button::primary
            } else {
                button::text
            })
            .padding([4, 8])
            .width(Length::Fill)
            .on_press(Message::Select(source))
        };

        let mut side = column![
            entry(
                "Library".into(),
                Some(peer.items.len() as i64),
                Source::Library,
                &peer.source
            ),
            heading("Playlists"),
        ]
        .spacing(2)
        .padding(iced::Padding {
            top: 0.0,
            right: 14.0,
            bottom: 0.0,
            left: 4.0,
        });

        for p in &peer.playlists {
            side = side.push(entry(
                p.name.clone(),
                None,
                Source::Playlist(p.id, p.name.clone()),
                &peer.source,
            ));
        }
        side = side.push(heading("Albums"));
        for a in &peer.albums {
            side = side.push(entry(
                a.name.clone(),
                Some(a.tracks),
                Source::Album(a.name.clone()),
                &peer.source,
            ));
        }
        side = side.push(heading("Artists"));
        for a in &peer.artists {
            side = side.push(entry(
                a.name.clone(),
                Some(a.tracks),
                Source::Artist(a.name.clone()),
                &peer.source,
            ));
        }

        container(scrollable(side))
            .width(Length::Fixed(220.0))
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
        let rows = peer.rows().iter().fold(column![].spacing(4), |col, item| {
            let favorited = item.on_playlist();
            let is_playing = playing == Some(item.id);
            #[cfg_attr(feature = "demo", allow(unused_mut))]
            let mut line = Row::new()
                .spacing(12)
                .align_y(iced::Alignment::Center)
                .push(
                    // An svg rather than a character or a canvas — see
                    // `heart.rs` for why it is neither.
                    button(heart::heart(favorited))
                        .style(button::text)
                        .padding(4)
                        .on_press(Message::ToggleFavorite(item.id, favorited)),
                )
                .push(
                    button(
                        column![
                            text(item.title.clone()).size(14).style(if is_playing {
                                text::primary
                            } else {
                                text::default
                            }),
                            text(item.creator.clone()).size(12).style(text::secondary),
                        ]
                        .spacing(2),
                    )
                    .style(button::text)
                    .padding(2)
                    .width(Length::Fill)
                    .on_press(Message::PlayItem(item.id)),
                )
                .push(
                    text(clock(item.duration_ms as f64 / 1000.0))
                        .size(12)
                        .style(text::secondary),
                );
            #[cfg(not(feature = "demo"))]
            {
                line = line
                    .push(
                        // Where it sits in the playlist, which is the number
                        // that moves when someone else favourites something
                        // first.
                        text(match item.playlist_pos {
                            Some(pos) => format!("#{pos}"),
                            None => String::new(),
                        })
                        .size(12)
                        .style(text::secondary),
                    )
                    .push(
                        button("remove")
                            .style(button::text)
                            .on_press(Message::RemoveSong(item.id)),
                    );
            }
            col.push(line)
        });

        #[cfg_attr(feature = "demo", allow(unused_mut))]
        let mut main = column![row![
            text(peer.source.title().to_string()).size(22),
            text(format!("{} tracks", peer.rows().len()))
                .size(12)
                .style(text::secondary),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)]
        .spacing(12)
        .width(Length::Fill);

        #[cfg(not(feature = "demo"))]
        {
            use iced::widget::text_input;
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
            main = main.push(entry).push(actions);
        }

        main.push(
            scrollable(rows.padding(iced::Padding {
                top: 0.0,
                right: 16.0,
                bottom: 0.0,
                left: 0.0,
            }))
            .height(Length::Fill),
        )
        .push(self.view_status(peer))
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
        text(line).size(12).style(text::secondary).into()
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
        let transport = row![
            button("«").style(button::text).on_press(Message::Skip(-1)),
            button(if self.player.is_playing() {
                "❚❚"
            } else {
                "▶"
            })
            .on_press_maybe(Player::AUDIBLE.then_some(Message::PlayPause)),
            button("»").style(button::text).on_press(Message::Skip(1)),
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
