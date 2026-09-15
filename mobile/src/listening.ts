/**
 * The other socket: what this account is listening to, and where.
 *
 * `/sync` carries the log and this does not. Everything about why is in
 * `domain/src/listening.rs`, which is where the protocol is *defined* — this
 * file is the TypeScript that has to agree with it, and nothing more. The one
 * rule it exists to answer is the same one the desktop asks: **this device
 * makes a sound only while it is the output.**
 *
 * **The shapes below are the wire, spelt the way the wire spells it.**
 * `position_ms`, not `positionMs`. Renaming on the way in would be a second
 * description of the protocol living in the client that reads it, and the
 * first time a field moved the two would disagree somewhere nobody was
 * looking. The awkwardness is the point: a field that looks foreign is a
 * field somebody else defined.
 *
 * **A device is a login.** `petros-auth` says a session is "one login on one
 * device", so the device id *is* `login.session` — already issued, already
 * stable across a relaunch, and honestly new when somebody signs out and back
 * in.
 *
 * **A module-level singleton, like `@petros/client`'s `session()`.** The
 * socket has to outlive every screen for the same reason the peer's does, and
 * the player provider that drives it is mounted at the root; the screen that
 * knows which server this phone is pointed at calls [`point`], and everything
 * else borrows what is here.
 */

import * as Device_ from 'expo-device';

// ---------------------------------------------------------------------------
// The wire. `domain/src/listening.rs` is the definition; this is the mirror.
// ---------------------------------------------------------------------------

export type DeviceId = string;

/** A track, carried rather than looked up: `file` is the log's path, and each
 *  device joins it to its own server. */
export type Track = {
  id: string;
  title: string;
  creator: string;
  album: string;
  duration_ms: number;
  file: string;
};

export type Device = { id: DeviceId; name: string; audible: boolean };

export type Session = {
  output: DeviceId | null;
  devices: Device[];
  queue: Track[];
  at: number;
  playing: boolean;
  position_ms: number;
};

export type Command =
  | { do: 'play' }
  | { do: 'pause' }
  | { do: 'next' }
  | { do: 'previous' }
  | { do: 'seek'; position_ms: number }
  | { do: 'start'; queue: Track[]; at: number; position_ms: number; playing: boolean };

type Say =
  | { say: 'hello'; token: string; device: DeviceId; name: string; audible: boolean }
  | { say: 'report'; queue: Track[]; at: number; playing: boolean; position_ms: number }
  | { say: 'do'; command: Command }
  | { say: 'transfer'; to: DeviceId | null };

type Hear =
  | { hear: 'denied'; reason: string }
  | { hear: 'state'; session: Session }
  | { hear: 'do'; command: Command };

// ---------------------------------------------------------------------------

/** A dropped socket is retried on this cadence. Slower than the engine's,
 *  because nothing here is durable: what a device missed while it was away is
 *  the whole state, and it arrives complete on the next `hello`. */
const RETRY_MS = 3_000;

/**
 * How far the position may drift before the output says so again.
 *
 * One number doing three jobs, which is why it is one rule rather than a
 * heartbeat plus a seek test: while playing the position crosses it about once
 * a second, so it *is* the heartbeat; while paused it never moves, so a paused
 * output is silent; and a seek crosses it at once however long ago the last
 * report was.
 */
const DRIFT_MS = 1_100;

type Reported = { at: number; playing: boolean; len: number; position_ms: number };

/** Where the listening socket is, from where the server is. The same rule
 *  `socketUrl` follows in `@petros/client`, with the other path. */
export function listenUrl(server: string): string {
  const base = server.trim().replace(/\/+$/, '');
  if (base.startsWith('https://')) return `wss://${base.slice(8)}/listen`;
  if (base.startsWith('http://')) return `ws://${base.slice(7)}/listen`;
  if (base.startsWith('ws://') || base.startsWith('wss://')) return `${base}/listen`;
  return `ws://${base}/listen`;
}

/** This phone, as somebody else's picker names it. */
function deviceName(): string {
  return Device_.deviceName || Device_.modelName || 'Phone';
}

class Wire {
  /** Where this peer is pointed, so the provider can join a `file` to it. */
  server = '';
  /** The session as the server last described it, and when that was by *this*
   *  phone's clock — the server has one and so does every device and they do
   *  not agree, so a scrubber that moves between reports counts from arrival. */
  session: Session | null = null;
  since = 0;
  /** The server's last word on this token. The socket is gone behind it, and
   *  re-dialling with the same token would get the same answer. */
  denied: string | null = null;

  private token = '';
  private device: DeviceId = '';
  private socket: WebSocket | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private said: Reported | null = null;
  private readonly listeners = new Set<() => void>();
  private obey: ((command: Command) => void) | null = null;

  /** Point this at a server as a device, and dial. Called by the screen that
   *  knows which server this phone is on. */
  point(server: string, token: string, device: DeviceId): void {
    if (this.server === server && this.token === token && this.device === device) return;
    this.server = server;
    this.token = token;
    this.device = device;
    this.denied = null;
    this.session = null;
    this.said = null;
    this.drop();
    this.dial();
  }

  /** Leave. A phone that has signed out is not in anybody's picker. */
  close(): void {
    this.token = '';
    this.server = '';
    this.drop();
    this.session = null;
    this.changed();
  }

