/**
 * A peer of a Petros server, for this app.
 *
 * Everything general — the socket, the pump, the hot-swap — is in
 * `@petros/client`. What is left here is what only this app can say: which
 * database to open, what its queries return, and what its verbs are called.
 *
 * Nothing below re-implements any of the engine. There is no `apply` here, no
 * CBOR, no notion of what a song is; two `apply`s that disagree make replicas
 * diverge silently, so there is only ever one and it is in Rust.
 *
 * What this reads is what `iced` reads, in the same order, through the same
 * functions: the library as a maintained view, and the three lists the sidebar
 * is built from. The folding is in `domain/src/functions.rs` for exactly that
 * reason — two clients that each invented a way to group a library by album
 * would group it two ways.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AppState } from 'react-native';
import { Paths } from 'expo-file-system';
import { held, socketUrl, type Login, type Scratch } from '@petros/client';
import { usePeer as usePetrosPeer } from '@petros/client/react';
// Aliased: this file's own `Peer` is the hook's return type, and the native
// one is the object it drives.
import {
  Peer as NativePeer,
  PatchOp,
  type Album,
  type Artist,
  type Item,
  type PeerLike,
  type Playlist,
} from 'harken-native';

import { install, watch } from './mutators';
// Generated from the module's own schema section by `nix run .#mutators`. A call
// site naming a verb the module does not have, or passing the wrong arguments
// to one it does, is a `tsc` error — which is where the engine's deliberately
// generic `mutate(kind, args)` gives up its opinion and this picks it back up.
import { asId, type MediaId, type MutationArgs, type PlaylistId, type Verb } from './mutators.gen';

/**
 * A verb with no arguments takes none at the call site; one with arguments
 * requires them.
 *
 * The test is against `Record<string, never>` itself rather than its `keyof`,
 * which is `string` — an index signature has every key, so asking whether it
 * has none always answers no.
 */
type ArgsFor<K extends Verb> = MutationArgs[K] extends Record<string, never>
  ? []
  : [args: MutationArgs[K]];

/**
 * What the list is showing, which is what the browser picks.
 *
 * The same four the desktop sidebar offers, and each carries the name it was
 * picked by rather than something to look up: the header draws it, and a
 * library that changed underneath the choice should not blank the header.
 */
export type Source =
  | { kind: 'library' }
  | { kind: 'playlist'; id: PlaylistId; name: string }
  | { kind: 'album'; name: string }
  | { kind: 'artist'; name: string };

export function sourceTitle(source: Source): string {
  return source.kind === 'library' ? 'Library' : source.name;
}

/** What one read of the database produces. Held together because it is read
 *  together: one change re-runs all of it, exactly as `reload_sidebar` does. */
type Shelf = {
  items: Item[];
  /**
   * What the list is showing, for whatever the browser picked.
   *
   * `Source.library` is `items` itself, maintained, and costs nothing here.
   * The others are one query per change, which is the right trade and the
   * desktop's: the maintained view is the list you are looking at most of the
   * time, and re-reading a single album when something moves is a hundred rows
   * rather than the library.
   */
  shown: Item[];
  playlist: PlaylistId | null;
  onPlaylist: number;
  playlists: Playlist[];
  albums: Album[];
  artists: Artist[];
  /** Which album each track is on, joined in memory while drawing.
   *
   *  A map beside the list rather than a field on `Item`, because `album`
   *  belongs to the song kind and the library row is deliberately
   *  kind-neutral — folding it in would put a join behind every list, which is
   *  the thing `media` exists to avoid. */
  albumOf: Record<string, string>;
};

const EMPTY: Shelf = {
  items: [],
  shown: [],
  playlist: null,
  onPlaylist: 0,
  playlists: [],
  albums: [],
  artists: [],
  albumOf: {},
};

export type Peer = Shelf & {
  cursor: number;
  pending: number;
  online: boolean;
  /** Why the server turned this peer away, or null. A sign-in clears it. */
  denied: string | null;
  note: string;
  mutators: number;
  lastMutationMs: number | null;
  /** Where this peer is pointed, or null when it is working alone. */
  server: string | null;
  /** What the browser picked, and how to pick something else. */
  source: Source;
  setSource: (next: Source) => void;
  /** Which playlists a track is already on, read on demand.
   *
   *  Not part of the shelf: it is one track's answer, asked when a sheet
   *  opens, where the shelf is read on every change. Putting it in the shelf
   *  would make every mutation pay for a question nobody is asking. */
  playlistsOf: (id: string) => Playlist[];
  /** Put a track on a playlist, or take it off. */
  setOnPlaylist: (playlist: PlaylistId, id: string, on: boolean) => void;
  /** Make one. The name is trimmed and a blank one is refused by `apply`,
   *  not here — the same rule on every peer. */
  newPlaylist: (name: string) => void;
  mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) => void;
  toggleLink: () => void;
  /** Try the socket now, rather than waiting out the backoff. */
  reconnect: () => void;
  /** Point it somewhere else, or nowhere, without leaving the screen. */
  setServer: (next: string | null) => void;
};

