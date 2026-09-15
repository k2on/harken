//! The media directory, as a peer.
//!
//! A server that owns a folder of files has to get them into the log somehow,
//! and there are only two honest ways: write the rows directly, or be a peer
//! and author mutations like everything else. This is the second, and it is
//! not a preference — writing rows would be a second definition of what
//! "add a song" means, and the first time it disagreed with `apply` the
//! replicas would diverge with nothing to say so.
//!
//! So the scanner holds an ordinary [`Client`], with its own database and its
//! own pending queue, and reaches the hub through `Hub::exchange` instead of a
//! socket. It gets acks, it gets the rebase, and a file it adds fans out to
//! every connected peer without anyone asking — see "A peer without a socket"
//! in the engine's decisions.
//!
//! **Idempotency is not this file's doing.** `add_song` refuses a file the
//! library already has, inside `apply`, so a rescan is a no-op for every peer
//! replaying the log rather than only for the scanner that happened to author
//! it. That matters because the id is chosen fresh per authoring call: a
//! second scan produces a *different* id for the same path, and nothing here
//! could catch that on its own.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use harken::HarkenApp;
use notify::{RecursiveMode, Watcher};
use petros::{AutoCtx, Client};
use petros_auth::Login;
use petros_axum::Hub;

/// What the scanner will look at. Everything else in the directory — artwork,
/// playlists, the stray `.DS_Store` — is not a track and is skipped in silence.
const AUDIO: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "aac", "wav", "wv", "aiff", "aif",
];

/// Where music lives under the media root.
///
/// The root is kind-neutral because `file` is: one directory holds every kind,
/// `/media/` serves all of it, and an episode or a sermon gets a sibling of
/// this rather than a second root and a second URL prefix to configure. So a
/// song's path reads `music/Bach/air.wav`, and a file outside this
/// subdirectory is not a track however much it sounds like one.
const MUSIC: &str = "music";

/// The account a scanned track is authored as.
///
/// A real name would be a lie — nobody added these by hand — and no name at all
/// is not on offer: the engine holds every entry to a session, so the server
/// mints one for this. It shows up as the `user_id` on every scanned row, which
/// is the truth: the library put it there.
pub const ACCOUNT: &str = "library";

/// Why the scanner woke up.
enum Nudge {
    /// Look at everything. Sent once at boot, because whatever happened while
    /// the server was down produced no events to watch.
    All,
    /// Look at one path the watcher saw appear.
    One(PathBuf),
}

/// A handle the watcher writes to. Dropping it ends the scanner's thread.
pub struct Scanner {
    tx: Sender<Nudge>,
    /// Kept alive for as long as the scanner is: dropping a `notify` watcher
    /// stops the watch, and a watcher nobody holds stops immediately.
    _watcher: Option<notify::RecommendedWatcher>,
}

