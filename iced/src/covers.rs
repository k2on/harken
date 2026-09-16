//! Covers: fetched once, kept, and drawn instead of the derived square.
//!
//! `artwork` in the log carries a *name* — a path under the media root, or a
//! whole URL — and never the bytes, for the reason `media.file` does not. So
//! the bytes are this file's problem, and the whole of the problem is doing it
//! once: a grid of forty albums re-fetching on every frame would be forty
//! requests twenty times a second.
//!
//! **Two caches, because the two targets have different ones to offer.** On a
//! desktop it is a directory, so a cover survives a restart and a library of
//! four hundred albums is fetched once ever. In a browser the disk belongs to
//! the browser, so it is the Cache API — the same store a service worker uses,
//! keyed by URL, which is exactly the shape of this question. Both sit behind
//! one `want`/`handle` pair, so nothing above knows which one it got.
//!
//! What is *not* cached is the decoded handle beyond the session: `Handle`
//! holds decoded pixels, and forty albums of those is tens of megabytes. The
//! map here is the session's, the disk is the machine's.

use std::collections::HashMap;

use iced::widget::image;
use iced::Task;

/// Where one cover has got to.
enum State {
    /// Asked for, not answered yet. Held so that a second frame does not ask
    /// again — which is the whole reason this type exists rather than a bare
    /// map of ready handles.
    Loading,
    Ready(image::Handle),
    /// Asked for and refused. Remembered rather than retried: a 404 is not
    /// going to become a 200 because the window was redrawn, and a cover that
    /// retries every frame is a cover that hammers a server.
    ///
    /// Why is deliberately dropped. A cover is decoration — the derived square
    /// is already a correct thing to draw — and a status line reading "could
    /// not fetch the cover" forty times would bury the notes that matter, like
    /// having been signed out. The one thing worth keeping is "do not ask
    /// again", and that is what this is.
    Missing,
}

/// Every cover this session has asked about.
#[derive(Default)]
pub struct Covers {
    by_url: HashMap<String, State>,
}

impl Covers {
    /// The handle to draw, if there is one yet.
    ///
    /// `None` covers all three of "not asked", "still coming" and "there is no
    /// such image", because the answer to every one of them is the same: draw
    /// the derived square. A spinner where a cover will be is a worse thing to
    /// look at than the square that is already right.
    pub fn handle(&self, url: &str) -> Option<&image::Handle> {
        match self.by_url.get(url) {
            Some(State::Ready(handle)) => Some(handle),
            _ => None,
        }
    }

    /// Start fetching this one, unless it is already known.
    ///
    /// Returns a `Task` rather than doing the work, because `update` is the
    /// only place in this program that is allowed to have side effects and
    /// `view` is the only place that knows what is on screen. So the call site
    /// is whatever `update` does after the library changes.
    pub fn want(&mut self, url: String) -> Option<Task<Loaded>> {
        if url.is_empty() || self.by_url.contains_key(&url) {
            return None;
        }
        self.by_url.insert(url.clone(), State::Loading);
        Some(fetch(url))
    }

    /// What a finished fetch did.
    pub fn loaded(&mut self, done: Loaded) {
        let state = match done.bytes {
            Ok(bytes) => State::Ready(image::Handle::from_bytes(bytes)),
            Err(_) => State::Missing,
        };
        self.by_url.insert(done.url, state);
    }
}

/// One finished fetch, on its way back into `update`.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub url: String,
    pub bytes: Result<Vec<u8>, String>,
}

