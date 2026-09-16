//! Every mutation and every query, each written once.
//!
//! A mutation is an *intent* — "put this at the end of the playlist", not "put
//! this at position 3" — and it is applied by every replica from the log, not
//! here. The body below runs on the server, in the iced client, and inside a
//! wasm sandbox on the phone, from the same source; `tests/conformance.rs`
//! drives every verb through both builds and compares the rows.
//!
//! # What the parameters mean
//!
//! `&mut Db` is the store. `NewId` and `Now` are the only non-determinism a
//! mutation gets, chosen once at the originating client and frozen in the log —
//! a body may not call a clock or invent an id, because two replicas would
//! choose differently and diverge. `ctx: &Ctx` is who authored the entry
//! (`ctx.user.id`, which the server verified) and under which login
//! (`ctx.session.id`). Which is which is decided by type, so there is no list
//! to keep in step.
//!
//! # There is no SQL here
//!
//! Reads and writes are the same shape: `db.select(query)` and `db.put(&row)`,
//! over row types generated from `schema.sql`. A misspelled column is a compile
//! error rather than a missing row on a device, and a write says what it
//! changed — which is what an incrementally maintained query will need.

// A mutation's arguments are the wire format: each one is a field in the
// log, and every verb here is generated into several functions — the
// authoring call, the dispatch arm, the foreign export. So the lint fires
// on code nobody wrote, and the refactor it asks for (bundle them into a
// struct) would put a second shape between a caller and the entry, which
// is the thing `mutations.txt` exists to pin down.
#![allow(clippy::too_many_arguments)]

use petros_schema::prelude::*;

// `#[mutation]` and `#[query]` each emit a method on the peer a foreign caller
// talks to. UniFFI will not take a qualified self-type, so the name has to be
// in scope here rather than in what they generate.
#[cfg(feature = "foreign")]
use crate::Peer;

use crate::schema::tables::{
    Album, Credit, Media, Movement, Person, Playlist, PlaylistItem, Recording, Song, Work,
};

// Only the queries below use these, and a query is not built for the sandbox.
#[cfg(feature = "storage")]
use crate::schema::Item;
// Grouping a sidebar's albums and artists, and the order an album's tracks
// come out in. An ordered map, so the lists come out in a stable order without
// a sort — two peers showing the same library show it the same way.
#[cfg(feature = "storage")]
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------- keys
//
// `album`, `person`, `work`, `movement` and `recording` are all keyed by
// something derived from what they *are*, never by a minted id, and the reason
// is mechanical: `fill_auto` hands a mutation exactly one uuid, so no verb can
// mint a work and a song in the same entry — and splitting it into two verbs
// would have a client authoring a reference to a row it has not seen
// confirmed. Two peers each adding "BWV 988" offline would author two rows
// with two ids, one of which loses the rebase, taking every track pointing at
// it into a dangling reference.
//
// Classical is the one genre where that costs nothing, because a catalogue
// number is exactly a name every peer agrees on without being told — which is
// what catalogue numbers are *for*. The rest follows from there.
//
// These are keys and never appear on a screen. They are also permanent: change
// one of these functions and every replica computes a different key for the
// same music, so a rebuild from the log lands in different rows.