impl Scanner {
    /// Start scanning `music/` under `root`, and watching `root`.
    ///
    /// The walk is narrow and the watch is wide, on purpose: only `music/`
    /// holds tracks, but watching the root is what notices that directory
    /// being created at all — on a fresh install it may not exist yet, and a
    /// watch on a path that is not there watches nothing forever.
    ///
    /// Returns immediately: the walk happens on a thread of its own, so a
    /// library of ten thousand files does not hold up the socket.
    pub fn start(
        root: PathBuf,
        hub: Arc<Hub<HarkenApp>>,
        db: PathBuf,
        login: Login,
    ) -> Result<Scanner, Box<dyn std::error::Error>> {
        let (tx, rx) = channel();

        let worker = tx.clone();
        let walking = root.clone();
        std::thread::spawn(move || {
            if let Err(e) = run(walking, hub, db, login, rx) {
                eprintln!("harken: the library scanner stopped: {e}");
            }
        });
        worker.send(Nudge::All)?;

        // The watch is best-effort: a directory that cannot be watched is
        // still scanned at boot, and saying so is better than refusing to
        // start over it.
        let events = tx.clone();
        let watcher =
            match notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else { return };
                // Creates and writes only. A removal is not handled on purpose:
                // the log is permanent, and a file disappearing is not evidence
                // that somebody meant to delete the song — an unplugged disk looks
                // exactly the same.
                if !matches!(
                    event.kind,
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                ) {
                    return;
                }
                for path in event.paths {
                    let _ = events.send(Nudge::One(path));
                }
            }) {
                Ok(mut w) => match w.watch(&root, RecursiveMode::Recursive) {
                    Ok(()) => Some(w),
                    Err(e) => {
                        eprintln!("harken: cannot watch {}: {e}", root.display());
                        None
                    }
                },
                Err(e) => {
                    eprintln!("harken: no filesystem watch available: {e}");
                    None
                }
            };

        Ok(Scanner {
            tx,
            _watcher: watcher,
        })
    }

    /// Walk the whole directory again. Nothing calls this yet — the watch is
    /// what keeps up — but a rescan is safe at any time, which is the property
    /// the file check in `add_song` buys.
    #[allow(dead_code)]
    pub fn rescan(&self) {
        let _ = self.tx.send(Nudge::All);
    }
}

/// The scanner's whole life: a client, and a loop over what it is told to look
/// at.
fn run(
    root: PathBuf,
    hub: Arc<Hub<HarkenApp>>,
    db: PathBuf,
    login: Login,
    rx: Receiver<Nudge>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::<HarkenApp>::open(
        petros::open_path(&db)?,
        login.user.id.clone(),
        AutoCtx::system(),
    )?;
    client.set_session(Some(login.session.clone()));
    client.set_token(Some(login.token.clone()));
    let conn = hub.local();
    client.connected()?;

    let music = root.join(MUSIC);

    while let Ok(nudge) = rx.recv() {
        // What the replica already has, once per wake rather than once per
        // file. Asking is not what makes this safe — `apply` is — but it keeps
        // a rescan of ten thousand files from authoring ten thousand entries
        // for the server to refuse one at a time.
        let known = known_files(&mut client);
        let added = match nudge {
            Nudge::All => {
                let mut found = Vec::new();
                walk(&music, &mut found);
                found
                    .iter()
                    .filter(|p| offer(&mut client, &known, &root, &music, p))
                    .count()
            }
            // A file is usually still being written when the create arrives,
            // so the tags are not there yet. Settling is cheaper than a retry
            // queue and this is not a hot path.
            Nudge::One(path) => {
                std::thread::sleep(Duration::from_millis(400));
                // A *directory* appearing is the case that matters, and it is
                // not the same as a file appearing. Dropping an album folder
                // in creates the directory and its tracks in the same breath,
                // and a recursive watch has to add a watch for the new
                // directory before it can report anything inside it — so the
                // tracks land in the gap and are never announced. What is
                // announced is the directory, so that is what gets walked.
                let mut found = Vec::new();
                if path.is_dir() {
                    walk(&path, &mut found);
                } else {
                    found.push(path);
                }
                found
                    .iter()
                    .filter(|p| offer(&mut client, &known, &root, &music, p))
                    .count()
            }
        };
        if added > 0 {
            println!("harken: library +{added}");
        }
        // Pump whatever that produced, and whatever the server has for us.
        // Not conditional on `added`: a scan that adds nothing still has to
        // collect the entries other peers wrote while it was walking.
        settle(&mut client, &hub, conn);
    }
    Ok(())
}

/// Every file the library already knows about.
///
/// Read against no playlist, because membership is not the question — the
/// `file` column is, and every row has one whichever playlist it is or is not
/// on.
fn known_files(client: &mut Client<HarkenApp>) -> std::collections::HashSet<String> {
    harken::library(&mut client.store(), harken::Id::nil())
        .unwrap_or_default()
        .into_iter()
        .map(|item| item.file)
        .filter(|f| !f.is_empty())
        .collect()
}