/// FNV-1a over the bytes of a URL, as sixteen hex digits.
///
/// The cache's filename. Not `art::hash`, which is 32 bits and picks one of
/// six gradients — a collision there means the wrong shade of gold and a
/// collision here means the wrong picture, which is a different kind of wrong.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn key(url: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.as_bytes() {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::{key, Loaded};
    use iced::Task;

    /// Where a cover lives between runs.
    ///
    /// Beside the remembered login rather than in the state directory: this is
    /// a *cache* — losing it costs a re-fetch and nothing else — so it belongs
    /// where a machine sweeps caches, and `XDG_CACHE_HOME` is that place.
    fn dir() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache"))
            })?;
        Some(base.join("harken").join("covers"))
    }

    /// The bytes, from the disk if they are there and from the network if not.
    ///
    /// Blocking, on a thread of its own. `iced::futures::channel::oneshot` is
    /// what carries the answer back, so this needs no runtime — which matters,
    /// because the desktop build's executor is `smol` and handing it a
    /// blocking HTTP call would stall every other task behind it.
    pub fn fetch(url: String) -> Task<Loaded> {
        let (tx, rx) = iced::futures::channel::oneshot::channel();
        let back = url.clone();
        std::thread::spawn(move || {
            let _ = tx.send(blocking(&back));
        });
        Task::perform(
            async move {
                rx.await
                    .unwrap_or_else(|_| Err("the cover fetch was dropped".to_string()))
            },
            move |bytes| Loaded {
                url: url.clone(),
                bytes,
            },
        )
    }

    fn blocking(url: &str) -> Result<Vec<u8>, String> {
        let path = dir().map(|d| d.join(key(url)));
        if let Some(path) = &path {
            if let Ok(bytes) = std::fs::read(path) {
                return Ok(bytes);
            }
        }
        let response = ureq::get(url)
            .call()
            .map_err(|e| format!("could not fetch the cover: {e}"))?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes)
            .map_err(|e| format!("could not read the cover: {e}"))?;
        // Written after it is whole, and a failed write is not a failed fetch:
        // a machine with no writable cache directory should still show covers,
        // it should just fetch them again next time.
        if let Some(path) = &path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, &bytes);
        }
        Ok(bytes)
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::Loaded;
    use iced::Task;
    use wasm_bindgen::prelude::*;

    // A snippet rather than `web-sys`, the same trade `player.rs` makes for
    // the media session: `CacheStorage` is behind `web_sys_unstable_apis`,
    // which is a `RUSTFLAGS` every build of this crate would have to agree
    // on — the nix one, the devshell's, and whatever else compiles it. This
    // travels inside the module instead.
    //
    // `caches` needs a secure context, so it is absent on plain `http://` that
    // is not localhost. That is a fall-through rather than a failure: no
    // cache, a plain fetch, and covers that arrive exactly as fast the first
    // time and slower the second.
    #[wasm_bindgen(inline_js = r#"
export async function cover(url) {
  let store = null;
  try { store = await caches.open('harken-covers-v1'); } catch (e) { store = null; }
  if (store) {
    const hit = await store.match(url);
    if (hit) return new Uint8Array(await hit.arrayBuffer());
  }
  const response = await fetch(url, { mode: 'cors' });
  if (!response.ok) throw new Error('HTTP ' + response.status);
  // Put the clone, read the original: a Response body is a stream and can be
  // consumed exactly once, so reading it first leaves the cache nothing.
  if (store) { try { await store.put(url, response.clone()); } catch (e) {} }
  return new Uint8Array(await response.arrayBuffer());
}
"#)]
    extern "C" {
        #[wasm_bindgen(catch)]
        async fn cover(url: &str) -> Result<JsValue, JsValue>;
    }

    pub fn fetch(url: String) -> Task<Loaded> {
        let back = url.clone();
        Task::perform(
            async move {
                match cover(&back).await {
                    Ok(value) => Ok(js_sys::Uint8Array::new(&value).to_vec()),
                    Err(e) => Err(format!(
                        "could not fetch the cover: {}",
                        e.as_string().unwrap_or_else(|| "failed".into())
                    )),
                }
            },
            move |bytes| Loaded {
                url: url.clone(),
                bytes,
            },
        )
    }
}

use imp::fetch;

#[cfg(test)]
mod tests {
    use super::{key, Covers};

    /// A cache filename has to be a *function of the URL* and nothing else, or
    /// two runs of the same program disagree about where a cover is and the
    /// cache never hits.
    #[test]
    fn the_key_is_the_url() {
        assert_eq!(
            key("https://example.com/a.jpg"),
            key("https://example.com/a.jpg")
        );
        assert_ne!(
            key("https://example.com/a.jpg"),
            key("https://example.com/b.jpg")
        );
        assert_eq!(key("").len(), 16);
    }

    /// The point of `State::Loading`: a second ask for a cover already in
    /// flight has to be a no-op, or a grid on screen issues one request per
    /// album per frame.
    #[test]
    fn asking_twice_fetches_once() {
        let mut covers = Covers::default();
        assert!(covers.want("https://example.com/a.jpg".into()).is_some());
        assert!(covers.want("https://example.com/a.jpg".into()).is_none());
    }

    /// An empty `art` is "nobody set one", not a URL to go and fail on.
    #[test]
    fn nothing_is_not_a_cover() {
        let mut covers = Covers::default();
        assert!(covers.want(String::new()).is_none());
    }
}
