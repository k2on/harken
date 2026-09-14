//! The scanner, against a real directory of real files.
//!
//! What this asserts is the property the feature exists for, not that the code
//! runs: a directory of audio becomes songs in the log, a second scan of the
//! same directory adds nothing, and a file that is not audio is not a song.
//!
//! The files are written here rather than checked in. A WAV is a header and
//! some samples, so a test can make one that `lofty` will really parse — which
//! is the part a fixture of zero bytes would not exercise.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use harken::HarkenApp;
use petros::backend::SqliteStore;
use petros_auth::{Account, Login};
use petros_axum::Hub;

/// A playable WAV of `ms` milliseconds: 44.1kHz, mono, 16-bit silence.
///
/// Silence rather than a tone because nothing here listens to it; what matters
/// is that the header is right, so `lofty` reads a duration out of it the way
/// it would from a real track.
fn wav(path: &Path, ms: u32) {
    let rate = 44_100u32;
    let samples = rate * ms / 1000;
    let data = samples * 2;
    let mut out = Vec::new();
    out.extend(b"RIFF");
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes()); // the size of this chunk
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(rate.to_le_bytes());
    out.extend((rate * 2).to_le_bytes()); // bytes per second
    out.extend(2u16.to_le_bytes()); // bytes per frame
    out.extend(16u16.to_le_bytes()); // bits per sample
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    out.extend(std::iter::repeat_n(0u8, data as usize));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, out).unwrap();
}

/// Trusting hubs ask for no token, but the scanner is an ordinary peer and
/// carries one anyway.
fn nobody() -> Login {
    Login {
        token: String::new(),
        session: String::new(),
        user: Account {
            id: "library".into(),
            name: "Library".into(),
            email: String::new(),
        },
        expires_ms: 0,
    }
}

/// What the server's own replica says the library is.
fn songs(hub: &Hub<HarkenApp>) -> Vec<(String, String)> {
    let mut server = hub.server();
    let mut store = SqliteStore::new(server.conn());
    harken::library(&mut store, harken::Id::nil())
        .unwrap_or_default()
        .into_iter()
        .map(|i| (i.title, i.file))
        .collect()
}

/// Wait for the library to stop *changing*, or give up.
///
/// Stopping rather than reaching a count, which matters for what this test is
/// for: a scanner writing duplicates would sail past "at least three" and the
/// assertion would never see them. Waiting for quiet counts what is actually
/// there.
fn settle(hub: &Hub<HarkenApp>, want: usize) -> Vec<(String, String)> {
    let until = Instant::now() + Duration::from_secs(20);
    let (mut last, mut since) = (usize::MAX, Instant::now());
    loop {
        let found = songs(hub);
        if found.len() != last {
            last = found.len();
            since = Instant::now();
        } else if found.len() >= want && since.elapsed() > Duration::from_millis(800) {
            return found;
        }
        if Instant::now() > until {
            return found;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Give the scanner room to do something wrong, then look.
///
/// The rescan is the assertion this test exists for, and it is an assertion
/// that *nothing* happens — so waiting for a change would wait forever and
/// waiting for quiet would find it before it started. A fixed pause is the
/// honest way to ask "did it write anything?": the walk it is doing is three
/// small files.
fn after(hub: &Hub<HarkenApp>, secs: u64) -> Vec<(String, String)> {
    std::thread::sleep(Duration::from_secs(secs));
    songs(hub)
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "harken-scan-{name}-{}",
        std::process::id() as u64 + name.len() as u64
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_directory_of_audio_becomes_songs_and_a_rescan_adds_none() {
    let root = temp("root");
    let music = root.join("music");
    wav(&music.join("Bach/air.wav"), 2_000);
    wav(&music.join("Beethoven/Bagatelles/fur-elise.wav"), 3_000);
    // Not audio, and not a song. A library folder is full of these.
    std::fs::write(music.join("cover.jpg"), b"not audio").unwrap();
    std::fs::write(music.join("Bach/notes.txt"), b"nor this").unwrap();

    let hub = Hub::<HarkenApp>::open(petros::open_memory().unwrap(), petros::Trusting).unwrap();
    let scanner = harken_server::library::Scanner::start(
        music.clone(),
        hub.clone(),
        root.join("scanner.db"),
        nobody(),
    )
    .unwrap();

    let found = settle(&hub, 2);
    let mut files: Vec<&str> = found.iter().map(|(_, f)| f.as_str()).collect();
    files.sort();
    assert_eq!(
        files,
        vec!["Bach/air.wav", "Beethoven/Bagatelles/fur-elise.wav"],
        "every audio file, by its path relative to the music root — which is \
         the same string `/media/` serves back"
    );
    assert_eq!(found.len(), 2, "the jpg and the txt are not songs");

    // A file appearing after the scan, without anyone asking for a rescan.
    wav(&music.join("Debussy/clair-de-lune.wav"), 1_000);
    assert_eq!(settle(&hub, 3).len(), 3, "the watch picked up a new file");

    // Now the point: scan the whole directory again, over the same log.
    // Every id is fresh, so nothing but the file check in `apply` can stop
    // these becoming three more songs.
    drop(scanner);
    let again = harken_server::library::Scanner::start(
        music.clone(),
        hub.clone(),
        root.join("scanner-again.db"),
        nobody(),
    )
    .unwrap();
    assert_eq!(
        after(&hub, 4).len(),
        3,
        "a second scan of the same directory is a no-op \u{2014} every id it \
         authors is fresh, so only the file check in `apply` can stop these \
         becoming three more songs"
    );
    drop(again);
    let _ = std::fs::remove_dir_all(&root);
}
