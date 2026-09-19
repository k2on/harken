/**
 * What this account is listening to, and where.
 *
 * **There is no socket here any more.** There was one: a second `WebSocket`
 * at `/listen`, carrying JSON, dialled and retried beside the log's, with its
 * own `hello` and its own token. That was a second thing to authenticate, a
 * second thing to reconnect, a second thing to keep alive through a proxy,
 * and a second answer to "am I online" — and the two disagreed at the worst
 * possible moment, which on a phone waking up is every morning. It rides
 * `/sync` now, as a [`petros::live`] room: what goes out is `say` and what
 * comes back is `heard`, on the same wire, through the same sign-in, with the
 * same keepalive under it.
 *
 * So what is left is a *state machine* — the session as the server last
 * described it, an outbox the pump drains, and the questions the player asks
 * — and it is deliberately the same one `iced/src/listening.rs` holds. Two
 * clients with two state machines is two answers to "am I the output", and
 * that question has exactly one right answer per device.
 *
 * **The wire is no longer spelt the way the wire spells it**, and that rule
 * going away is the point rather than a regression. It existed because this
 * file used to carry a hand-written copy of the protocol, so a field renamed
 * in Rust would have disagreed with a field renamed here — and the awkward
 * `position_ms` in TypeScript was the tell that somebody else had defined it.
 * There is no copy now. `domain/src/listening.rs` is the one description and
 * these types are *generated* from it, which is a stronger guarantee than a
 * spelling convention: a field that moves is a `tsc` error rather than a
 * mismatch nobody was looking at. What that costs is UniFFI's own spelling —
 * `positionMs`, and an `i64` arriving as a `bigint`.
 *
 * **A device is a login.** `petros-auth` says a session is "one login on one
 * device", so the device id *is* `login.session`. It is never sent: it is
 * what the engine already puts on a room's peer, so the server knows who is
 * talking to it and this side holds the id only so a picker can mark which
 * row is you.
 *
 * **A module-level singleton, like `@petros/client`'s `session()`.** It has
 * to outlive every screen for the same reason that one does, and it is driven
 * by that session's own pump — see [`Wire.pump`], which `peer.ts` hands over
 * as `tick`.
 */

import * as Device_ from 'expo-device';
import { Kind, Verb, type Device, type Doing, type Session, type Track } from 'harken-native';

export { Kind, Verb };
export type { Device, Doing, Session, Track };

/** A device id. A login's session id for a client, an entity for a speaker. */
export type DeviceId = string;

/**
 * What a peer has to be for this to drive it.
 *
 * Structural, so that nothing here imports `peer.ts` and `peer.ts` can import
 * this. Every method is one `#[uniffi::export]` in `domain/src/lib.rs`.
 */
export interface Listener {
  linked(): boolean;
  listenEpoch(): bigint | number;
  listenHere(name: string): void;
  listenReport(queue: Track[], at: number, playing: boolean, positionMs: bigint): void;
  listenDo(doing: Doing): void;
  listenTransfer(to: string | undefined): void;
  // `Option<Session>` arrives as `Session | undefined`, written as an
  // optional field so that a generated type declaring it either way matches.
  listenTake(): { session?: Session | undefined; todo: Doing[] };
}

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

/** What was last reported, so that a quiet second costs nothing. */
type Reported = { at: number; playing: boolean; len: number; positionMs: number };

/** An `i64` crosses UniFFI as a `bigint`, and a clock is arithmetic. */
export const ms = (n: bigint | number): number => Number(n);

/** …and back, which throws on anything that is not whole. */
const i64 = (n: number): bigint => BigInt(Math.round(n));

/** A verb with nothing attached, which is four of the six. */
export function verb(v: Verb): Doing {
  return { verb: v, queue: [], at: 0, positionMs: 0n, playing: true };
}

/** This phone, as somebody else's picker names it. */
function deviceName(): string {
  return Device_.deviceName || Device_.modelName || 'Phone';
}

class Wire {
  /** Where this peer is pointed, so a `file` can be joined to it. */
  server = '';
  /** The session as the server last described it, and when that was by *this*
   *  phone's clock — the server has one and so does every device and they do
   *  not agree, so a scrubber that moves between reports counts from arrival. */
  session: Session | null = null;
  since = 0;

  private device: DeviceId = '';
  private said: Reported | null = null;
  /** What to say on the next pump. An outbox rather than a client handed to
   *  every caller: a button knows what it wants, not where the socket is. */
  private out: Doing[] = [];
  /** …and the two sentences that are not commands, kept apart because they
   *  are not interchangeable with them and there is at most one of each. */
  private moving: { to: DeviceId | undefined } | null = null;
  private report_: { queue: Track[]; at: number; playing: boolean; positionMs: number } | null =
    null;
  /** Which connection this device last introduced itself on. A room is the
   *  server's memory of a socket, so a new socket is a room that has never
   *  heard of this device — and the engine counts connections for this. */
  private epoch = 0;
  private readonly listeners = new Set<() => void>();
  private obey: ((doing: Doing) => void) | null = null;

  /** Point this at a server as a device. Called by the shell, which is the
   *  screen that knows both. */
  open(server: string, device: DeviceId): void {
    if (this.server === server && this.device === device) return;
    this.server = server;
    this.device = device;
    this.forget();
    this.changed();
  }

  /** Leave. A phone that has signed out is not in anybody's picker. */
  close(): void {
    this.server = '';
    this.device = '';
    this.forget();
    this.changed();
  }