/// Every file under `dir`, depth first. A directory that cannot be read is
/// skipped rather than fatal — a permission on one folder is not a reason to
/// index none of the others.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&path, out),
            Ok(t) if t.is_file() => out.push(path),
            _ => {}
        }
    }
}

/// Offer one path to the log. Says whether it authored anything.
///
/// Authoring is not the same as adding: `apply` refuses a file the library
/// already has, so this returns true for "a mutation was written", and the
/// second scan of the same directory writes none.
fn offer(
    client: &mut Client<HarkenApp>,
    known: &std::collections::HashSet<String>,
    root: &Path,
    music: &Path,
    path: &Path,
) -> bool {
    // The watch covers the whole media root, so this is where everything that
    // is not music gets dropped. Without it a podcast appearing under a
    // sibling directory would be indexed as a song, because it has exactly the
    // extension and exactly the tags of one.
    if !path.starts_with(music) {
        return false;
    }
    let Some(rel) = relative(root, path) else {
        return false;
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    if !ext.is_some_and(|e| AUDIO.contains(&e.as_str())) {
        return false;
    }
    if known.contains(&rel) {
        return false;
    }
    let Some(track) = read(path) else {
        return false;
    };
    match client.mutate(harken::add_song(
        track.title,
        track.artist,
        track.album,
        track.duration_ms,
        rel,
    )) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("harken: {} was refused: {e}", path.display());
            false
        }
    }
}

/// The path as the log should carry it: relative to the media root, with `/`
/// between the parts. A track therefore reads `music/Bach/air.wav`.
///
/// **Not the absolute path.** The log is permanent, so an absolute one would
/// freeze this machine's layout into it forever and break the day the
/// directory moves. Relative is also exactly what `/media/` serves, so the
/// column a client plays from and the column the scanner writes are the same
/// string.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for part in rel.components() {
        match part {
            std::path::Component::Normal(p) => parts.push(p.to_str()?.to_string()),
            // Anything that is not a plain name — a `..`, a root — is not
            // something under the media directory, whatever the path says.
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

struct Track {
    title: String,
    artist: String,
    album: String,
    duration_ms: i64,
}

/// What the tags say, with the file name as the fallback for a title.
///
/// A scanner that guessed the artist and album from the directory layout would
/// be wrong for every library not arranged the way it expected, so an untagged
/// file gets a title and nothing else rather than a confident mistake.
fn read(path: &Path) -> Option<Track> {
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::probe::Probe;
    use lofty::tag::Accessor;

    let tagged = Probe::open(path).ok()?.read().ok()?;
    let duration_ms = tagged.properties().duration().as_millis() as i64;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let text = |v: Option<std::borrow::Cow<'_, str>>| {
        v.map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_default()
    };
    let title = text(tag.and_then(|t| t.title()));
    let title = if title.is_empty() {
        path.file_stem()?.to_str()?.to_string()
    } else {
        title
    };
    Some(Track {
        title,
        artist: text(tag.and_then(|t| t.artist())),
        album: text(tag.and_then(|t| t.album())),
        duration_ms,
    })
}

/// Trade messages with the hub until there is nothing left to say.
///
/// Bounded rather than "until pending is empty": if the server refuses
/// something this peer keeps offering, a loop with no end is a spin, and one
/// that stops is a line in the log.
fn settle(client: &mut Client<HarkenApp>, hub: &Hub<HarkenApp>, conn: petros::ConnId) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let outgoing = client.take_outgoing();
        if outgoing.is_empty() || Instant::now() > deadline {
            return;
        }
        for msg in outgoing {
            for reply in hub.exchange(conn, msg) {
                if let Err(e) = client.recv(reply) {
                    eprintln!("harken: the library scanner lost its place: {e}");
                    return;
                }
            }
        }
    }
}
