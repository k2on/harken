//! What the address bar says, in a browser.
//!
//! A page that shows a library, then an album, then an artist and answers the
//! back button by leaving the site is a page that has broken the one control
//! every browser has. So what the sidebar picked goes in the URL's fragment,
//! and the fragment is read back.
//!
//! **The fragment, not the query.** `?server=` and `?code=` are already in the
//! query — the first is how a page is told where its server is, the second is
//! what a sign-in comes back with — and rewriting the query to record a
//! sidebar click would mean rewriting those every time. A fragment is also the
//! one part of a URL a static host never has to be told about, which is what
//! GitHub Pages serves the demo as.
//!
//! **A route is not a [`crate::Source`].** A playlist is identified by id and
//! an id is not in the URL: two peers of one server agree about the id, but
//! the address bar is read by a person, and `#playlist/Favourites` is the part
//! they can type. So this carries names, and the app resolves a name against
//! the playlists it has — which also means a link to a playlist that has since
//! been renamed lands on the library rather than on nothing.
//!
//! **Read by polling rather than by listening.** The back button fires
//! `popstate`, and hearing it means a closure that outlives this call, kept
//! alive for the life of the page, publishing into a channel iced can
//! subscribe to. The tick already runs twenty times a second for the
//! transport; reading `location.hash` on it is a string compare, and the
//! answer is the same one `popstate` would have given a frame earlier.

/// Somewhere the sidebar can be, as the URL spells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Library,
    Playlist(String),
    Album(String),
    Artist(String),
}

// Only a browser reads or writes an address bar, so on the desktop these are
// reached by the tests below and by nothing else. They are not `cfg`d away
// with the rest of the browser half for exactly that reason: the encoding is
// the part worth testing, and `cargo test` runs on the target that cannot run
// a browser.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Route {
    /// The fragment this route is written as, `#` included.
    pub fn fragment(&self) -> String {
        let (kind, name) = match self {
            Route::Library => return "#library".to_string(),
            Route::Playlist(name) => ("playlist", name),
            Route::Album(name) => ("album", name),
            Route::Artist(name) => ("artist", name),
        };
        let mut out = format!("#{kind}/");
        crate::encode(name, &mut out);
        out
    }

    /// Parse one back. Anything unrecognised is the library, because a URL
    /// somebody typed wrong should land somewhere rather than nowhere.
    pub fn parse(fragment: &str) -> Route {
        let body = fragment.trim_start_matches('#');
        let (kind, name) = match body.split_once('/') {
            Some(pair) => pair,
            None => return Route::Library,
        };
        let name = decode(name);
        match kind {
            "playlist" => Route::Playlist(name),
            "album" => Route::Album(name),
            "artist" => Route::Artist(name),
            _ => Route::Library,
        }
    }
}

/// Percent-decode. The other half of [`crate::encode`], and the only place it
/// is needed: everything else in this program writes URLs and never reads one.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // A fragment somebody hand-edited is not guaranteed to be UTF-8, and a
    // name that does not decode is better as the bytes that did than as a
    // panic.
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_arch = "wasm32")]
pub use browser::{read, write};

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::Route;

    /// What the address bar currently says.
    pub fn read() -> Option<Route> {
        let hash = web_sys::window()?.location().hash().ok()?;
        (!hash.is_empty()).then(|| Route::parse(&hash))
    }

    /// Put `route` in the address bar, as a new entry in the history or in
    /// place of the current one.
    ///
    /// The history API rather than setting `location.hash`: the two do the
    /// same thing to the URL, and the second also fires a `hashchange` that
    /// the poll would read back as somebody pressing the back button. Writing
    /// through `history` leaves the page's own read of the hash to be the one
    /// it just wrote.
    pub fn write(route: &Route, keep: bool) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(history) = window.history() else {
            return;
        };
        let url = Some(route.fragment());
        let null = wasm_bindgen::JsValue::NULL;
        let _ = if keep {
            history.push_state_with_url(&null, "", url.as_deref())
        } else {
            history.replace_state_with_url(&null, "", url.as_deref())
        };
    }
}

/// Nothing on the desktop has an address bar, so nothing here does anything.
/// The type stays shared so that the one place the sidebar records a choice is
/// written once and reads the same in both builds.
#[cfg(not(target_arch = "wasm32"))]
pub fn read() -> Option<Route> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write(_route: &Route, _keep: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route survives the round trip, including the characters a real
    /// library is full of.
    #[test]
    fn a_route_survives_the_address_bar() {
        for route in [
            Route::Library,
            Route::Playlist("Favourites".into()),
            Route::Album("Water Music".into()),
            Route::Artist("Johann Sebastian Bach".into()),
            // The ones that break a naive encoding: a slash would look like
            // the separator, a `#` would end the fragment, and an accent is
            // two bytes.
            Route::Album("Boléro / Pavane #1".into()),
        ] {
            let fragment = route.fragment();
            assert_eq!(Route::parse(&fragment), route, "round trip of {fragment}");
        }
    }

    /// A fragment nobody wrote lands somewhere rather than nowhere.
    #[test]
    fn a_fragment_that_means_nothing_is_the_library() {
        assert_eq!(Route::parse(""), Route::Library);
        assert_eq!(Route::parse("#"), Route::Library);
        assert_eq!(Route::parse("#library"), Route::Library);
        assert_eq!(Route::parse("#nonsense/Water Music"), Route::Library);
        // A truncated escape is not a reason to fail: what decoded, decoded.
        assert_eq!(Route::parse("#album/Bol%"), Route::Album("Bol%".into()));
    }
}