  /** What to do when the server tells this device to do something — which it
   *  only ever does to the output. */
  commands(obey: (command: Command) => void): void {
    this.obey = obey;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Whether this device is the one making the sound. */
  get outputsHere(): boolean {
    return this.session?.output === this.device && this.device !== '';
  }

  /**
   * Whether the sound is on another of this account's devices — which is when
   * a transport button is a message rather than an instruction.
   *
   * Not the negation of `outputsHere`: a session with *no* output is neither,
   * and there the right answer is to play here and let the report claim it.
   */
  get elsewhere(): boolean {
    const session = this.session;
    return !!session && session.output !== null && session.output !== this.device;
  }

  get me(): DeviceId {
    return this.device;
  }

  /** How far in, counted forward from when the state arrived. */
  get positionMs(): number {
    const session = this.session;
    if (!session) return 0;
    if (!session.playing) return session.position_ms;
    return session.position_ms + Math.max(0, Date.now() - this.since);
  }

  /** Ask for something, wherever the sound is. */
  ask(command: Command): void {
    this.assume(command);
    this.say({ say: 'do', command });
  }

  /** Move the sound, or stop it everywhere with `null`. */
  transfer(to: DeviceId | null): void {
    // The session about to be handed over is the one this device last
    // reported; forgetting that here means the next report says it again,
    // which is what a device that has just *lost* the output should not do.
    this.said = null;
    this.say({ say: 'transfer', to });
  }

  /**
   * Say what this device is doing, if it is the one doing it.
   *
   * Called from the player's status rather than from each button, for the
   * reason the lock-screen metadata is set that way: the platform pauses
   * itself when a stream stalls or runs out, and that was nobody's press.
   */
  report(queue: Track[], at: number, playing: boolean, position_ms: number): void {
    if (this.elsewhere || !this.socket) return;
    const now: Reported = { at, playing, len: queue.length, position_ms };
    const was = this.said;
    const stale =
      !was ||
      was.at !== now.at ||
      was.playing !== now.playing ||
      was.len !== now.len ||
      Math.abs(was.position_ms - now.position_ms) >= DRIFT_MS;
    if (!stale) return;
    this.said = now;
    this.say({ say: 'report', queue, at, playing, position_ms });
  }

  /**
   * Apply what was just asked for to the copy of the session this device
   * holds, so the bar answers the tap now rather than on the round trip.
   *
   * Optimistic the way the engine's own view is, and corrected the same way.
   * Only the three that can be guessed from here: `next` and `start` change
   * *which* track it is, which means guessing against a queue another device
   * holds, and a bar showing the wrong title is worse than a bar a moment
   * behind.
   */
  private assume(command: Command): void {
    const session = this.session;
    if (!session) return;
    const counted = this.positionMs;
    if (command.do === 'play') this.session = { ...session, position_ms: counted, playing: true };
    else if (command.do === 'pause')
      this.session = { ...session, position_ms: counted, playing: false };
    else if (command.do === 'seek')
      this.session = { ...session, position_ms: command.position_ms };
    else return;
    this.since = Date.now();
    this.changed();
  }

  private say(frame: Say): void {
    if (this.socket && this.socket.readyState === 1) this.socket.send(JSON.stringify(frame));
  }

  private dial(): void {
    if (!this.token || !this.server || this.denied) return;
    let socket: WebSocket;
    try {
      socket = new WebSocket(listenUrl(this.server));
    } catch {
      this.later();
      return;
    }
    this.socket = socket;
    socket.onopen = () => {
      // The first frame, and the only one carrying a token.
      socket.send(
        JSON.stringify({
          say: 'hello',
          token: this.token,
          device: this.device,
          name: deviceName(),
          // A phone can always be heard, which is the whole reason it is the
          // interesting device in this feature.
          audible: true,
        } satisfies Say),
      );
    };
    socket.onmessage = (event) => {
      if (typeof event.data !== 'string') return;
      let frame: Hear;
      try {
        frame = JSON.parse(event.data) as Hear;
      } catch {
        return;
      }
      if (frame.hear === 'state') {
        this.session = frame.session;
        this.since = Date.now();
        this.changed();
      } else if (frame.hear === 'do') {
        this.obey?.(frame.command);
      } else if (frame.hear === 'denied') {
        this.denied = frame.reason;
        this.drop();
        this.session = null;
        this.changed();
      }
      // Anything else is a sentence a newer server invented. Carrying on is
      // the right answer; dropping the socket over it is not.
    };
    const gone = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.session = null;
      this.said = null;
      this.changed();
      this.later();
    };
    socket.onclose = gone;
    socket.onerror = gone;
  }

  private later(): void {
    if (this.timer || this.denied || !this.token) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      if (!this.socket) this.dial();
    }, RETRY_MS);
  }

  private drop(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    const socket = this.socket;
    this.socket = null;
    if (socket) {
      socket.onopen = null;
      socket.onmessage = null;
      socket.onclose = null;
      socket.onerror = null;
      try {
        socket.close();
      } catch {
        // A socket that will not close is a socket that is already gone.
      }
    }
  }

  private changed(): void {
    for (const listener of this.listeners) listener();
  }
}

/** The one for this process. */
export const listening = new Wire();