  /** What to do when the server tells this device to do something — which it
   *  only ever does to the output. */
  commands(obey: (doing: Doing) => void): void {
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
    return !!this.session && this.session.output === this.device && this.device !== '';
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
    return !!session && !!session.output && session.output !== this.device;
  }

  /** Where a hand-off is going, if one is in flight.
   *
   *  A speaker in the house takes a second or two to fetch anything, so the
   *  sheet draws that row as *connecting* rather than as the one playing —
   *  which is the difference between a press that appears to have done
   *  nothing and one that is visibly under way. */
  get movingTo(): DeviceId | null {
    return this.session?.moving ?? null;
  }

  get me(): DeviceId {
    return this.device;
  }

  /** How far in, counted forward from when the state arrived. */
  get positionMs(): number {
    const session = this.session;
    if (!session) return 0;
    const at = ms(session.positionMs);
    if (!session.playing) return at;
    return at + Math.max(0, Date.now() - this.since);
  }

  /** Ask for something, wherever the sound is. */
  ask(doing: Doing): void {
    this.assume(doing);
    this.out.push(doing);
  }

  /** Move the sound, or stop it everywhere with `null`. */
  transfer(to: DeviceId | null): void {
    // The session about to be handed over is the one this device last
    // reported; forgetting that here means the next report says it again,
    // which is what a device that has just *lost* the output should not do.
    this.said = null;
    this.moving = { to: to ?? undefined };
  }

  /**
   * Say what this device is doing, if it is the one doing it.
   *
   * Called from the player's status rather than from each button, for the
   * reason the lock-screen metadata is set that way: the platform pauses
   * itself when a stream stalls or runs out, and that was nobody's press.
   *
   * It only *queues* the report. What decides whether it is worth saying is
   * [`DRIFT_MS`], and what puts it on the wire is the pump.
   */
  report(queue: Track[], at: number, playing: boolean, positionMs: number): void {
    if (this.elsewhere || !this.session) return;
    const now: Reported = { at, playing, len: queue.length, positionMs };
    const was = this.said;
    const stale =
      !was ||
      was.at !== now.at ||
      was.playing !== now.playing ||
      was.len !== now.len ||
      Math.abs(was.positionMs - now.positionMs) >= DRIFT_MS;
    if (!stale) return;
    this.said = now;
    this.report_ = { queue, at, playing, positionMs };
  }

  /**
   * Move whatever is waiting in each direction, over the log's own socket.
   *
   * Handed to `@petros/client`'s session as its `tick`, so it runs on that
   * pump whether or not anything is mounted — a phone with its screen off is
   * still the device the laptop's pause button is aimed at. It notifies its
   * own subscribers rather than the session's, which is the whole reason
   * `tick` returns nothing: a report a second must not re-run a read model.
   */
  pump(client: Listener): void {
    if (!this.device) return;
    if (!client.linked()) {
      // The room is the server's memory of a socket. With no socket there is
      // no room, and drawing the last thing it said would be a picker full of
      // devices nobody can reach.
      if (this.session !== null || this.epoch !== 0) {
        this.forget();
        this.changed();
      }
      return;
    }
    const epoch = Number(client.listenEpoch());
    if (epoch !== this.epoch) {
      this.epoch = epoch;
      // A fresh connection is a room that has never heard of this device, so
      // it says what it is again — and forgets what it last reported, because
      // nobody over there remembers hearing it.
      this.said = null;
      try {
        client.listenHere(deviceName());
      } catch {
        // A frame that would not go is a frame the next connection will make
        // again. It is not a screen.
      }
    }
    try {
      if (this.moving) {
        client.listenTransfer(this.moving.to);
        this.moving = null;
      }
      for (const doing of this.out) client.listenDo(doing);
      this.out.length = 0;
      const now = this.report_;
      if (now) {
        this.report_ = null;
        client.listenReport(now.queue, now.at, now.playing, i64(now.positionMs));
      }
    } catch {
      // Same: this channel carries what is true *now*, so a frame that could
      // not be sent is worth less than the one after it.
      this.out.length = 0;
      this.report_ = null;
    }

    const heard = client.listenTake();
    if (heard.session) {
      this.session = heard.session;
      this.since = Date.now();
      this.changed();
    }
    // In order, and after the state: two skips are two tracks, where two
    // states are one truth and its predecessor.
    for (const doing of heard.todo) this.obey?.(doing);
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
  private assume(doing: Doing): void {
    const session = this.session;
    if (!session) return;
    const counted = this.positionMs;
    if (doing.verb === Verb.Play)
      this.session = { ...session, positionMs: i64(counted), playing: true };
    else if (doing.verb === Verb.Pause)
      this.session = { ...session, positionMs: i64(counted), playing: false };
    else if (doing.verb === Verb.Seek)
      this.session = { ...session, positionMs: doing.positionMs };
    else return;
    this.since = Date.now();
    this.changed();
  }

  /** Everything the server told us, dropped. Not the server or the device:
   *  those are where this peer is pointed, which a dropped socket has not
   *  changed. */
  private forget(): void {
    this.session = null;
    this.said = null;
    this.out.length = 0;
    this.moving = null;
    this.report_ = null;
    this.epoch = 0;
  }

  private changed(): void {
    for (const listener of this.listeners) listener();
  }
}

/** The one for this process. */
export const listening = new Wire();
