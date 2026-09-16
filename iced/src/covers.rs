//! Covers: fetched once, shrunk once, and drawn instead of the derived square.
//!
//! `artwork` in the log carries a *name* — a path under the media root, or a
//! whole URL — and never the bytes, for the reason `media.file` does not. So
//! the bytes are this file's problem, and the problem has two halves: doing
//! the fetch once, and handing the renderer something the size of the thing on
//! screen.
//!
//! **The second half is the one that was wrong, and it stuttered.**
//! `image::Handle::from_bytes` does not decode — it hands iced the encoded
//! bytes and the decode happens in whichever frame first *draws* them. So a
//! 960×1262 portrait was a 1.2-megapixel JPEG decoded inside a frame, and then
//! 4.8 MB of RGBA in the texture atlas for something 132 pixels wide. Seven of
//! those is 34 MB of atlas, and iced evicts what a frame did not use: leaving
//! the artists page dropped six of them and coming back decoded all six again.
//! Which is exactly where it showed — a hitch on *navigation*, both ways.
//!
//! So the decode happens here, where the fetch already is and where being slow
//! costs nothing, and what reaches the renderer is `Handle::from_rgba` at
//! [`BOUND`] on its longest side: pixels to upload, no decoder in the frame,
//! and an atlas a twentieth of the size.
//!
//! **Two caches, because the two targets have different ones to offer.** On a
//! desktop it is a directory, so a cover survives a restart and a library of
//! four hundred albums is fetched once ever. In a browser the disk belongs to
//! the browser, so it is the Cache API — the same store a service worker uses,
//! keyed by URL, which is exactly the shape of this question. Both sit behind
//! one `want`/`handle` pair, so nothing above knows which one it got.
//!
//! What both of them cache is the *encoded* original, which is the fetch and
//! not the decode. Keeping the shrunk pixels instead would be ten times the
//! disk to save a few milliseconds of CPU that is not on the render thread
//! anyway — and a cache of the answer could not be re-shrunk if [`BOUND`] ever
//! moved. What is not cached at all beyond the session is the `Handle`: it
//! holds decoded pixels, and the map here is the session's while the disk is
//! the machine's.

use std::collections::HashMap;

use iced::widget::image;
use iced::Task;

/// The longest side a cover is kept at, in pixels.
///
/// The biggest square this program draws is the 132px card on an index page;
/// the record header's is 116. So this is a shade under 3× the largest drawn
/// size, which covers every display anybody has and leaves the atlas holding
/// at most 590 KB a cover rather than the 4.8 MB a full-size portrait was.
///
/// It is a *bound*, not a size: aspect is kept, so a wide painting comes out
/// wide and `ContentFit::Cover` still does the cropping in `view`, where that
/// decision belongs. Anything already smaller is left exactly as it is.
pub const BOUND: u32 = 384;

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
    ///
    /// `from_rgba` rather than `from_bytes`: the pixels are already decoded and
    /// already the size they will be drawn at, so there is nothing left for the
    /// renderer to do but upload them.
    pub fn loaded(&mut self, done: Loaded) {
        let state = match done.image {
            Ok(rgba) => State::Ready(image::Handle::from_rgba(
                rgba.width,
                rgba.height,
                rgba.pixels,
            )),
            Err(_) => State::Missing,
        };
        self.by_url.insert(done.url, state);
    }
}

/// A decoded cover, at most [`BOUND`] on its longest side.
#[derive(Clone)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    /// Four bytes a pixel, row major. What `image::Handle::from_rgba` wants.
    pub pixels: Vec<u8>,
}

// Written out rather than derived, because `Message` is `Debug` and a derived
// one would put half a megabyte of pixels in anything that ever prints a
// message. The size is the only part worth reading.
impl std::fmt::Debug for Rgba {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rgba({}×{}, {} B)",
            self.width,
            self.height,
            self.pixels.len()
        )
    }
}

