/**
 * Where a track's bytes are.
 *
 * `file` is one of two things, and which one is not a mode: the demo's
 * recordings are whole URLs into Wikimedia, and a scanned track's is the path
 * the scanner wrote, relative to the media root — which is exactly what
 * `/media/` serves back. Joining it to the server *here* is what keeps that
 * one string one string: the log carries no machine's address, and a client
 * plays what the scanner wrote without either of them knowing where the
 * directory is.
 *
 * The same function as `media_url` in `iced/src/main.rs`, and it has to be:
 * handing the relative path straight to a player is what it exists to stop.
 * A player resolves it against nothing useful, the `/media/` prefix goes
 * missing, and a server with a single-page fallback answers the wrong path
 * with `index.html` and a 200 — so what comes back is HTML that reports only
 * that the resource is unsuitable, naming neither the URL nor the type.
 *
 * A peer working offline has no server to join a relative path to, so it has
 * no URL for one and says so rather than handing the player something that
 * will fail in silence.
 */

/**
 * Percent-encode one path segment.
 *
 * Real libraries are full of spaces, ampersands and the occasional `#`, and
 * `#` is the one that is silently destructive: everything after it is a
 * fragment, so the request goes out for a path that stops mid-filename.
 *
 * `encodeURIComponent` and not `encodeURI`, because this is a segment rather
 * than a URL — `encodeURI` leaves `#`, `?` and `&` alone, which is exactly
 * the set that breaks this.
 */
function segment(part: string): string {
  return encodeURIComponent(part);
}

export function mediaUrl(file: string, server: string | null): string | null {
  if (!file) return null;
  if (/^https?:\/\//i.test(file)) return file;
  if (!server) return null;
  return `${server.replace(/\/+$/, '')}/media/${file.split('/').map(segment).join('/')}`;
}
