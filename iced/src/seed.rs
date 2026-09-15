//! What an empty demo library starts with.
//!
//! Only the demo has this: `harken-web` is built with no features and never
//! compiles the file, so none of these strings reach the client that talks to
//! a real server. The library there is whatever the scanner found.
//!
//! Everything goes in through `mutate`, not through SQL — the demo runs the
//! same `apply` as every other peer, and seeding it any other way would be
//! showing something the engine did not do.
//!
//! Public-domain recordings on Wikimedia Commons, by way of the mp3 transcode
//! Commons generates for every audio file: a browser plays mp3 everywhere, and
//! Vorbis in an `.ogg` does not play in Safari at all. The URL goes in `file`,
//! which is what that column has always been for, so nothing about the log
//! changes to carry a recording.
//!
//! Every entry was checked, because a dead link here is a silent demo: no
//! rights reserved by Commons' own licence field, and a transcode that answers
//! `audio/mpeg` to a range request.

use crate::{Peer, Source};
use harken::{self as mutators};

/// Put something in an empty demo library, so the page has a list on it.
///
/// Through `mutate`, not through SQL: the demo runs the same `apply` as
/// every other peer, and seeding it any other way would be showing
/// something the engine did not do.
pub fn seed(peer: &mut Peer) {
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
    "11. Allegro",
    "George Frideric Handel",
    "Water Music Suite No. 1 in F major, HWV 348",
    125000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/d/de/Handel%27s_Water_Music_-_11._Allegro_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_11._Allegro_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "12. Alla Hornpipe",
    "George Frideric Handel",
    "Water Music Suite No. 2 in D major, HWV 349",
    229000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/5/5c/Handel%27s_Water_Music_-_12._Alla_hornpipe_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_12._Alla_hornpipe_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "13. Minuet",
    "George Frideric Handel",
    "Water Music Suite No. 2 in D major, HWV 349",
    195000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/c/c9/Handel%27s_Water_Music_-_13._Minuet_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_13._Minuet_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "14. Lentement",
    "George Frideric Handel",
    "Water Music Suite No. 2 in D major, HWV 349",
    136000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/7/75/Handel%27s_Water_Music_-_14._Lentement_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_14._Lentement_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "15. Bourrée",
    "George Frideric Handel",
    "Water Music Suite No. 2 in D major, HWV 349",
    76000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/2/2d/Handel%27s_Water_Music_-_15._Bourree_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_15._Bourree_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "16. Sarabande",
    "George Frideric Handel",
    "Water Music Suite No. 3 in G major, HWV 350",
    168000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/e/e8/Handel%27s_Water_Music_-_16._Sarabande_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_16._Sarabande_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "17-18. Rigaudon",
    "George Frideric Handel",
    "Water Music Suite No. 3 in G major, HWV 350",
    156000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/c/c7/Handel%27s_Water_Music_-_17._%26_18._Rigaudon_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_17._%26_18._Rigaudon_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "19-20. Menuet",
    "George Frideric Handel",
    "Water Music Suite No. 3 in G major, HWV 350",
    227000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/a/ac/Handel%27s_Water_Music_-_19._%26_20._Menuet_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_19._%26_20._Menuet_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "21-22. Gigue",
    "George Frideric Handel",
    "Water Music Suite No. 3 in G major, HWV 350",
    85000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/f/fe/Handel%27s_Water_Music_-_21._%26_22._Gigue_-_Chamber_Orchestra_-_United_States_Marine_Band.opus/Handel%27s_Water_Music_-_21._%26_22._Gigue_-_Chamber_Orchestra_-_United_States_Marine_Band.opus.mp3",
),
(
    "Impromptu in G-flat major, D. 899",
    "Franz Schubert",
    "Impromptus",
    301000,
    "https://upload.wikimedia.org/wikipedia/commons/transcoded/0/0b/Schubert_Gb_Impromptu_Andriy_Bondarenko_%28Live%29.ogg/Schubert_Gb_Impromptu_Andriy_Bondarenko_%28Live%29.ogg.mp3",
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
        let Some(list) = peer.choices.iter().find_map(|c| match &c.source {
            Source::Playlist(id, n) if n == name => Some(*id),
            _ => None,
        }) else {
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
