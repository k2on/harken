//! The browser client, served with a validator the store cannot give it.
//!
//! `HARKEN_WEB` is a nix store path, and nix sets every file in one to
//! mtime 1 when it registers it. `ServeDir` answers a plain GET with
//! `Last-Modified: Thu, 01 Jan 1970 00:00:01 GMT` and no `Cache-Control`, and
//! from that a browser draws two conclusions that are each correct and
//! together permanent:
//!
//! - **It is fresh for years.** With no `Cache-Control`, freshness is a
//!   heuristic — a tenth of the time since `Last-Modified`, which for 1970 is
//!   about five and a half years. A reload revalidates the page it is *on*;
//!   the module the page imports and the wasm the module fetches are
//!   subresources, taken from the cache without a request.
//! - **And when it does ask, the answer is always no.** A conditional GET
//!   carries `If-Modified-Since: …1970…`, the *new* file's mtime is that same
//!   second, and `ServeDir` answers `304 Not Modified` — about a build that
//!   replaced every byte.
//!
//! So a browser that has ever loaded this client keeps that client until it
//! evicts the cache on its own, whatever is deployed behind it. It presented as
//! a feature vanishing: the listening session moved off its own socket, the
//! server stopped serving `/listen`, and every tab that had been open before
//! went on dialling it and drew an empty device picker over a server that was
//! doing everything right.
//!
//! What this does instead is answer "which build is this?" honestly, once per
//! request, for every file under the directory:
//!
//! - **The validator is the build, not the file.** `ETag` is the store hash of
//!   the directory — the one thing that changes with every rebuild and never
//!   without one — so a `304` means *this build*, and every file of a build
//!   shares it. Off the store (`HARKEN_WEB=iced/web` on a laptop) it is the
//!   module's mtime and length, which is what changes there.
//! - **`no-cache` is not "do not cache".** It is "ask every time", and asking
//!   is one conditional GET per file answered `304` with no body — which for a
//!   fifteen-megabyte module is the whole point of having a cache. The store
//!   hash is answered before `ServeDir` ever opens the file.
//! - **`Last-Modified` is taken off, both ways.** Off the response so a browser
//!   has nothing to be heuristic about, and `If-Modified-Since` off the
//!   request so a browser still holding the 1970 date — every one that loaded
//!   the client before this — gets `200` and the new page rather than the
//!   `304` that was keeping it where it was.
//!
//! The half this cannot do is in `iced/web/index.html`: the page names the
//! module and the wasm with `?v=<build>`, which the nix build fills in, so a
//! browser whose cached copies are heuristically fresh — and therefore never
//! asked about at all — finds the new page pointing at URLs it has never
//! seen. Both halves are needed: the query string reaches a stale cache, and
//! the validator is what stops the next one forming.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

/// The module `index.html` loads, and the file whose change *is* a rebuild
/// when the directory is not a store path.
const MODULE: &str = "pkg/harken-iced_bg.wasm";

/// Serve `dir` at `/`, every miss falling back to its `index.html` — which is
/// what makes a reload of a deep link work in a single-page client — with the
/// build as the validator on every answer.
pub fn router(dir: PathBuf) -> Router {
    let index = dir.join("index.html");
    Router::new()
        .fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(index)))
        .layer(from_fn_with_state(Arc::new(dir), validate))
}

/// Which build this directory is, as an opaque string that changes exactly
/// when the build does.
///
/// A store path carries its own answer in its name: the hash is over every
/// input, so a rebuild that changed nothing has the same one and a rebuild
/// that changed anything has another. Anywhere else the module's mtime and
/// length stand in — on a laptop the file is what changes, and it is what
/// `nix run .#web-build` rewrites.
pub fn build_tag(dir: &Path) -> String {
    if let Some(hash) = store_hash(dir) {
        return hash.to_string();
    }
    match std::fs::metadata(dir.join(MODULE)) {
        Ok(meta) => {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            format!("{mtime:x}-{:x}", meta.len())
        }
        // No module at all: nothing to validate against, so every request is a
        // miss, which is the right answer for a directory that has nothing to
        // be stale about.
        Err(_) => String::from("0"),
    }
}

/// The 32-character hash at the front of a store path's name, if `dir` is one.
fn store_hash(dir: &Path) -> Option<&str> {
    let name = dir
        .strip_prefix("/nix/store")
        .ok()?
        .iter()
        .next()?
        .to_str()?;
    let (hash, _) = name.split_once('-')?;
    (hash.len() == 32 && hash.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(hash)
}

/// The build as an entity tag, quoted the way the header wants it.
fn etag(dir: &Path) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{}\"", build_tag(dir)))
        .unwrap_or_else(|_| HeaderValue::from_static("\"0\""))
}

/// One request: answer it from the tag if the browser already has this build,
/// and stamp the tag on whatever `ServeDir` says otherwise.
async fn validate(State(dir): State<Arc<PathBuf>>, mut req: Request, next: Next) -> Response {
    let tag = etag(&dir);
    let stamp = |mut res: Response| {
        let headers = res.headers_mut();
        headers.remove(header::LAST_MODIFIED);
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        headers.insert(header::ETAG, tag.clone());
        res
    };
    // `If-None-Match` may carry several tags, and `*`. Only an exact match on
    // ours is this build; anything else is a browser holding something else.
    let held = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim() == tag));
    if held {
        return stamp(StatusCode::NOT_MODIFIED.into_response());
    }
    // A date is not a validator here — see the module docs — so `ServeDir`
    // never sees one and cannot answer `304` about a file it has not compared.
    req.headers_mut().remove(header::IF_MODIFIED_SINCE);
    stamp(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_path_is_its_hash() {
        let dir = Path::new("/nix/store/6ilgmw07dbmi7y4klpgc9v5k1nmfbs66-harken-web-0.1.0");
        assert_eq!(build_tag(dir), "6ilgmw07dbmi7y4klpgc9v5k1nmfbs66");
        // …and a name that is not one is not mistaken for one.
        assert!(store_hash(Path::new("/nix/store/not-a-hash")).is_none());
        assert!(store_hash(Path::new("/srv/6ilgmw07dbmi7y4klpgc9v5k1nmfbs66-x")).is_none());
    }

    #[test]
    fn off_the_store_the_module_is_the_build() {
        let dir = std::env::temp_dir().join(format!("harken-web-tag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("pkg")).unwrap();
        assert_eq!(build_tag(&dir), "0", "no module, no build");
        std::fs::write(dir.join(MODULE), b"one build").unwrap();
        let first = build_tag(&dir);
        assert_ne!(first, "0");
        // A rebuild writes a different module. Length changes here so the test
        // does not depend on the filesystem's mtime resolution.
        std::fs::write(dir.join(MODULE), b"another build entirely").unwrap();
        assert_ne!(build_tag(&dir), first);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
