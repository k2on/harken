/**
 * A peer of a Petros server, for this app.
 *
 * Everything general — the socket, the pump, the hot-swap — is in
 * `@petros/client`. What is left here is what only this app can say: which
 * database to open, what its query returns, and what its verbs are called.
 *
 * Nothing below re-implements any of the engine. There is no `apply` here, no
 * CBOR, no notion of what a song is; two `apply`s that disagree make replicas
 * diverge silently, so there is only ever one and it is in Rust.
 */

import { useEffect, useMemo } from 'react';
import { AppState } from 'react-native';
import { Paths } from 'expo-file-system';
import { held } from '@petros/client';
import { usePeer as usePetrosPeer } from '@petros/client/react';
// Aliased: this file's own `Peer` is the hook's return type, and the native
// one is the object it drives.
import { Peer as NativePeer, PatchOp, type PeerLike, type Song } from 'harken-native';

import { install, watch } from './mutators';
// Generated from the module's own schema section by `just mutators`. A call
// site naming a verb the module does not have, or passing the wrong arguments
// to one it does, is a `tsc` error — which is where the engine's deliberately
// generic `mutate(kind, args)` gives up its opinion and this picks it back up.
import type { MutationArgs, Verb } from './mutators.gen';

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

export type Peer = {
  songs: Song[];
  cursor: number;
  pending: number;
  online: boolean;
  note: string;
  mutators: number;
  lastMutationMs: number | null;
  /** Where this peer is pointed, or null when it is working alone. */
  server: string | null;
  addSong: (title: string, artist: string) => void;
  /** The heart. `true` puts the song on the favourites playlist. */
  setFavorite: (id: string, favorited: boolean) => void;
  removeSong: (id: string) => void;
  mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) => void;
  toggleLink: () => void;
  /** Point it somewhere else, or nowhere, without leaving the screen. */
  setServer: (next: string | null) => void;
};

/** Where this peer's database lives. One file per actor, so two names on one
 *  device are two peers, exactly as `--user` is on the desktop. */
function databasePath(actor: string): string {
  const dir = Paths.document.uri.replace(/^file:\/\//, '').replace(/\/$/, '');
  return `${dir}/harken-${actor.replace(/[^a-zA-Z0-9._-]/g, '_')}.db`;
}

export function usePeer(actor: string, server: string | null): Peer {
  // The list, held here and spliced from what the peer sends. `library()` would
  // hand back every song on every change — at a thousand songs that is a
  // thousand rows and about seventy kilobytes across the bridge for one added
  // title, and the bridge is the expensive part on a phone. `libraryUpdate()`
  // sends what moved: one row, seventy bytes, whatever the library's size.
  //
  // `reset` is not an error. A rebase rolls the optimistic view back and a
  // rollback reports nothing, so no sequence of patches describes it and the
  // peer says to take the whole list again.
  //
  // The copy below is a copy of references, so React sees a new array without
  // anything being decoded twice. That is the part still proportional to the
  // library, and it is the cheap part.
  const peer = usePetrosPeer<PeerLike, Song[]>({
    key: actor,
    server,
    open: () => NativePeer.open(databasePath(actor), actor),
    query: (client, scratch) => {
      // The list lives in the session's scratch, not in a ref. `libraryUpdate`
      // reports what moved *since it was last asked*, and it is asked once per
      // session — so a list held for the life of a component starts empty on
      // the second mount and then receives patches against a list that is not
      // there. Everything is in the database and the screen shows nothing,
      // which reads exactly like "it saves nothing".
      const list = held(scratch, 'library', (): Song[] => []);
      const update = client.libraryUpdate();
      if (update.reset) {
        // In place, because the identity is what the scratch is holding.
        list.length = 0;
        for (const song of update.songs) list.push(song);
        return list.slice();
      }
      for (const patch of update.patches) {
        if (patch.op === PatchOp.Insert) list.splice(patch.at, 0, patch.song!);
        else if (patch.op === PatchOp.Remove) list.splice(patch.at, 1);
        else list[patch.at] = patch.song!;
      }
      return list.slice();
    },
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

  return useMemo(
    () => ({
      songs: peer.data ?? [],
      cursor: peer.cursor,
      pending: peer.pending,
      online: peer.online,
      note: peer.note,
      mutators: peer.mutators,
      lastMutationMs: peer.lastMutationMs,
      server: peer.server,
      addSong: (title: string, artist: string) => peer.run((c) => void c.addSong(title, artist)),
      setFavorite: (id: string, favorited: boolean) =>
        peer.run((c) => (favorited ? c.favorite(id) : c.unfavorite(id))),
      removeSong: (id: string) => peer.run((c) => c.removeSong(id)),
      mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) =>
        peer.run((c) => c.mutate(kind, JSON.stringify(args[0] ?? {}))),
      toggleLink: peer.toggleLink,
      setServer: peer.setServer,
    }),
    [peer],
  );
}
