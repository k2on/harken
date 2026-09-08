/**
 * A peer of an Exo server, for React.
 *
 * Everything that decides anything lives in Rust: `TodoClient` comes from
 * `crates/ffi`, which wraps `crates/todo`, which is the one definition of the
 * mutations and the queries. Nothing below re-implements any of it — there is
 * no `apply` here, no CBOR, no notion of what a to-do is. Two `apply`s that
 * disagree make replicas diverge silently, so there is only ever one.
 *
 * What is left for TypeScript is a socket and a clock:
 *
 *   - `takeOutgoing()` hands back encoded frames; we put them on a WebSocket.
 *   - frames that arrive go straight into `recv()`.
 *   - a 50ms tick drives it, because a sans-io client has to be pumped by
 *     someone, and in React that someone is an interval.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Paths } from 'expo-file-system';
import { TodoClient, type TodoClientLike, type TodoItem } from 'exo-todo';

import { MUTATORS_BUILD, installMutators, watchMutators } from './mutators';
// Generated from crates/todo-wasm/src/verbs.rs, rewritten by `just mutators`
// every time the module is. A call site naming a verb the module does not have,
// or passing the wrong arguments to one it does, is a `tsc` error — which is
// where the engine's deliberately generic `mutate(kind, args)` gives up its
// opinion and this picks it back up.
import type { MutationArgs, Verb } from './mutators.gen';


/** How often the transport is pumped. Small enough to feel live. */
const TICK_MS = 50;

/** A millisecond clock, wherever this happens to be running. */
const now = (): number =>
  typeof globalThis.performance?.now === 'function' ? globalThis.performance.now() : Date.now();

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

export type PeerState = {
  items: TodoItem[];
  cursor: number;
  pending: number;
  online: boolean;
  /** The last thing worth saying out loud: an error, a rejection, a status. */
  note: string;
  /** Which mutator module is running. Moves on every hot swap. */
  mutators: number;
  /**
   * How long the last mutation took inside Rust, in milliseconds: the engine,
   * the wasm module and the SQL, but not this render. Development only — it is
   * the number to look at before believing the phone is the slow part.
   */
  lastMutationMs: number | null;
};

export type Peer = PeerState & {
  add: (text: string) => void;
  setDone: (id: string, done: boolean) => void;
  remove: (id: string) => void;
  /**
   * Author any mutation the loaded module understands, by name.
   *
   * The call that does not need a new native build when the domain grows a
   * verb: the module decides what `kind` means, so adding one is a
   * `just mutators` away rather than an `eas build` away — and the arguments
   * are still checked, because they are generated from the same declaration
   * the module dispatches on.
   */
  mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) => void;
  toggleLink: () => void;
};

/** Where this peer's database lives. One file per actor, so two names on one
 *  device are two peers, exactly as `--user` is on the desktop. */
