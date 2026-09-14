/**
 * Where the bytes of a track are.
 *
 * `media.file` is "the file in the media store, which is not the log's
 * business: the bytes travel over HTTP and only the name of them is synced".
 * So it is a name, and turning a name into something a player can open is the
 * client's job — the same job `iced/src/player.rs` does by handing the string
 * straight to an `<audio>` element.
 *
 * Two shapes are worth understanding. An absolute URL is already an answer and
 * is used as it stands, which is what the demo library's Wikimedia recordings
 * are. Anything else is a path in the server's media store and is resolved
 * against the server this peer is pointed at — so a peer working offline has
 * no URL for it, and says so rather than handing the player a relative path it
 * would fail on silently.
 */

export function mediaUrl(file: string, server: string | null): string | null {
  const name = file.trim();
  if (!name) return null;
  if (/^https?:\/\//i.test(name)) return name;
  if (!server) return null;
  return `${server.replace(/\/+$/, '')}/${name.replace(/^\/+/, '')}`;
}