/// One finished fetch, on its way back into `update`.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub url: String,
    pub image: Result<Rgba, String>,
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
    use super::{key, Loaded, Rgba, BOUND};
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

    /// The pixels, from the disk if the bytes are there and from the network if
    /// not, decoded and shrunk before they come back.
    ///
    /// Blocking, on a thread of its own. `iced::futures::channel::oneshot` is
    /// what carries the answer back, so this needs no runtime — which matters,
    /// because the desktop build's executor is `smol` and handing it a
    /// blocking HTTP call would stall every other task behind it. The decode
    /// rides along on that same thread for the same reason it is not in the
    /// renderer: it is the one place in this program where taking twenty
    /// milliseconds costs nobody anything.
    pub fn fetch(url: String) -> Task<Loaded> {
        let (tx, rx) = iced::futures::channel::oneshot::channel();
        let back = url.clone();
        std::thread::spawn(move || {
            let _ = tx.send(blocking(&back).and_then(|bytes| shrink(&bytes)));
        });
        Task::perform(
            async move {
                rx.await
                    .unwrap_or_else(|_| Err("the cover fetch was dropped".to_string()))
            },
            move |image| Loaded {
                url: url.clone(),
                image,
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

    /// Decode, and shrink to fit [`BOUND`] if it does not already.
    ///
    /// `::image` is the crate; `image` in this file is `iced::widget::image`.
    /// `resize` fits inside the box given and keeps the aspect ratio, so one
    /// call says "no longer than this on either side" without arithmetic here.
    /// `Triangle` is a weighted downscale — `Nearest` at these ratios drops
    /// most of the pixels it is averaging over and a portrait comes out
    /// visibly speckled.
    pub fn shrink(bytes: &[u8]) -> Result<Rgba, String> {
        let decoded = ::image::load_from_memory(bytes)
            .map_err(|e| format!("could not decode the cover: {e}"))?;
        let long = decoded.width().max(decoded.height());
        let decoded = match long > BOUND {
            true => decoded.resize(BOUND, BOUND, ::image::imageops::FilterType::Triangle),
            false => decoded,
        };
        let rgba = decoded.into_rgba8();
        Ok(Rgba {
            width: rgba.width(),
            height: rgba.height(),
            pixels: rgba.into_raw(),
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::{Loaded, Rgba, BOUND};
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
    //
    // The shrink is `createImageBitmap`, which is the browser's own decoder
    // and runs it *off the main thread* — so this is the same trade the
    // desktop makes, made with what the platform already has rather than with
    // a decoder compiled into the module. A browser too old to have it throws
    // here and the page draws the derived square, which is correct on its own.
    #[wasm_bindgen(inline_js = r#"
export async function cover(url, bound) {
  let store = null;
  try { store = await caches.open('harken-covers-v1'); } catch (e) { store = null; }
  let response = store ? await store.match(url) : null;
  if (!response) {
    response = await fetch(url, { mode: 'cors' });
    if (!response.ok) throw new Error('HTTP ' + response.status);
    // Put the clone, read the original: a Response body is a stream and can be
    // consumed exactly once, so reading it first leaves the cache nothing.
    if (store) { try { await store.put(url, response.clone()); } catch (e) {} }
  }
  // Decoded from the bytes, never from the URL. A canvas that has drawn a
  // cross-origin image is *tainted* and `getImageData` on it throws a
  // SecurityError — but a blob we are already holding is same-origin whatever
  // it came from, so going through the fetched bytes is what makes the pixels
  // readable at all.
  const source = await createImageBitmap(await response.blob());
  const long = Math.max(source.width, source.height);
  const scale = long > bound ? bound / long : 1;
  const w = Math.max(1, Math.round(source.width * scale));
  const h = Math.max(1, Math.round(source.height * scale));
  const canvas = typeof OffscreenCanvas === 'function'
    ? new OffscreenCanvas(w, h)
    : Object.assign(document.createElement('canvas'), { width: w, height: h });
  const context = canvas.getContext('2d', { willReadFrequently: true });
  context.drawImage(source, 0, 0, w, h);
  source.close();
  return { width: w, height: h, data: context.getImageData(0, 0, w, h).data };
}
"#)]
    extern "C" {
        #[wasm_bindgen(catch)]
        async fn cover(url: &str, bound: u32) -> Result<JsValue, JsValue>;
    }

    /// One field of the object the snippet resolved to.
    fn field(value: &JsValue, name: &str) -> Result<JsValue, String> {
        js_sys::Reflect::get(value, &JsValue::from_str(name))
            .map_err(|_| format!("the cover had no {name}"))
    }

    pub fn fetch(url: String) -> Task<Loaded> {
        let back = url.clone();
        Task::perform(
            async move {
                let value = cover(&back, BOUND).await.map_err(|e| {
                    format!(
                        "could not fetch the cover: {}",
                        e.as_string().unwrap_or_else(|| "failed".into())
                    )
                })?;
                let width = field(&value, "width")?.as_f64().unwrap_or_default() as u32;
                let height = field(&value, "height")?.as_f64().unwrap_or_default() as u32;
                let pixels = js_sys::Uint8Array::new(&field(&value, "data")?).to_vec();
                Ok(Rgba {
                    width,
                    height,
                    pixels,
                })
            },
            move |image| Loaded {
                url: url.clone(),
                image,
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

    /// The whole of the stutter fix, asserted on the thing that was wrong: a
    /// portrait the size of the ones the demo seeds must not reach the
    /// renderer at the size it was fetched.
    ///
    /// The numbers are the real ones — 960×1262 is Haussmann's Bach, and it
    /// was going into the atlas as 4.8 MB to be drawn 132 pixels wide.
    /// Falsify it by deleting the `resize`: the assertion names the megabytes.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn a_portrait_is_shrunk_to_what_is_drawn() {
        use super::BOUND;
        let mut big = ::image::RgbImage::new(960, 1262);
        // Not a flat fill: a solid image encodes to almost nothing and would
        // prove the decoder ran on something no bigger than the header.
        for (x, y, pixel) in big.enumerate_pixels_mut() {
            *pixel = ::image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x ^ y) % 239) as u8]);
        }
        let mut png = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(big)
            .write_to(&mut png, ::image::ImageFormat::Png)
            .expect("the fixture encodes");

        let out = super::imp::shrink(png.get_ref()).expect("a PNG decodes");
        assert!(
            out.width.max(out.height) <= BOUND,
            "a cover reaches the renderer at {}×{}, which is {:.1} MB of atlas \
             for something drawn 132 pixels wide",
            out.width,
            out.height,
            (out.width as f64 * out.height as f64 * 4.0) / 1e6,
        );
        // The aspect ratio is kept, so `ContentFit::Cover` still crops in
        // `view` rather than this having quietly decided what a cover looks
        // like. 960/1262 at a 384 bound is 292×384.
        assert_eq!((out.width, out.height), (292, 384));
        assert_eq!(out.pixels.len(), 292 * 384 * 4);
    }

    /// Anything already small enough is left exactly as it is: shrinking a
    /// 361×295 title page to fit a bound it is already inside would be a
    /// resample that only loses detail.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn a_small_cover_is_left_alone() {
        let small = ::image::RgbImage::new(361, 295);
        let mut png = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(small)
            .write_to(&mut png, ::image::ImageFormat::Png)
            .expect("the fixture encodes");
        let out = super::imp::shrink(png.get_ref()).expect("a PNG decodes");
        assert_eq!((out.width, out.height), (361, 295));
    }
}
