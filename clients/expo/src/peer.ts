/**
 * A peer of a Petros server, for this app.
 *
 * Everything general — the socket, the pump, the hot-swap — is in
 * `@petros/client`. What is left here is what only this app can say: which
 * database to open, what its query returns, and what its verbs are called.
 *
 * Nothing below re-implements any of the engine. There is no `apply` here, no
 * CBOR, no notion of what a to-do is; two `apply`s that disagree make replicas
 * diverge silently, so there is only ever one and it is in Rust.
 */

import { useMemo } from 'react';
import { Paths } from 'expo-file-system';
import { usePeer as usePetrosPeer } from '@petros/client/react';
import { TodoClient, type TodoClientLike, type TodoItem } from 'harken-native';

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
  items: TodoItem[];
  cursor: number;
  pending: number;
  online: boolean;
  note: string;
  mutators: number;
  lastMutationMs: number | null;
  add: (text: string) => void;
  setDone: (id: string, done: boolean) => void;
  remove: (id: string) => void;
  mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) => void;
  toggleLink: () => void;
};

/** Where this peer's database lives. One file per actor, so two names on one
 *  device are two peers, exactly as `--user` is on the desktop. */
function databasePath(actor: string): string {
  const dir = Paths.document.uri.replace(/^file:\/\//, '').replace(/\/$/, '');
  return `${dir}/petros-demo-${actor.replace(/[^a-zA-Z0-9._-]/g, '_')}.db`;
}

export function usePeer(actor: string, server: string): Peer {
  const peer = usePetrosPeer<TodoClientLike, TodoItem[]>({
    key: actor,
    server,
    open: () => TodoClient.open(databasePath(actor), actor),
    query: (client) => client.list(),
    install,
    watch,
  });

  return useMemo(
    () => ({
      items: peer.data ?? [],
      cursor: peer.cursor,
      pending: peer.pending,
      online: peer.online,
      note: peer.note,
      mutators: peer.mutators,
      lastMutationMs: peer.lastMutationMs,
      add: (text: string) => peer.run((c) => void c.add(text)),
      setDone: (id: string, done: boolean) => peer.run((c) => c.setDone(id, done)),
      remove: (id: string) => peer.run((c) => c.remove(id)),
      mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) =>
        peer.run((c) => c.mutate(kind, JSON.stringify(args[0] ?? {}))),
      toggleLink: peer.toggleLink,
    }),
    [peer],
  );
}