/// One name, reduced to something that can stand in a key.
///
/// Letters and digits survive lowercased, everything else becomes a single
/// dash. Non-ASCII letters are *kept* rather than folded: "Dvořák" and "Dvorak"
/// are two keys, which is the same answer `person.name` already gives, and
/// guessing that they are one person is a guess.
fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// …and the same, for a name that has no letters in it at all.
///
/// "!!!" is a band and `slug` answers "" for it, which would put every such
/// name in one key. The hash is not pretty and does not have to be: nothing
/// reads a key.
fn key_part(text: &str) -> String {
    let out = slug(text);
    if !out.is_empty() {
        return out;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.trim().as_bytes() {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("x{h:016x}")
}

/// The key of a work: its composer, and the name that survives translation.
///
/// The catalogue number when there is one, because that is the only stable
/// name a work has — titles are translated, transliterated and abbreviated
/// differently by every publisher, and `BWV 988` is `BWV 988` everywhere. The
/// composer is in the key because catalogue numbers are per-composer: `BWV` is
/// Bach's alone, but `Op. 23` belongs to everybody.
pub fn work_key(composer: &str, catalogue: &str, title: &str) -> String {
    let named = match catalogue.trim().is_empty() {
        true => title,
        false => catalogue,
    };
    format!("{}/{}", key_part(composer), key_part(named))
}

/// The key of a movement: its work, and where it sits in it.
pub fn movement_key(work_id: &str, no: i64) -> String {
    format!("{work_id}#{no}")
}

/// The key of a recording: what it is a recording *of*, and by whom.
///
/// `of` is the work's key where there is a work, and where there is not — most
/// music — it is the release and the title, which makes a pop track its own
/// recording. That is right rather than a fallback: one pop track *is* one
/// performance, and the row exists so that `credit` can be the one answer to
/// "who played this" whatever the genre.
///
/// It collides when the same people record the same work twice, which is rare
/// and would want the year in the key. Nothing here does that yet.
pub fn recording_key(of: &str, who: &str) -> String {
    format!("{of}@{}", key_part(who))
}

// ------------------------------------------------------------------ mutations

/// Put a song in the library.
///
/// Two rows: what every playable thing has, and what only a song has. A second
/// kind is another verb beside this one writing `media` the same way and its
/// own side table — nothing here changes for it.
#[mutation]
pub fn add_song(
    db: &mut Db,
    ctx: &Ctx,
    id: NewId<Media>,
    added_ms: Now,
    title: String,
    artist: String,
    album: String,
    duration_ms: i64,
    file: String,
    // Everything below was added after the log had entries in it. That is
    // allowed and is why the arguments are at the end: a payload written
    // before they existed decodes with each at its default, so an old entry
    // replays as a song with no track number and no catalogue — which is
    // exactly what it was.
    track: i64,
    part: String,
    catalogue: String,
    performer: String,
    bpm: i64,
    // The covers, which arrive with the song rather than through a verb of
    // their own. There was a `set_artwork` here and it is gone: an album and
    // an artist are rows now, so a picture has something to be a column *on*,
    // and the thing that knows a track's album is the entry that adds the
    // track. Empty is "nobody said" and leaves whatever is already there —
    // see `name_art`, which is where that rule is.
    album_art: String,
    artist_art: String,
    // …and these, added when an album and an artist stopped being the only
    // things a track belonged to. An entry written before them decodes with
    // each at its default, and the two rules just below say what an old entry
    // meant — which is the whole reason arguments are only ever appended.
    disc: i64,
    // The composition, where the track is of one. Empty is pop: no work, no
    // movement, and a recording of its own.
    work_title: String,
    // Where it sits in the *work*, which is not where it sits on the release.
    movement_no: i64,
) -> Result {
    if title.trim().is_empty() {
        return Err("a song needs a title".into());
    }
    // The same entry arriving twice is a no-op, which is what makes redelivery
    // safe.
    if db.exists::<Media>(&Media::key_of(&id)) {
        return Ok(());
    }
    // And the same *file* arriving twice is a no-op too, which is what makes
    // rescanning safe.
    //
    // This has to be here rather than in whatever is scanning, because `id` is
    // chosen fresh at the originating client: a second scan authors a new id
    // for a file the library already has, and nothing above would catch it.
    // Deciding it inside `apply` means every peer replaying the log reaches
    // the same answer — the first entry for a path wins, wherever the rescan
    // happened. An empty `file` is not a path and does not collide: a song
    // typed in by hand has no file, and two of those are two songs.
    let file = file.trim().to_string();
    if !file.is_empty()
        && !db
            .select(Media::all().filter(Media::file.eq(file.clone())))
            .is_empty()
    {
        return Ok(());
    }
    // `pos` is read out of current state: an intent, not a fact. It is what
    // makes the rebase visible when an entry lands underneath yours.
    let last = last_pos(db);
    db.put(&Media {
        id,
        kind: crate::schema::kind::SONG.to_string(),
        title: title.trim().to_string(),
        creator: artist.trim().to_string(),
        duration_ms,
        file,
        pos: last + 1,
        added_ms,
        user_id: ctx.user.id.clone(),
    })?;
    // Everything a track belongs to, as rows. Written here and not by verbs of
    // their own, because the key is derived: `add_song` naming a work that
    // already exists is one row, from any peer, in any order, with nothing to
    // reconcile. A verb that minted an id could not say that.
    let album = album.trim().to_string();
    let artist = artist.trim().to_string();
    let performer = performer.trim().to_string();
    let catalogue = catalogue.trim().to_string();
    let title = title.trim().to_string();

    // **How an entry written before any of this replays.** `work_title` decodes
    // empty for every entry already in a log, and reading that as "no work"
    // would throw away what those entries plainly say.
    //
    // Two things in an old entry say there is a work. A **catalogue number** is
    // one: nobody catalogues a track. A **part** is the other, and it is the one
    // that is easy to miss — `song.part` was "the division of the work this
    // belongs to", so a track that has one is a track of a work by the
    // definition of the column. Missing that second case silently dropped the
    // part of every uncatalogued suite, which is a whole album's grouping gone,
    // and `an_album_is_in_the_works_order_and_not_the_librarys` is what said so.
    //
    // The work's *name* is then the album's, because that is how this library
    // was authored: one record per work. `movement_no` reads as the track
    // number for the same reason — nothing here has ever put a work's movements
    // out of order on its own record. Both are readings of old entries rather
    // than defaults, which is why they are here and not at a call site.
    let of_a_work = !catalogue.is_empty() || !part.trim().is_empty();
    let work_title = match work_title.trim() {
        "" if of_a_work => album.as_str(),
        other => other,
    }
    .to_string();
    let movement_no = match movement_no {
        0 => track,
        n => n,
    };

    if !album.is_empty() {
        let have = db.select(Album::all().filter(Album::name.eq(album.clone())));
        if let Some(art) = art_to_write(have.first().map(|a| a.art.as_str()), &album_art) {
            db.put(&Album {
                name: album.clone(),
                label: have.first().map(|a| a.label.clone()).unwrap_or_default(),
                released: have.first().map(|a| a.released).unwrap_or_default(),
                art,
                added_ms,
                user_id: ctx.user.id.clone(),
            })?;
        }
    }
    if !artist.is_empty() {
        touch_person(db, ctx, added_ms, &artist, &artist_art)?;
    }

    // The work and the movement, when the track is of a named composition.
    // A composer is needed for the work's foreign key, and `media.creator` is
    // the composer for this repertoire — which is what the demo has always
    // passed in `artist`.
    let work_id = match work_title.is_empty() || artist.is_empty() {
        true => None,
        false => {
            let id = work_key(&artist, &catalogue, &work_title);
            let have = db.select(Work::all().filter(Work::id.eq(id.clone())));
            let row = Work {
                id: id.clone(),
                composer: artist.clone(),
                title: work_title.clone(),
                catalogue: catalogue.clone(),
                opus: filled(have.first().map(|w| w.opus.as_str()), ""),
                key_sig: filled(have.first().map(|w| w.key_sig.as_str()), ""),
                form: filled(have.first().map(|w| w.form.as_str()), ""),
                period: filled(have.first().map(|w| w.period.as_str()), ""),
                composed: have.first().map(|w| w.composed).unwrap_or_default(),
                art: filled(have.first().map(|w| w.art.as_str()), ""),
                added_ms: have.first().map(|w| w.added_ms).unwrap_or(added_ms),
                user_id: have
                    .first()
                    .map(|w| w.user_id.clone())
                    .unwrap_or_else(|| ctx.user.id.clone()),
            };
            if have.first() != Some(&row) {
                db.put(&row)?;
            }
            Some(id)
        }
    };
    let movement_id = match &work_id {
        None => None,
        Some(work_id) => {
            let id = movement_key(work_id, movement_no);
            let have = db.select(Movement::all().filter(Movement::id.eq(id.clone())));
            let row = Movement {
                id: id.clone(),
                work_id: work_id.clone(),
                no: movement_no,
                title: title.clone(),
                part: part.trim().to_string(),
                added_ms: have.first().map(|m| m.added_ms).unwrap_or(added_ms),
                user_id: have
                    .first()
                    .map(|m| m.user_id.clone())
                    .unwrap_or_else(|| ctx.user.id.clone()),
            };
            if have.first() != Some(&row) {
                db.put(&row)?;
            }
            Some(id)
        }
    };

    // The performance. Every track has one — see `recording` in `schema.sql`
    // for why that is not optional.
    let of = match &work_id {
        Some(work_id) => work_id.clone(),
        None => format!("{}/{}", key_part(&album), key_part(&title)),
    };
    let who = match performer.is_empty() {
        true => artist.clone(),
        false => performer.clone(),
    };
    let recording_id = recording_key(&of, &who);
    let have = db.select(Recording::all().filter(Recording::id.eq(recording_id.clone())));
    if have.is_empty() {
        db.put(&Recording {
            id: recording_id.clone(),
            work_id: work_id.clone(),
            recorded: 0,
            venue: String::new(),
            label: String::new(),
            licence: String::new(),
            art: String::new(),
            added_ms,
            user_id: ctx.user.id.clone(),
        })?;
    }

    // Who played it. One credit, because a track carries one lumped string and
    // splitting "London Symphony Orchestra, Hermann Scherchen" into two people
    // with two roles is a guess — `credit_recording` is how a caller says it
    // properly. A classical track that names no performer gets no credit, which
    // is more honest than crediting the composer with the performance.
    let (who, role) = match (performer.is_empty(), work_id.is_some()) {
        (false, true) => (performer.clone(), "performer"),
        (false, false) => (performer.clone(), "artist"),
        (true, false) => (artist.clone(), "artist"),
        (true, true) => (String::new(), ""),
    };
    if !who.is_empty() {
        touch_person(db, ctx, added_ms, &who, "")?;
        if !db.exists::<Credit>(&Credit::key_of(&recording_id, &who, &role.to_string())) {
            db.put(&Credit {
                recording_id: recording_id.clone(),
                person_name: who,
                role: role.to_string(),
                instrument: String::new(),
                // **0 is what makes this a fallback rather than a credit.** It
                // is one string a track carried, and "London Symphony
                // Orchestra, Hermann Scherchen" is two people with two roles —
                // this row is only standing in until somebody says which. A
                // real credit starts at 1, so `performers_of` can prefer them
                // and ignore this one. Without that the two would be read
                // together and every properly credited recording would list
                // its orchestra twice, once inside the lumped string.
                pos: 0,
                added_ms,
                user_id: ctx.user.id.clone(),
            })?;
        }
    }

    db.put(&Song {
        media_id: id,
        // `None`, not `""`: there is a foreign key on this column, so a song
        // with no record has nothing to point at rather than a row with no
        // name to point at.
        album_name: match album.is_empty() {
            true => None,
            false => Some(album),
        },
        // Everything is on disc 1 unless somebody says otherwise, and an entry
        // written before this argument existed says nothing.
        disc: match disc {
            n if n > 0 => n.min(99),
            _ => 1,
        },
        // Negatives are not a position in an album, and neither is a number
        // past any plausible one: clamped rather than refused, because a bad
        // track number is not a reason to lose the recording.
        track: track.clamp(0, 999),
        recording_id,
        movement_id,
        // The slowest marking anybody writes is around 20 and the fastest
        // around 300; outside that it is not a tempo.
        bpm: if (20..=300).contains(&bpm) { bpm } else { 0 },
    })?;
    Ok(())
}

/// Make sure there is a `person` row for this name, and give it a picture.
///
/// Every table that names somebody has a foreign key into `person`, so this is
/// what keeps `credit` and `work.composer` writable from a verb that was handed
/// a name and nothing else. The picture follows `art_to_write`; the rest of the
/// row is what `describe_person` is for.
fn touch_person(db: &mut impl Store, ctx: &Ctx, added_ms: Now, name: &str, art: &str) -> Result {
    let have = db.select(Person::all().filter(Person::name.eq(name.to_string())));
    let Some(art) = art_to_write(have.first().map(|p| p.art.as_str()), art) else {
        return Ok(());
    };
    db.put(&Person {
        name: name.to_string(),
        sort_name: have
            .first()
            .map(|p| p.sort_name.clone())
            .unwrap_or_default(),
        born: have.first().map(|p| p.born).unwrap_or_default(),
        died: have.first().map(|p| p.died).unwrap_or_default(),
        art,
        added_ms: have.first().map(|p| p.added_ms).unwrap_or(added_ms),
        user_id: have
            .first()
            .map(|p| p.user_id.clone())
            .unwrap_or_else(|| ctx.user.id.clone()),
    })?;
    Ok(())
}

/// What a `describe_*` verb puts in a text field: the new value, or the old one
/// when the entry offered nothing.
///
/// The same rule as `art_to_write` and for the same reason — an entry that says
/// nothing about a field must not erase what another entry said about it, which
/// is the case a rescan is in on every pass.
fn filled(current: Option<&str>, incoming: &str) -> String {
    match incoming.trim() {
        "" => current.unwrap_or_default().to_string(),
        given => given.to_string(),
    }
}

/// …and the same for a number, where 0 is "nobody said".
fn filled_num(current: Option<i64>, incoming: i64) -> i64 {
    match incoming {
        0 => current.unwrap_or_default(),
        given => given,
    }
}

/// What to put in a row's `art`, or `None` to leave the row exactly as it is.
///
/// One rule, here rather than twice in `add_song`, because an album and an
/// artist follow the same one and two copies of it would eventually be two
/// rules:
///
/// - **No row yet — make one**, carrying whatever this entry brought. Often
///   that is nothing, and a row with an empty `art` is right: the row is the
///   thing a song points at, and having no cover is a fact about the album
///   rather than a reason not to have the album.
/// - **A row already there — a picture replaces a picture, and nothing
///   replaces nothing.** This is the one last-write-wins rule in the file, and
///   it is deliberate: `add_song` lets the *first* entry win on a file, because
///   adding a song twice is a mistake, but somebody who picks a better cover
///   means the newer one. The log is totally ordered, so "newer" is a fact
///   every replica reaches the same way.
/// - **The same picture again writes nothing**, which is what keeps a rescan
///   of a thousand tracks a thousand reads and no writes — a `put` would move
///   `added_ms` and report a change to every maintained view watching.
///
/// Not behind `storage`: this runs inside `apply`, which is exactly the build
/// that has no read model.
fn art_to_write(current: Option<&str>, incoming: &str) -> Option<String> {
    let incoming = incoming.trim();
    match current {
        None => Some(incoming.to_string()),
        Some(have) if incoming.is_empty() || have == incoming => None,
        Some(_) => Some(incoming.to_string()),
    }
}

/// Say more about a work than the track that created it could.
///
/// A work row appears because a song named it — there is no `create_work`, for
/// the reason in `schema.sql` — so this **refuses a work that is not there**
/// rather than making one: a work with an id and no composer would break the
/// foreign key that makes it a work at all, and a caller who has the detail but
/// not the music is describing something this library does not have.
///
/// Every field is fill-if-given: an empty string and a 0 leave what is already
/// there, so a scanner that learns the period but not the key does not erase
/// the key somebody else supplied.
#[mutation]
pub fn describe_work(
    db: &mut Db,
    added_ms: Now,
    id: String,
    opus: String,
    key_sig: String,
    form: String,
    period: String,
    composed: i64,
    art: String,
) -> Result {
    let have = db.select(Work::all().filter(Work::id.eq(id.clone())));
    let Some(work) = have.first() else {
        return Err(format!(
            "no work {id}; a work exists because a song named it"
        ));
    };
    let row = Work {
        opus: filled(Some(&work.opus), &opus),
        key_sig: filled(Some(&work.key_sig), &key_sig),
        form: filled(Some(&work.form), &form),
        period: filled(Some(&work.period), &period),
        composed: filled_num(Some(work.composed), composed),
        art: filled(Some(&work.art), &art),
        added_ms,
        ..work.clone()
    };
    if row != *work {
        db.put(&row)?;
    }
    Ok(())
}

/// Say more about a performance than the track that created it could.
///
/// Refuses an unknown recording, for the reason `describe_work` does.
#[mutation]
pub fn describe_recording(
    db: &mut Db,
    added_ms: Now,
    id: String,
    recorded: i64,
    venue: String,
    label: String,
    licence: String,
    art: String,
) -> Result {
    let have = db.select(Recording::all().filter(Recording::id.eq(id.clone())));
    let Some(rec) = have.first() else {
        return Err(format!(
            "no recording {id}; one exists because a song is part of it"
        ));
    };
    let row = Recording {
        recorded: filled_num(Some(rec.recorded), recorded),
        venue: filled(Some(&rec.venue), &venue),
        label: filled(Some(&rec.label), &label),
        licence: filled(Some(&rec.licence), &licence),
        art: filled(Some(&rec.art), &art),
        added_ms,
        ..rec.clone()
    };
    if row != *rec {
        db.put(&row)?;
    }
    Ok(())
}

/// Say more about somebody than the track that named them could.
///
/// This one *does* make the row when it is not there, unlike the two above, and
/// the difference is the foreign key: a person row needs nothing but a name, so
/// there is nothing to be missing. Somebody can be described before any of
/// their music arrives.
#[mutation]
pub fn describe_person(
    db: &mut Db,
    ctx: &Ctx,
    added_ms: Now,
    name: String,
    sort_name: String,
    born: i64,
    died: i64,
    art: String,
) -> Result {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("a person needs a name".into());
    }
    let have = db.select(Person::all().filter(Person::name.eq(name.clone())));
    let old = have.first();
    let row = Person {
        name,
        sort_name: filled(old.map(|p| p.sort_name.as_str()), &sort_name),
        born: filled_num(old.map(|p| p.born), born),
        died: filled_num(old.map(|p| p.died), died),
        art: filled(old.map(|p| p.art.as_str()), &art),
        added_ms: old.map(|p| p.added_ms).unwrap_or(added_ms),
        user_id: old
            .map(|p| p.user_id.clone())
            .unwrap_or_else(|| ctx.user.id.clone()),
    };
    if old != Some(&row) {
        db.put(&row)?;
    }
    Ok(())
}

/// Credit somebody on a recording, properly.
///
/// `add_song` writes one credit from the one lumped string a track carries;
/// this is how "London Symphony Orchestra, Hermann Scherchen" becomes an
/// orchestra and a conductor, which is what makes either of them browsable.
///
/// Keyed by the three of them, so one person can hold two roles on one
/// recording — Bernstein conducting and playing — and so a second entry for the
/// same person in the same role is a correction rather than a duplicate.
#[mutation]
pub fn credit_recording(
    db: &mut Db,
    ctx: &Ctx,
    added_ms: Now,
    recording_id: String,
    person_name: String,
    role: String,
    instrument: String,
    pos: i64,
) -> Result {
    let person_name = person_name.trim().to_string();
    if person_name.is_empty() {
        return Err("a credit needs somebody to credit".into());
    }
    // Refused rather than ignored, and refused *here* rather than by the
    // foreign key, because the key's message names a constraint and this one
    // names the mistake.
    if !db.exists::<Recording>(&Recording::key_of(&recording_id)) {
        return Err(format!("no recording {recording_id} to credit anybody on"));
    }
    let role = match role.trim() {
        "" => "artist",
        given => given,
    }
    .to_string();
    touch_person(db, ctx, added_ms, &person_name, "")?;
    let have = db.select(
        Credit::all()
            .filter(Credit::recording_id.eq(recording_id.clone()))
            .filter(Credit::person_name.eq(person_name.clone()))
            .filter(Credit::role.eq(role.clone())),
    );
    let row = Credit {
        recording_id,
        person_name,
        role,
        instrument: filled(have.first().map(|c| c.instrument.as_str()), &instrument),
        // At least 1, because anything written here is somebody saying who
        // played: 0 is reserved for the lumped string `add_song` falls back to.
        // A caller with no opinion about billing gets 1 rather than 0.
        pos: filled_num(have.first().map(|c| c.pos), pos).max(1),
        added_ms,
        user_id: ctx.user.id.clone(),
    };
    if have.first() != Some(&row) {
        db.put(&row)?;
    }
    Ok(())
}

/// Make a playlist.
#[mutation]
pub fn create_playlist(
    db: &mut Db,
    ctx: &Ctx,
    id: NewId<Playlist>,
    created_ms: Now,
    name: String,
) -> Result {
    if name.trim().is_empty() {
        return Err("a playlist needs a name".into());
    }
    if db.exists::<Playlist>(&Playlist::key_of(&id)) {
        return Ok(());
    }
    // And the same *name* from the same person is a no-op too, which is what
    // makes a second device safe.
    //
    // Every client makes a default playlist on its first run, and it has to do
    // that before it has seen the log — the list is empty because nothing has
    // synced yet, not because nobody has one. So a person signing in on a
    // phone, a laptop and a browser tab authored three "Favorites", and they
    // all landed.
    //
    // It has to be decided here rather than in the clients, for the reason
    // `add_song`'s file check does: `id` is chosen fresh per authoring call, so
    // the second device authors a *different* id for a name the log already
    // has and nothing above could catch it. Inside `apply`, every peer
    // replaying reaches the same answer — the first entry for a (person, name)
    // wins, wherever it was authored.
    //
    // By person, because the log is one library and several people may be in
    // it: Bob's "Favorites" is not Alice's, and a rule that looked only at the
    // name would leave whoever signed in second without one.
    let name = name.trim().to_string();
    if !db
        .select(
            Playlist::all()
                .filter(Playlist::user_id.eq(ctx.user.id.clone()))
                .filter(Playlist::name.eq(name.clone())),
        )
        .is_empty()
    {
        return Ok(());
    }
    let last = db
        .select(Playlist::all().order_by(Playlist::pos.desc()).limit(1))
        .first()
        .map(|p| p.pos)
        .unwrap_or(0);
    db.put(&Playlist {
        id,
        name,
        pos: last + 1,
        created_ms,
        user_id: ctx.user.id.clone(),
    })?;
    Ok(())
}

/// Put something on a playlist, at the end of it.
///
/// A playlist holds an item once — it is a set with an order — so adding one
/// already there keeps the place it first got. Adding to a playlist that is
/// gone, or an item that is gone, is a no-op rather than an error: an entry
/// earlier in the log may have removed either.
#[mutation]
pub fn add_to_playlist(
    db: &mut Db,
    ctx: &Ctx,
    added_ms: Now,
    playlist_id: Id<Playlist>,
    media_id: Id<Media>,
) -> Result {
    if !db.exists::<Playlist>(&Playlist::key_of(&playlist_id)) {
        return Ok(());
    }
    if !db.exists::<Media>(&Media::key_of(&media_id)) {
        return Ok(());
    }
    if db.exists::<PlaylistItem>(&PlaylistItem::key_of(&playlist_id, &media_id)) {
        return Ok(());
    }
    let last = last_playlist_pos(db, playlist_id);
    db.put(&PlaylistItem {
        playlist_id,
        media_id,
        pos: last + 1,
        added_ms,
        user_id: ctx.user.id.clone(),
    })?;
    Ok(())
}

/// Take something off a playlist. The item itself stays in the library.
#[mutation]
pub fn remove_from_playlist(db: &mut Db, playlist_id: Id<Playlist>, media_id: Id<Media>) -> Result {
    db.delete::<PlaylistItem>(&PlaylistItem::key_of(&playlist_id, &media_id))?;
    Ok(())
}

/// Put everything in the library on a playlist.
///
/// One entry rather than one per item, and that is the point: it is an
/// *intent*, so a replica replaying it covers whatever else was in the library
/// by then — including items another peer added while this was in flight. A
/// client sending N `AddToPlaylist` entries instead would freeze the list as
/// it looked when the button was pressed.
///
/// It was one `INSERT ... SELECT` with a window function when this was SQL. It
/// is a loop now, and that is the price of a write that says what it changed:
/// a statement that inserts a thousand rows produces one result and no record
/// of which rows they were, which is exactly what an incremental view cannot
/// work from.
#[mutation]
pub fn add_all_to_playlist(
    db: &mut Db,
    ctx: &Ctx,
    added_ms: Now,
    playlist_id: Id<Playlist>,
) -> Result {
    if !db.exists::<Playlist>(&Playlist::key_of(&playlist_id)) {
        return Ok(());
    }
    let mut pos = last_playlist_pos(db, playlist_id);
    // Ordered, because the positions it assigns go into the log and every
    // replica has to assign the same ones.
    let items = db.select(
        Media::all()
            .order_by(Media::pos.asc())
            .order_by(Media::id.asc()),
    );
    for item in items {
        if db.exists::<PlaylistItem>(&PlaylistItem::key_of(&playlist_id, &item.id)) {
            continue;
        }
        pos += 1;
        db.put(&PlaylistItem {
            playlist_id,
            media_id: item.id,
            pos,
            added_ms,
            user_id: ctx.user.id.clone(),
        })?;
    }
    Ok(())
}

/// Take something out of the library, and off every playlist holding it.
#[mutation]
pub fn remove_media(db: &mut Db, id: Id<Media>) -> Result {
    let on = db.select(
        PlaylistItem::all()
            .filter(PlaylistItem::media_id.eq(id))
            .order_by(PlaylistItem::playlist_id.asc()),
    );
    for item in on {
        db.delete::<PlaylistItem>(&PlaylistItem::key_of(&item.playlist_id, &item.media_id))?;
    }
    db.delete::<Song>(&Song::key_of(&id))?;
    db.delete::<Media>(&Media::key_of(&id))?;
    Ok(())
}

// The two helpers take `&mut impl Store` rather than `&mut Db`. Inside a
// `#[mutation]` the store is a type parameter — the same body runs over SQLite
// natively and over the host's store through the sandbox ABI — and `Db` is only
// the marker that stands for it in a signature the attribute rewrites.

/// The end of the library, which is where anything new goes — one order across
/// every kind, because the library is one list.
fn last_pos(db: &mut impl Store) -> i64 {
    db.select(Media::all().order_by(Media::pos.desc()).limit(1))
        .first()
        .map(|s| s.pos)
        .unwrap_or(0)
}

/// The end of one playlist. `MAX(pos)` within it, as a query that reads one row.
fn last_playlist_pos(db: &mut impl Store, playlist: Id<Playlist>) -> i64 {
    db.select(
        PlaylistItem::all()
            .filter(PlaylistItem::playlist_id.eq(playlist))
            .order_by(PlaylistItem::pos.desc())
            .limit(1),
    )
    .first()
    .map(|i| i.pos)
    .unwrap_or(0)
}

// -------------------------------------------------------------------- queries

/// The whole library, in the order things were added — every kind, one list.
///
/// `ORDER BY` is explicit here as everywhere: SQLite's natural order is not a
/// contract, and two peers showing the same rows in different orders is a bug
/// that only appears on someone else's machine.
#[query]
pub fn library(db: &mut Db, playlist_id: Id<Playlist>) -> Result<Vec<Item>> {
    // An item *with* its favorite, which is a tree rather than a join: one
    // that is not favorited is still a row, carrying nothing. That is the LEFT
    // JOIN, and it is the relationship's shape rather than a keyword.
    //
    // One table, whatever the kind. A screen that lists the library never joins
    // a side table, which is the point of `media` carrying what a list needs.
    let rows = db.select_with(library_query(), Media::playlist_item, items_on(playlist_id));
    Ok(rows.iter().map(item_of).collect())
}

/// The query `library` answers, written once so that running it and maintaining
/// it cannot drift apart.
#[cfg(feature = "storage")]
fn library_query() -> petros_schema::Query<Media> {
    Media::all()
        .order_by(Media::pos.asc())
        .order_by(Media::id.asc())
}

/// One playlist's entries, as a child pipeline.
///
/// Filtered to a single playlist, so a media row carries at most the one entry
/// saying where it sits on it — which is what a heart draws. There is no
/// favorites table and no favorites verb: a heart means "this is on the
/// playlist I am showing you", and which playlist that is belongs to the
/// client, not to the domain.
#[cfg(feature = "storage")]
fn items_on(playlist: Id<Playlist>) -> petros_schema::Query<PlaylistItem> {
    PlaylistItem::all().filter(PlaylistItem::playlist_id.eq(playlist))
}

/// `library`, maintained rather than re-run.
///
/// A client holds one of these and hands it what each mutation changed, instead
/// of reading the whole list back on every frame. Same query, same rows, same
/// order — [`library_query`] is the single definition of all three.
#[cfg(feature = "storage")]
pub type LibraryView = petros::ivm::View<Media>;

/// Build one. Hydrate it once against a store, then feed it
/// `Client::take_changes()`.
#[cfg(feature = "storage")]
pub fn library_view(playlist: Id<Playlist>) -> LibraryView {
    petros::ivm::View::related(library_query(), Media::playlist_item, items_on(playlist))
}

/// Read a maintained view the way `library` reads a fetched one.
///
/// Every row, decoded. For the steady state prefer [`patch`], which touches
/// only the rows that moved — this one is for a fresh hydrate.
#[cfg(feature = "storage")]
pub fn items_of(view: &LibraryView) -> Vec<Item> {
    view.with::<PlaylistItem>().iter().map(item_of).collect()
}

/// How many songs are on the playlist, maintained.
///
/// A screen shows this beside the list and would otherwise recount it on every
/// frame. A tally holds the number rather than the rows, so it costs nothing at
/// any library size — and it is the *favorites* that are counted, so it reads
/// the playlist table rather than filtering songs.
#[cfg(feature = "storage")]
pub type PlaylistCount = petros::ivm::Tally;

#[cfg(feature = "storage")]
pub fn playlist_count(playlist: Id<Playlist>) -> PlaylistCount {
    petros::ivm::Tally::of(items_on(playlist))
}

/// Bring a list a screen holds up to date with what a view just did.
///
/// Maintaining the query and then decoding every row again is still O(n), and
/// past a few hundred songs that decode is most of what is left. These are the
/// entries that moved, in the order they moved, so the rest are not touched.
#[cfg(feature = "storage")]
pub fn patch(items: &mut Vec<Item>, patches: &[petros::ivm::Patch]) {
    for change in patches {
        match change {
            petros::ivm::Patch::Insert { at, node } => {
                if let Some(item) = node.decode::<Media, PlaylistItem>().as_ref().map(item_of) {
                    items.insert(*at, item);
                }
            }
            petros::ivm::Patch::Remove { at } => {
                items.remove(*at);
            }
            petros::ivm::Patch::Update { at, node } => {
                if let Some(item) = node.decode::<Media, PlaylistItem>().as_ref().map(item_of) {
                    items[*at] = item;
                }
            }
        }
    }
}

/// Every album in the library, by name.
///
/// Grouped here rather than in SQL because the query builder has no GROUP BY
/// and should not grow one for this: the rows are already being read, and a
/// sidebar is built when a selection changes rather than on every frame.
#[query]
pub fn albums(db: &mut Db) -> Result<Vec<crate::schema::Album>> {
    // A song carries its album; the media row beside it carries who made it.
    // Reading from the song end is what makes this song-shaped: a kind with no
    // albums contributes nothing and needs no case here.
    let rows = db.select_with(Song::all(), Song::media, Media::all());
    let mut by_name: BTreeMap<String, (String, i64)> = BTreeMap::new();
    for row in &rows {
        let Some(media) = row.one() else { continue };
        // A song on no record contributes to no album, which is what the
        // nullable column buys: this used to group every album-less song under
        // one entry named `""` and draw it as a card with no title.
        let Some(name) = &row.row.album_name else {
            continue;
        };
        let entry = by_name
            .entry(name.clone())
            .or_insert_with(|| (media.creator.clone(), 0));
        entry.1 += 1;
    }
    // Which albums there *are* is still the songs' answer, not this table's: a
    // row here with nothing pointing at it would be an album with no tracks,
    // which is not something to put in a sidebar. What the table is asked for
    // is the cover — and in one read rather than a point lookup per album,
    // because two hundred albums would otherwise be two hundred of those to
    // draw a sidebar, and these rows are small.
    let mut art: BTreeMap<String, String> = db
        .select(Album::all())
        .into_iter()
        .map(|a| (a.name, a.art))
        .collect();
    Ok(by_name
        .into_iter()
        .map(|(name, (creator, tracks))| crate::schema::Album {
            art: art.remove(&name).unwrap_or_default(),
            name,
            creator,
            tracks,
        })
        .collect())
}

/// Everyone who made something in the library, by name.
#[query]
pub fn artists(db: &mut Db) -> Result<Vec<crate::schema::Artist>> {
    let mut by_name: BTreeMap<String, i64> = BTreeMap::new();
    for media in db.select(Media::all()) {
        *by_name.entry(media.creator).or_insert(0) += 1;
    }
    // The names come from `media` above, so a kind with no songs still has its
    // creator in this list; `person` is joined for the picture and nothing
    // else, which is the whole reason nothing has a foreign key into it.
    let mut art: BTreeMap<String, String> = db
        .select(Person::all())
        .into_iter()
        .map(|a| (a.name, a.art))
        .collect();
    Ok(by_name
        .into_iter()
        .map(|(name, tracks)| crate::schema::Artist {
            art: art.remove(&name).unwrap_or_default(),
            name,
            tracks,
        })
        .collect())
}

/// Who played each recording, as the one string a column can draw.
///
/// **Real credits win over the fallback.** `add_song` can only write the one
/// lumped string a track carries, at `pos: 0`; `credit_recording` writes people
/// with roles, from 1. Reading them together would list an orchestra twice —
/// once on its own and once inside the lumped string it came from — so a
/// recording that has been credited properly ignores what the track said.
///
/// The composer is skipped: it is on the work, and a column headed by the
/// performer repeating it would be saying the same thing twice.
#[cfg(feature = "storage")]
fn performers_of(db: &mut impl Store) -> BTreeMap<String, String> {
    let mut by_recording: BTreeMap<String, Vec<(i64, String)>> = BTreeMap::new();
    for credit in db.select(Credit::all()) {
        if credit.role == "composer" {
            continue;
        }
        by_recording
            .entry(credit.recording_id)
            .or_default()
            .push((credit.pos, credit.person_name));
    }
    by_recording
        .into_iter()
        .map(|(id, mut names)| {
            if names.iter().any(|(pos, _)| *pos > 0) {
                names.retain(|(pos, _)| *pos > 0);
            }
            names.sort();
            let joined = names
                .into_iter()
                .map(|(_, name)| name)
                .collect::<Vec<_>>()
                .join(", ");
            (id, joined)
        })
        .collect()
}

/// Every track that is on an album, and which one.
///
/// The columns a table draws beside the artist. Read from the song end, so a
/// kind with no albums contributes nothing and needs no case here.
///
/// Three of these used to be columns on `song` and are now joined: `part` and
/// `catalogue` are facts about the *work* — a catalogue number repeated once
/// per movement was one number stored fifty times — and `performer` is the
/// recording's credits, put back into the one string a table column can draw.
/// Whole tables are read and indexed rather than a lookup per track, because
/// this is one call for the whole library and a per-track lookup would be four
/// point reads times however many tracks there are.
#[query]
pub fn track_details(db: &mut Db) -> Result<Vec<crate::schema::TrackDetail>> {
    let works: BTreeMap<String, String> = db
        .select(Work::all())
        .into_iter()
        .map(|w| (w.id, w.catalogue))
        .collect();
    let movements: BTreeMap<String, (String, String)> = db
        .select(Movement::all())
        .into_iter()
        .map(|m| (m.id, (m.part, m.work_id)))
        .collect();
    let credits = performers_of(db);
    Ok(db
        .select(Song::all())
        .into_iter()
        .map(|s| {
            let movement = s.movement_id.as_ref().and_then(|id| movements.get(id));
            crate::schema::TrackDetail {
                media_id: s.media_id,
                album: s.album_name.unwrap_or_default(),
                track: s.track,
                part: movement.map(|(part, _)| part.clone()).unwrap_or_default(),
                catalogue: movement
                    .and_then(|(_, work)| works.get(work))
                    .cloned()
                    .unwrap_or_default(),
                performer: credits.get(&s.recording_id).cloned().unwrap_or_default(),
                bpm: s.bpm,
            }
        })
        .collect())
}

/// One album's tracks, in the order the work goes, read against a playlist.
///
/// The one list here that is *not* in library order, because an album is a
/// work and a work has an order of its own. It is also the only screen that
/// draws a track number, so this is the one place those numbers have to mean
/// the sequence they are sitting beside — a column counting something else is
/// worse than no column. Part first, so a work in four suites reads as four
/// blocks; the title breaks a tie, so a half-tagged album comes out
/// alphabetical rather than arbitrary, and the id breaks the last one, so two
/// peers showing the same album show it the same way.
///
/// The same `Item` the library list renders, so a screen showing an album is
/// the library screen with a different source and not a second row type.
#[query]
pub fn album(db: &mut Db, playlist_id: Id<Playlist>, name: String) -> Result<Vec<Item>> {
    // The part is the work's, so it is joined rather than read off the track.
    let parts: BTreeMap<String, String> = db
        .select(Movement::all())
        .into_iter()
        .map(|m| (m.id, m.part))
        .collect();
    let order: BTreeMap<Id<Media>, (String, i64)> = db
        .select(Song::all().filter(Song::album_name.eq(Some(name))))
        .into_iter()
        // 0 is "nobody said", and an untagged track belongs after the ones
        // that did say, not ahead of track 1.
        .map(|s| {
            let part = s
                .movement_id
                .as_ref()
                .and_then(|id| parts.get(id))
                .cloned()
                .unwrap_or_default();
            (
                s.media_id,
                (part, if s.track > 0 { s.track } else { i64::MAX }),
            )
        })
        .collect();
    let rows = db.select_with(library_query(), Media::playlist_item, items_on(playlist_id));
    let mut items: Vec<Item> = rows
        .iter()
        .filter(|r| order.contains_key(&r.row.id))
        .map(item_of)
        .collect();
    items.sort_by(|a, b| {
        order[&a.id]
            .cmp(&order[&b.id])
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(items)
}

/// One artist's tracks, in library order, read against a playlist.
#[query]
pub fn artist(db: &mut Db, playlist_id: Id<Playlist>, name: String) -> Result<Vec<Item>> {
    let rows = db.select_with(
        library_query().filter(Media::creator.eq(name)),
        Media::playlist_item,
        items_on(playlist_id),
    );
    Ok(rows.iter().map(item_of).collect())
}

/// Everyone this library has a work by, with what it has of them.
///
/// The Composers page. Read from `work` rather than from `media`, which is the
/// whole difference between this and `artists()`: that one answers "whose name
/// is under a title" for every kind, and this one answers "who wrote something
/// here" — so a library of pop and podcasts has none of these and a client
/// draws no Composers page for it.
#[query]
pub fn composers(db: &mut Db) -> Result<Vec<crate::schema::Composer>> {
    // Tracks per work first, through the movements, since a track names a
    // movement and the movement names the work.
    let mut tracks_of: BTreeMap<String, i64> = BTreeMap::new();
    let movements: BTreeMap<String, String> = db
        .select(Movement::all())
        .into_iter()
        .map(|m| (m.id, m.work_id))
        .collect();
    for song in db.select(Song::all()) {
        let Some(work) = song.movement_id.as_ref().and_then(|id| movements.get(id)) else {
            continue;
        };
        *tracks_of.entry(work.clone()).or_insert(0) += 1;
    }
    let mut by_name: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for work in db.select(Work::all()) {
        let entry = by_name.entry(work.composer).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += tracks_of.get(&work.id).copied().unwrap_or_default();
    }
    let mut known: BTreeMap<String, crate::schema::tables::Person> = db
        .select(Person::all())
        .into_iter()
        .map(|p| (p.name.clone(), p))
        .collect();
    Ok(by_name
        .into_iter()
        .map(|(name, (works, tracks))| {
            let person = known.remove(&name);
            crate::schema::Composer {
                sort_name: match person.as_ref().map(|p| p.sort_name.as_str()) {
                    // A client sorting an index needs *something*, and the name
                    // is a better guess than the empty string — which would put
                    // everyone nobody has described at the top together.
                    None | Some("") => name.clone(),
                    Some(sorted) => sorted.to_string(),
                },
                born: person.as_ref().map(|p| p.born).unwrap_or_default(),
                died: person.as_ref().map(|p| p.died).unwrap_or_default(),
                art: person.map(|p| p.art).unwrap_or_default(),
                name,
                works,
                tracks,
            }
        })
        .collect())
}

/// One composer's works, with how much of each this library holds.
#[query]
pub fn works(db: &mut Db, composer: String) -> Result<Vec<crate::schema::Work>> {
    let movements: BTreeMap<String, String> = db
        .select(Movement::all())
        .into_iter()
        .map(|m| (m.id, m.work_id))
        .collect();
    let mut tracks_of: BTreeMap<String, i64> = BTreeMap::new();
    for song in db.select(Song::all()) {
        let Some(work) = song.movement_id.as_ref().and_then(|id| movements.get(id)) else {
            continue;
        };
        *tracks_of.entry(work.clone()).or_insert(0) += 1;
    }
    let mut takes_of: BTreeMap<String, i64> = BTreeMap::new();
    for rec in db.select(Recording::all()) {
        let Some(work) = rec.work_id else { continue };
        *takes_of.entry(work).or_insert(0) += 1;
    }
    let mut out: Vec<crate::schema::Work> = db
        .select(Work::all().filter(Work::composer.eq(composer)))
        .into_iter()
        .map(|w| crate::schema::Work {
            recordings: takes_of.get(&w.id).copied().unwrap_or_default(),
            tracks: tracks_of.get(&w.id).copied().unwrap_or_default(),
            id: w.id,
            title: w.title,
            composer: w.composer,
            catalogue: w.catalogue,
            form: w.form,
            period: w.period,
            art: w.art,
        })
        .collect();
    // By catalogue and then title, which is how a composer's page is ordered
    // everywhere — `BWV 988` before `BWV 1013` is wrong as a string compare and
    // right as one nobody has asked for yet, so this is alphabetical and says
    // so rather than pretending to sort numbers it has not parsed.
    out.sort_by(|a, b| {
        a.catalogue
            .cmp(&b.catalogue)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(out)
}

/// Every performance of one work that this library holds.
///
/// The list an Apple Music Classical work page is: one row per recording, told
/// apart by who played it rather than by what it is called, because every row
/// here is the same piece of music.
#[query]
pub fn recordings(db: &mut Db, work_id: String) -> Result<Vec<crate::schema::Recording>> {
    let mut tracks_of: BTreeMap<String, i64> = BTreeMap::new();
    for song in db.select(Song::all()) {
        *tracks_of.entry(song.recording_id).or_insert(0) += 1;
    }
    let credits = performers_of(db);
    let mut out: Vec<crate::schema::Recording> = db
        .select(Recording::all().filter(Recording::work_id.eq(Some(work_id))))
        .into_iter()
        .map(|r| crate::schema::Recording {
            performers: credits.get(&r.id).cloned().unwrap_or_default(),
            tracks: tracks_of.get(&r.id).copied().unwrap_or_default(),
            id: r.id,
            recorded: r.recorded,
            label: r.label,
            licence: r.licence,
            art: r.art,
        })
        .collect();
    // The most complete first, because that is what somebody opening a work
    // wants to hear; then the oldest, then the key, so two peers agree.
    out.sort_by(|a, b| {
        b.tracks
            .cmp(&a.tracks)
            .then_with(|| a.recorded.cmp(&b.recorded))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(out)
}

/// One performance's tracks, in the order the work goes.
///
/// **By movement number, not by track number**, which is the difference this
/// whole shape exists for: a compilation puts the Moonlight's first movement at
/// track 9, and a page about the sonata has to put it first. `album()` is the
/// other half of the same pair and sorts the other way, because a release page
/// is about the release.
#[query]
pub fn recording(db: &mut Db, playlist_id: Id<Playlist>, id: String) -> Result<Vec<Item>> {
    let movements: BTreeMap<String, i64> = db
        .select(Movement::all())
        .into_iter()
        .map(|m| (m.id, m.no))
        .collect();
    let order: BTreeMap<Id<Media>, i64> = db
        .select(Song::all().filter(Song::recording_id.eq(id)))
        .into_iter()
        .map(|s| {
            let no = s
                .movement_id
                .as_ref()
                .and_then(|id| movements.get(id))
                .copied()
                // Not of a work, so there is no movement to be: after the ones
                // that are, rather than ahead of movement 1.
                .unwrap_or(i64::MAX);
            (s.media_id, no)
        })
        .collect();
    let rows = db.select_with(library_query(), Media::playlist_item, items_on(playlist_id));
    let mut items: Vec<Item> = rows
        .iter()
        .filter(|r| order.contains_key(&r.row.id))
        .map(item_of)
        .collect();
    items.sort_by(|a, b| {
        order[&a.id]
            .cmp(&order[&b.id])
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(items)
}

/// Every playlist, in the order they were made.
#[query]
pub fn playlists(db: &mut Db) -> Result<Vec<crate::schema::Playlist>> {
    Ok(db
        .select(
            Playlist::all()
                .order_by(Playlist::pos.asc())
                .order_by(Playlist::id.asc()),
        )
        .into_iter()
        .map(|p| crate::schema::Playlist {
            id: p.id,
            name: p.name,
            pos: p.pos,
            created_ms: p.created_ms,
            user_id: p.user_id,
        })
        .collect())
}

/// Which playlists a track is already on, in the order the playlists were made.
///
/// The sheet that adds a song to a playlist needs this to be a *toggle* rather
/// than a one-way door: a list of every playlist with no mark on the ones it
/// is already on is a list you can add the same track to twice and never take
/// it off. Read from the entries rather than from the playlists, so the cost
/// is the memberships this one track has and not the number of playlists.
#[query]
pub fn playlists_of(db: &mut Db, media_id: Id<Media>) -> Result<Vec<crate::schema::Playlist>> {
    let on: BTreeSet<Id<Playlist>> = db
        .select(PlaylistItem::all().filter(PlaylistItem::media_id.eq(media_id)))
        .into_iter()
        .map(|e| e.playlist_id)
        .collect();
    Ok(playlists(db)?
        .into_iter()
        .filter(|p| on.contains(&p.id))
        .collect())
}

/// One playlist's contents, in playlist order.
#[query]
pub fn playlist(db: &mut Db, id: Id<Playlist>) -> Result<Vec<Item>> {
    // Read from the other end: favorites, each carrying its item. A favorite
    // whose item is gone carries nothing and is dropped, which is the INNER
    // JOIN — and `remove_song` deletes both, so it should not arise.
    let rows = db.select_with(
        items_on(id)
            .order_by(PlaylistItem::pos.asc())
            .order_by(PlaylistItem::media_id.asc()),
        PlaylistItem::media,
        Media::all(),
    );
    Ok(rows
        .iter()
        .filter_map(|r| {
            let media = r.one()?;
            Some(Item {
                id: media.id,
                kind: media.kind.clone(),
                title: media.title.clone(),
                creator: media.creator.clone(),
                duration_ms: media.duration_ms,
                file: media.file.clone(),
                pos: media.pos,
                added_ms: media.added_ms,
                user_id: media.user_id.clone(),
                playlist_pos: Some(r.row.pos),
            })
        })
        .collect())
}

/// One playable row and its place in the playlist, as a client reads it.
#[cfg(feature = "storage")]
fn item_of(row: &With<Media, PlaylistItem>) -> Item {
    Item {
        id: row.row.id,
        kind: row.row.kind.clone(),
        title: row.row.title.clone(),
        creator: row.row.creator.clone(),
        duration_ms: row.row.duration_ms,
        file: row.row.file.clone(),
        pos: row.row.pos,
        added_ms: row.row.added_ms,
        user_id: row.row.user_id.clone(),
        playlist_pos: row.one().map(|i| i.pos),
    }
}

// Every verb, and this list is the one that decides whether a mutation can be
// *applied*. `#[mutation]` alone is not enough: it writes the authoring
// function and the schema section, so a verb missing from here still compiles,
// still type-checks at every call site, and still appears in `mutations.txt` —
// and is then refused at apply time as an unknown mutation. `set_artwork` was
// exactly that for its whole life — two commits, and what it looked like was
// covers that never loaded. It is gone now, and the covers it could not set
// ride on `add_song`.
peer!(
    add_song,
    describe_work,
    describe_recording,
    describe_person,
    credit_recording,
    create_playlist,
    add_to_playlist,
    add_all_to_playlist,
    remove_from_playlist,
    remove_media
);

// ---------------------------------------------------------- what a peer keeps

/// What the peer maintains beside its client.
///
/// `petros::foreign_peer!` hydrates this at open and settles it after every
/// mutation and every message from the server, so nothing here depends on a
/// caller remembering to ask.
#[cfg(feature = "storage")]
pub struct Views {
    library: LibraryView,
    count: PlaylistCount,
    /// Which playlist a heart means. Nil until a caller says, because the
    /// domain has no favorites of its own — a client chooses the playlist it
    /// is showing membership for.
    playlist: Id<Playlist>,
    /// What the library did since the caller last collected. A foreign caller
    /// polls rather than being pushed to, so this has to accumulate.
    pending: Vec<petros::ivm::Patch>,
    /// Set by a rebase, which no sequence of patches describes. The caller
    /// takes the whole list again and starts over.
    reset: bool,
}

#[cfg(feature = "storage")]
impl Views {
    /// Show membership of this playlist from now on.
    ///
    /// Rebuilds the maintained view against it and asks the caller to take the
    /// whole list again, because no sequence of patches turns one playlist's
    /// memberships into another's.
    pub fn use_playlist(&mut self, id: Id<Playlist>, store: &mut petros::backend::SqliteStore<'_>) {
        self.library = library_view(id);
        self.count = playlist_count(id);
        self.playlist = id;
        self.library.hydrate(store);
        self.count.hydrate(store);
        self.pending.clear();
        self.reset = true;
    }

    /// Which playlist a heart currently means.
    pub fn playlist(&self) -> Id<Playlist> {
        self.playlist
    }
}

#[cfg(feature = "storage")]
impl petros::Views for Views {
    fn build() -> Self {
        Views {
            library: library_view(Id::nil()),
            count: playlist_count(Id::nil()),
            playlist: Id::nil(),
            pending: Vec::new(),
            reset: true,
        }
    }

    fn hydrate(&mut self, store: &mut petros::backend::SqliteStore<'_>) {
        self.library.hydrate(store);
        self.count.hydrate(store);
        // Anything already collected describes a database that no longer
        // exists, and anything not yet collected describes one the caller never
        // saw. Both are wrong; the whole list is not.
        self.pending.clear();
        self.reset = true;
    }

    fn apply(
        &mut self,
        store: &mut petros::backend::SqliteStore<'_>,
        changes: &[petros_schema::Change],
    ) {
        self.count.apply(store, changes);
        let patches = self.library.apply(store, changes);
        self.pending.extend(patches);
    }
}

// ------------------------------------------------------- across the boundary

/// What the library did, as a foreign caller hears it.
///
/// The list used to cross whole on every change, which is the expensive half on
/// a phone: maintaining the query saves the SQL and the bridge charges for the
/// rows regardless. This carries the rows that moved.
///
/// `reset` is the rebase, and it is not a failure — it is the case no sequence
/// of patches can describe, because the optimistic view was rolled back and a
/// rollback reports nothing. Then `items` is the whole list and `patches` is
/// empty; otherwise `items` is empty and `patches` is what to splice.
#[cfg(feature = "foreign")]
#[derive(uniffi::Record)]
pub struct LibraryUpdate {
    pub reset: bool,
    pub items: Vec<crate::schema::foreign::Item>,
    pub patches: Vec<ItemPatch>,
    /// How many items are on the playlist being shown, maintained rather than
    /// counted on the far side.
    pub on_playlist: u32,
}

/// One entry of the list moving. Positions are valid in sequence: apply them in
/// order to a list that started equal and it ends equal.
#[cfg(feature = "foreign")]
#[derive(uniffi::Record)]
pub struct ItemPatch {
    pub op: PatchOp,
    pub at: u32,
    /// Absent for a removal, which needs only the position.
    pub item: Option<crate::schema::foreign::Item>,
}

#[cfg(feature = "foreign")]
#[derive(uniffi::Enum)]
pub enum PatchOp {
    Insert,
    Remove,
    Update,
}

#[cfg(feature = "foreign")]
impl Views {
    /// Collect what has happened since the last call, and start accumulating
    /// again.
    ///
    /// `foreign` rather than `storage`, so it builds the boundary record
    /// directly. It was a tuple of four things and a mapping step, because the
    /// storage build has no boundary types — but `foreign` implies `storage`,
    /// so the split bought nothing and cost a shape clippy was right to dislike.
    fn take_update(&mut self) -> LibraryUpdate {
        let on_playlist = self.count.get() as u32;
        if std::mem::take(&mut self.reset) {
            self.pending.clear();
            return LibraryUpdate {
                reset: true,
                items: items_of(&self.library)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                patches: Vec::new(),
                on_playlist,
            };
        }
        let item_of_node = |node: &petros_schema::Tree| {
            node.decode::<Media, PlaylistItem>()
                .as_ref()
                .map(item_of)
                .map(Into::into)
        };
        LibraryUpdate {
            reset: false,
            items: Vec::new(),
            patches: std::mem::take(&mut self.pending)
                .into_iter()
                .map(|patch| match patch {
                    petros::ivm::Patch::Insert { at, node } => ItemPatch {
                        op: PatchOp::Insert,
                        at: at as u32,
                        item: item_of_node(&node),
                    },
                    petros::ivm::Patch::Remove { at } => ItemPatch {
                        op: PatchOp::Remove,
                        at: at as u32,
                        item: None,
                    },
                    petros::ivm::Patch::Update { at, node } => ItemPatch {
                        op: PatchOp::Update,
                        at: at as u32,
                        item: item_of_node(&node),
                    },
                })
                .collect(),
            on_playlist,
        }
    }
}

#[cfg(feature = "foreign")]
#[uniffi::export]
impl Peer {
    /// What the library did since you last asked.
    ///
    /// Poll this after a mutation or a frame from the server. The peer keeps the
    /// view up to date on its own, so an empty answer means nothing moved and
    /// the screen need not redraw.
    pub fn library_update(&self) -> ::core::result::Result<LibraryUpdate, crate::PeerError> {
        self.views(|views| ::core::result::Result::Ok(views.take_update()))
    }

    /// Show membership of this playlist: what a heart in the library means.
    ///
    /// The domain has no favorites of its own, so a client says which playlist
    /// it is drawing hearts for. The next `library_update` is a reset, because
    /// no sequence of patches turns one playlist's memberships into another's.
    pub fn show_playlist(&self, id: String) -> ::core::result::Result<(), crate::PeerError> {
        let playlist: Id<Playlist> = id.parse().map_err(|_| crate::PeerError::Refused {
            reason: ::std::format!("{id} is not an id"),
        })?;
        self.views_with_store(|store, views| {
            views.use_playlist(playlist, store);
            ::core::result::Result::Ok(())
        })
    }
}