/** Where this peer's database lives. One file per user, so two people on
 *  one device are two peers, exactly as two logins are on the desktop.
 *
 *  Exported because the debug overlay shows it: "the phone is empty" and "the
 *  phone is looking at a different file" are the same screen. */
export function databasePath(user: string): string {
  const dir = Paths.document.uri.replace(/^file:\/\//, '').replace(/\/$/, '');
  return `${dir}/harken-${user.replace(/[^a-zA-Z0-9._-]/g, '_')}.db`;
}

/**
 * The playlist the library view is read against, making one the first time.
 *
 * Every read of a list wants a playlist to report membership against — that is
 * what gives a row its `on_playlist` — so a peer with none has nothing to read
 * against. The first run makes one; after that it is whichever came back
 * first, which is stable because playlists are ordered by when they were made.
 * The same three lines as `Peer::open` in the desktop client, and for the same
 * reason.
 *
 * It has to happen after the module is installed, because creating a playlist
 * is a mutation and a mutation is what the module *is*. `usePeer` installs in
 * an effect that runs before the one that first calls this, so by here there
 * is an `apply` to run.
 */
function firstPlaylist(client: PeerLike): PlaylistId | null {
  let lists = client.playlists();
  if (lists.length === 0) {
    try {
      client.createPlaylist('Favourites');
    } catch {
      // A module that would not install leaves nothing to author with. That is
      // a note in the status line, not a screen that will not render: the
      // library is still readable and the next render tries again.
      return null;
    }
    lists = client.playlists();
  }
  const first = lists[0];
  return first ? asId('playlist', first.id) : null;
}

/**
 * A peer for `login`, pointed at `server` — the server's base URL, from which
 * the socket is derived — or nowhere.
 */
export function usePeer(login: Login, server: string | null): Peer {
  const user = login.user.id;

  // What the browser picked. Kept in a ref as well as in state because the
  // read below runs from `snapshot`, which is called synchronously by `run`
  // before React has re-rendered — so state alone would answer with the
  // selection that was replaced.
  const [source, setSourceState] = useState<Source>({ kind: 'library' });
  const chosen = useRef<Source>(source);

  const read = useCallback(
    (scratch: Scratch, client: PeerLike): Shelf => {
      // The list lives in the session's scratch, not in a ref. `libraryUpdate`
      // reports what moved *since it was last asked*, and it is asked once per
      // session — so a list held for the life of a component starts empty on
      // the second mount and then receives patches against a list that is not
      // there. Everything is in the database and the screen shows nothing,
      // which reads exactly like "it saves nothing".
      const kept = held(scratch, 'library', (): { items: Item[]; playlist: PlaylistId | null } => ({
        items: [],
        playlist: null,
      }));
      if (kept.playlist === null) {
        kept.playlist = firstPlaylist(client);
        // Membership of *that* playlist is what a row reports, so the view
        // has to be told. The next update is a reset, which is why this is
        // asked once per session and not once per render.
        if (kept.playlist !== null) client.showPlaylist(kept.playlist);
      }

      // `library()` would hand back every song on every change — at a thousand
      // songs that is a thousand rows and about seventy kilobytes across the
      // bridge for one added title, and the bridge is the expensive part on a
      // phone. `libraryUpdate()` sends what moved: one row, seventy bytes,
      // whatever the library's size.
      //
      // `reset` is not an error. A rebase rolls the optimistic view back and a
      // rollback reports nothing, so no sequence of patches describes it and
      // the peer says to take the whole list again.
      const update = client.libraryUpdate();
      if (update.reset) {
        // In place, because the identity is what the scratch is holding.
        kept.items.length = 0;
        for (const item of update.items) kept.items.push(item);
      } else {
        for (const patch of update.patches) {
          if (patch.op === PatchOp.Insert && patch.item) kept.items.splice(patch.at, 0, patch.item);
          else if (patch.op === PatchOp.Remove) kept.items.splice(patch.at, 1);
          else if (patch.item) kept.items[patch.at] = patch.item;
        }
      }

      // `trackDetails` carries the track number, the part, the catalogue and
      // who played it as well. Only the album is folded out here, because that
      // is all this screen draws — the rest is a query away when it wants it.
      const albumOf: Record<string, string> = {};
      for (const d of client.trackDetails()) albumOf[d.mediaId] = d.album;

      // A copy of references, so React sees a new array without anything being
      // decoded twice. That is the part still proportional to the library, and
      // it is the cheap part.
      const items = kept.items.slice();
      const where = chosen.current;
      // The same array, not a second copy of it, when the library is what is
      // being shown: the two are the same list and a row memoised on identity
      // should be able to see that.
      const shown =
        where.kind === 'library' || kept.playlist === null
          ? items
          : where.kind === 'playlist'
            ? client.playlist(where.id)
            : where.kind === 'album'
              ? client.album(kept.playlist, where.name)
              : client.artist(kept.playlist, where.name);

      return {
        items,
        shown,
        playlist: kept.playlist,
        onPlaylist: update.onPlaylist,
        playlists: client.playlists(),
        albums: client.albums(),
        artists: client.artists(),
        albumOf,
      };
    },
    [],
  );

  const peer = usePetrosPeer<PeerLike, Shelf>({
    key: user,
    server: server === null ? null : socketUrl(server),
    token: login.token,
    // Who every entry is authored as, and under which login. Both what the
    // server said at sign-in; both checked by it on the way back.
    open: () => NativePeer.open(databasePath(user), user, login.session),
    query: (client, scratch) => read(scratch, client),
    install,
    watch,
  });

  // The OS suspends a backgrounded app and takes the socket with it. To the
  // engine that is indistinguishable from being offline, so coming back is a
  // `Hello` and whatever the log gained meanwhile — no special case, just a
  // nudge to try the socket again rather than waiting out the backoff.
  useEffect(() => {
    const sub = AppState.addEventListener('change', (next) => {
      if (next === 'active') peer.reconnect();
    });
    return () => sub.remove();
  }, [peer]);

  const shelf = peer.data ?? EMPTY;
  const playlist = shelf.playlist;

  // `run` is stable — `@petros/client`'s own `useCallback` closes over the
  // session, which does not change — so this is too, and a list memoised on it
  // is not rebuilt on every tick.
  const { run } = peer;
  const setSource = useCallback(
    (next: Source) => {
      chosen.current = next;
      setSourceState(next);
      // Re-read now, with the new selection, rather than waiting for something
      // to move: a tap that changes what the list is showing is exactly the
      // moment to pay for a query. The desktop does the same thing under
      // `Message::Select`.
      run(() => {});
    },
    [run],
  );

  return useMemo(
    () => ({
      ...shelf,
      source,
      setSource,
      cursor: peer.cursor,
      pending: peer.pending,
      online: peer.online,
      denied: peer.denied,
      note: peer.note,
      mutators: peer.mutators,
      lastMutationMs: peer.lastMutationMs,
      server: peer.server,
      playlistsOf: (id: string) => {
        // `run` is the only way in, and it is synchronous — it calls this
        // before it returns — so a read can borrow it. The cost is one render
        // and a `lastMutationMs` that measured a query; worth it against
        // carrying every track's memberships in the shelf, which would make
        // every change pay for a question only an open sheet asks.
        let on: Playlist[] = [];
        peer.run((c) => {
          on = c.playlistsOf(asId('media', id));
        });
        return on;
      },
      setOnPlaylist: (list: PlaylistId, id: string, on: boolean) =>
        peer.run((c) => {
          const media = asId('media', id);
          if (on) c.addToPlaylist(list, media);
          else c.removeFromPlaylist(list, media);
        }),
      newPlaylist: (name: string) => peer.run((c) => c.createPlaylist(name)),
      mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) =>
        peer.run((c) => c.mutate(kind, JSON.stringify(args[0] ?? {}))),
      toggleLink: peer.toggleLink,
      reconnect: peer.reconnect,
      setServer: peer.setServer,
    }),
    [peer, shelf, playlist, source, setSource],
  );
}

/** The four kinds of id this file hands back out, for a screen that wants to
 *  say what it is holding. */
export type { Album, Artist, Item, MediaId, Playlist, PlaylistId };