function databasePath(actor: string): string {
  const dir = Paths.document.uri.replace(/^file:\/\//, '').replace(/\/$/, '');
  return `${dir}/exo-demo-${actor.replace(/[^a-zA-Z0-9._-]/g, '_')}.db`;
}

function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function usePeer(actor: string, server: string): Peer {
  const clientRef = useRef<TodoClientLike | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  /** Frames written before the socket opened. A `WebSocket` throws on send
   *  until then, and the first thing a client says is its `Hello`. */
  const backlogRef = useRef<ArrayBuffer[]>([]);
  const openRef = useRef(false);
  const noteRef = useRef('');
  const dirtyRef = useRef(true);
  const lastMsRef = useRef<number | null>(null);

  const [state, setState] = useState<PeerState>({
    items: [],
    cursor: 0,
    pending: 0,
    online: false,
    note: '',
    mutators: 0,
    lastMutationMs: null,
  });

  const snapshot = useCallback((client: TodoClientLike) => {
    setState({
      items: client.list(),
      cursor: Number(client.cursor()),
      pending: client.pendingLen(),
      online: socketRef.current !== null,
      note: noteRef.current,
      mutators: Number(client.mutatorsGeneration()),
      lastMutationMs: lastMsRef.current,
    });
  }, []);

  const disconnect = useCallback((note: string) => {
    const socket = socketRef.current;
    socketRef.current = null;
    openRef.current = false;
    backlogRef.current = [];
    noteRef.current = note;
    dirtyRef.current = true;
    if (socket) {
      socket.onopen = null;
      socket.onmessage = null;
      socket.onerror = null;
      socket.onclose = null;
      try {
        socket.close();
      } catch {
        // Closing a socket that never opened is not worth reporting.
      }
    }
  }, []);

  const connect = useCallback(() => {
    const client = clientRef.current;
    if (!client || socketRef.current) return;
    let socket: WebSocket;
    try {
      socket = new WebSocket(server);
    } catch (e) {
      noteRef.current = `cannot reach ${server} (${messageOf(e)}) — working offline`;
      dirtyRef.current = true;
      return;
    }
    socket.binaryType = 'arraybuffer';
    socketRef.current = socket;
    openRef.current = false;

    socket.onopen = () => {
      openRef.current = true;
      for (const frame of backlogRef.current) socket.send(frame);
      backlogRef.current = [];
      noteRef.current = `connected to ${server}`;
      dirtyRef.current = true;
    };
    socket.onmessage = (event: WebSocketMessageEvent) => {
      if (!(event.data instanceof ArrayBuffer)) return;
      try {
        // Straight into Rust. TypeScript never looks inside a frame.
        client.recv(event.data);
      } catch (e) {
        noteRef.current = messageOf(e);
      }
      dirtyRef.current = true;
    };
    socket.onerror = () => {
      if (socketRef.current === socket) disconnect(`cannot reach ${server} — working offline`);
    };
    socket.onclose = () => {
      if (socketRef.current === socket) disconnect('the link dropped');
    };

    // Ask for everything since our cursor and re-offer everything pending.
    try {
      client.connected();
    } catch (e) {
      noteRef.current = messageOf(e);
    }
    dirtyRef.current = true;
  }, [server, disconnect]);

  // Open the database once per actor, and close it when we are done with it.
  useEffect(() => {
    let client: TodoClientLike;
    try {
      client = TodoClient.open(databasePath(actor), actor);
    } catch (e) {
      noteRef.current = `could not open the database: ${messageOf(e)}`;
      setState((s) => ({ ...s, note: noteRef.current }));
      return;
    }
    clientRef.current = client;

    // The domain arrives as a wasm module rather than being linked in, so it
    // has to be installed before the first mutation — and reinstalled whenever
    // Metro pushes a new one. That second line is the whole hot-reload story:
    // the database, the socket and the React tree all survive it, and only
    // `apply` changes underneath them.
    try {
      installMutators(client);
      noteRef.current = `mutators ${MUTATORS_BUILD}`;
    } catch (e) {
      noteRef.current = `could not install the mutators: ${messageOf(e)}`;
    }
    const unwatch = watchMutators(client, (generation, build) => {
      noteRef.current =
        generation < 0 ? `the new mutators would not load: ${build}` : `mutators ${build} · gen ${generation}`;
      dirtyRef.current = true;
    });

    dirtyRef.current = true;
    connect();
    return () => {
      unwatch();
      disconnect('');
      clientRef.current = null;
      // The Rust object is reference counted; let go of it explicitly rather
      // than waiting for whenever the JS engine gets around to it.
      (client as unknown as { uniffiDestroy?: () => void }).uniffiDestroy?.();
    };
    // `connect` closes over `server`, which is fixed for the life of a screen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [actor]);

  // The pump. A sans-io client has to be driven by someone.
  useEffect(() => {
    const id = setInterval(() => {
      const client = clientRef.current;
      if (!client) return;
      const socket = socketRef.current;
      try {
        for (const frame of client.takeOutgoing()) {
          if (!socket) continue;
          // While offline the outbox is drained and dropped: reconnecting
          // re-offers everything still pending, and the server dedupes what it
          // has already seen.
          if (openRef.current) socket.send(frame);
          else backlogRef.current.push(frame);
        }
        for (const r of client.takeRejections()) {
          noteRef.current = `the server refused a change: ${r.reason}`;
          dirtyRef.current = true;
        }
      } catch (e) {
        noteRef.current = messageOf(e);
        dirtyRef.current = true;
      }
      // Re-reading the list is the expensive part of a frame, so it waits for a
      // reason. An idle tick costs one `takeOutgoing` that returns nothing.
      if (dirtyRef.current) {
        dirtyRef.current = false;
        snapshot(client);
      }
    }, TICK_MS);
    return () => clearInterval(id);
  }, [snapshot]);

  const run = useCallback(
    (f: (client: TodoClientLike) => void) => {
      const client = clientRef.current;
      if (!client) return;
      const started = now();
      try {
        f(client);
      } catch (e) {
        // A mutation the app itself refuses never reaches the pending queue.
        noteRef.current = messageOf(e);
      }
      lastMsRef.current = now() - started;
      // Render what just happened, rather than waiting for the pump.
      //
      // This used to only set the dirty flag and let the 50ms tick pick it up,
      // which put 0-50ms between a tap and the screen moving — on its own more
      // than the whole engine costs for a typical mutation. The tick is for
      // things that arrive on their own; a tap is not one of them.
      dirtyRef.current = false;
      snapshot(client);
    },
    [snapshot],
  );

  return useMemo(
    () => ({
      ...state,
      add: (text: string) => run((c) => void c.add(text)),
      setDone: (id: string, done: boolean) => run((c) => c.setDone(id, done)),
      remove: (id: string) => run((c) => c.remove(id)),
      mutate: <K extends Verb>(kind: K, ...args: ArgsFor<K>) =>
        run((c) => c.mutate(kind, JSON.stringify(args[0] ?? {}))),
      toggleLink: () => {
        if (socketRef.current) disconnect('gone offline — edits pile up locally');
        else connect();
      },
    }),
    [state, run, connect, disconnect],
  );
}
