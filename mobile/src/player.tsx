/**
 * What is playing.
 *
 * The desktop client's `player.rs` says the same thing in Rust: hand a URL to
 * something the platform already has and get streaming, buffering, range
 * requests and seeking for free — four problems that would each have to be
 * solved again to play the same bytes ourselves. In a browser that is an
 * `<audio>` element; here it is `expo-audio`, which is ExoPlayer on Android
 * and AVPlayer on iOS. Neither client decodes anything.
 *
 * The one thing this has that the desktop does not is a lock screen. A music
 * app whose transport disappears when the phone locks is not a music app, and
 * `setActiveForLockScreen` is also what keeps Android from stopping background
 * playback after about three minutes.
 *
 * The queue is the list as it stood when play was pressed — a snapshot, not a
 * reference to what the screen is showing — so that changing the source, or
 * somebody else's edit arriving, cannot silently redirect what plays next.
 * That rule is the desktop's too, and it is the reason `play` takes a list.
 *
 * **It is also this account's end of the listening session** (`src/listening.ts`,
 * and `domain/src/listening.rs` for why). One account has one thing playing,
 * however many devices are watching it, so:
 *
 * - every transport verb goes through `ask`, which *sends* when the sound is
 *   on another device and *does it here* otherwise. `obey` is the other half
 *   and is also what the server calls into when it tells this phone to do
 *   something, so a button pressed on a laptop and a button pressed here reach
 *   the same five lines;
 * - what this hook reports as playing is the **session**, not the platform, so
 *   the bar and the sheet draw a laptop's track without either of them
 *   learning that a laptop exists;
 * - and this phone is silent while it is not the output, enforced from the
 *   status rather than at the buttons — losing the sound is not something a
 *   device does, it is told, and the status is the only place that can notice.
 *
 * It is here rather than in a provider of its own because these two things are
 * one thing: the session is what is playing, and so is this.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { Platform } from 'react-native';
import {
  requestNotificationPermissionsAsync,
  setAudioModeAsync,
  useAudioPlayer,
  useAudioPlayerStatus,
} from 'expo-audio';

import {
  listening,
  type Command,
  type Device,
  type Session,
  type Track as WireTrack,
} from '@/listening';
import { mediaUrl } from '@/media';

/**
 * Enough to draw the bar without going back to the list, because the list can
 * change underneath a playing track.
 */
export type Track = {
  id: string;
  title: string;
  creator: string;
  album: string;
  /** What the library says it runs for. The stream's own figure is preferred
   *  once it arrives — a transcode is not always the length the catalogue
   *  claims. */
  ms: number;
  /** Where the bytes are, or null for a track nothing can stream: a title
   *  typed in with no file. It is still selectable, and the bar says so. */
  url: string | null;
  /** The path as the log carries it, beside the URL rather than instead of
   *  it. The URL is this phone's answer; the path is what another device is
   *  handed, because it joins it to *its* server. */
  file: string;
};

/** Both directions across the boundary, in one place. The wire's spelling is
 *  the wire's (`duration_ms`), and this is the only file that has to know it. */
function wireOf(track: Track): WireTrack {
  return {
    id: track.id,
    title: track.title,
    creator: track.creator,
    album: track.album,
    duration_ms: track.ms,
    file: track.file,
  };
}

function trackOf(wire: WireTrack, server: string): Track {
  return {
    id: wire.id,
    title: wire.title,
    creator: wire.creator,
    album: wire.album,
    ms: wire.duration_ms,
    file: wire.file,
    url: mediaUrl(wire.file, server),
  };
}

export type Player = {
  track: Track | null;
  queue: Track[];
  playing: boolean;
  buffering: boolean;
  /** Seconds. */
  position: number;
  /** Seconds — the stream's, if it knows, and the library's otherwise. */
  duration: number;
  /** Whatever the platform said went wrong, or null. */
  error: string | null;
  /** Start one, and make `queue` what skipping moves through. */
  play: (track: Track, queue: Track[]) => void;
  toggle: () => void;
  /** Move through the queue, stopping at either end rather than wrapping —
   *  a list that loops silently is hard to tell from one that is stuck. */
  skip: (delta: number) => void;
  seek: (seconds: number) => void;
  /** Where in the queue the current track is, or -1. */
  at: number;

  /** Every device signed in as this account, or empty when there is no
   *  session — which is what decides whether the bar draws a picker at all. */
  devices: Device[];
  /** The one making the sound, if it is one this phone can name. */
  output: Device | null;
  /** Whether that is this phone. */
  outputsHere: boolean;
  /**
   * Whether the sound is on another of this account's devices.
   *
   * Not the negation of `outputsHere`: a session with *no* output is neither,
   * and there the right answer is to play here and let the report claim it.
   * Without that third case the first tap of the day would need a device
   * picked first, which is a setup step for the common case.
   */
  elsewhere: boolean;
  /** This device's id, so a picker can mark which row is you. */
  me: string;
  /** Move the sound, or stop it everywhere with `null`. */
  pickDevice: (to: string | null) => void;
};

const Context = createContext<Player | null>(null);

export function usePlayer(): Player {
  const player = useContext(Context);
  if (!player) throw new Error('usePlayer outside PlayerProvider');
  return player;
}

export function PlayerProvider({ children }: { children: ReactNode }) {
  // 250ms, because the progress bar is drawn against it and the default 500
  // reads as a stutter. Nothing else in the app polls at all.
  const audio = useAudioPlayer(undefined, { updateInterval: 250 });
  const status = useAudioPlayerStatus(audio);

  const [track, setTrack] = useState<Track | null>(null);
  const [queue, setQueue] = useState<Track[]>([]);

  // Once, for the app. Keeping the session active is what lets the transport
  // survive a locked screen; ducking rather than taking exclusive focus is
  // what a music app does when a navigation prompt speaks over it.
  useEffect(() => {
    void setAudioModeAsync({
      playsInSilentMode: true,
      shouldPlayInBackground: true,
      interruptionMode: 'duckOthers',
    }).catch(() => {
      // An audio session that will not configure is not worth a screen. The
      // player still plays; it just may not survive the lock screen.
    });
    if (Platform.OS === 'android') {
      // The notification *is* the transport when the screen is off, and
      // Android 13 asks before showing one.
      void requestNotificationPermissionsAsync().catch(() => {});
    }
  }, []);

  const start = useCallback(
    (next: Track, positionMs = 0, playing = true) => {
      setTrack(next);
      if (!next.url) {
        // Selected, not playing. The bar says there is nothing to stream,
        // which is what the library looks like when a song was typed in
        // rather than seeded.
        audio.pause();
        return;
      }
      audio.replace(next.url);
      // A hand-off arrives mid-track, and the seek is issued before the
      // platform has read any of the stream. It queues it against the load
      // rather than refusing, and the worst case is a second of the beginning
      // — which is why this is not worth waiting for a status to do properly.
      if (positionMs > 0) void audio.seekTo(positionMs / 1000).catch(() => {});
      if (playing) audio.play();
      else audio.pause();
    },
    [audio],
  );

  /** Put something on the lock screen, or take it off. */
  const announce = useCallback(
    (what: { title: string; creator: string; album: string } | null) => {
      try {
        if (what) {
          audio.setActiveForLockScreen(true, {
            title: what.title,
            artist: what.creator,
            albumTitle: what.album,
          });
        } else {
          audio.setActiveForLockScreen(false);
        }
      } catch {
        // A platform with no lock-screen transport is a platform with no
        // lock-screen transport. It is not a reason to stop the music.
      }
    },
    [audio],
  );

  const at = useMemo(
    () => (track ? queue.findIndex((t) => t.id === track.id) : -1),
    [queue, track],
  );

  // Read through a ref so `skipHere` does not change identity on every tick,
  // which would re-render the bar and every row under it.
  const state = useRef({ queue, at });
  state.current = { queue, at };

  const skipHere = useCallback(
    (delta: number) => {
      const { queue: list, at: here } = state.current;
      if (here < 0) return;
      const next = here + delta;
      if (next < 0 || next >= list.length) return;
      start(list[next]);
    },
    [start],
  );

  // ---- the listening session -------------------------------------------
  //
  // One account has one thing playing. What follows is this phone's end of
  // that: the session as the server last described it, what to do when the
  // server tells this phone to do something, and where a transport verb goes.

  const [session, setSession] = useState<Session | null>(listening.session);
  useEffect(() => listening.subscribe(() => setSession(listening.session)), []);

  const me = listening.me;
  const elsewhere = !!session && session.output !== null && session.output !== me;
  const outputsHere = !!session && session.output === me && me !== '';

  /** Do it here. Reached from `ask`, and from the server — which only ever
   *  tells the device that *is* the output. */
  const obey = useCallback(
    (command: Command) => {
      switch (command.do) {
        case 'play':
          audio.play();
          break;
        case 'pause':
          audio.pause();
          break;
        case 'next':
          skipHere(1);
          break;
        case 'previous':
          skipHere(-1);
          break;
        case 'seek':
          void audio.seekTo(Math.max(0, command.position_ms / 1000)).catch(() => {});
          break;
        case 'start': {
          // The only place a `file` becomes a URL on this side: a hand-off
          // carries the log's path so that each device resolves it against
          // its own server.
          const list = command.queue.map((wire) => trackOf(wire, listening.server));
          setQueue(list);
          const next = list[command.at];
          if (next) start(next, command.position_ms, command.playing);
          break;
        }
      }
    },
    [audio, skipHere, start],
  );
  useEffect(() => {
    listening.commands(obey);
  }, [obey]);

  /**
   * Where a transport verb goes, and the only place that is decided.
   *
   * If the sound is on another of this account's devices the verb is a
   * *message*. Otherwise it is an instruction — either this phone is the
   * output, or nothing is and the report below claims it.
   */
  const ask = useCallback(
    (command: Command) => {
      if (listening.elsewhere) listening.ask(command);
      else obey(command);
    },
    [obey],
  );

  const play = useCallback(
    (next: Track, list: Track[]) => {
      if (listening.elsewhere) {
        listening.ask({
          do: 'start',
          queue: list.map(wireOf),
          at: Math.max(
            0,
            list.findIndex((t) => t.id === next.id),
          ),
          position_ms: 0,
          playing: true,
        });
        return;
      }
      // Not through `obey`: these tracks already carry a URL this phone
      // resolved, and sending them the long way round would need a server to
      // resolve them again — which a peer working alone does not have.
      setQueue(list);
      start(next);
    },
    [start],
  );

  const skip = useCallback(
    (delta: number) => ask(delta > 0 ? { do: 'next' } : { do: 'previous' }),
    [ask],
  );

  const seek = useCallback(
    (seconds: number) => ask({ do: 'seek', position_ms: Math.round(Math.max(0, seconds) * 1000) }),
    [ask],
  );

  // Play and pause are two verbs rather than one toggle, and the moment the
  // sound might be on another device that stops being a detail: "the other one
  // of whatever you are" is not something a phone can mean about a laptop. So
  // which it is gets decided against whichever is actually making the sound.
  const toggle = useCallback(() => {
    const playing = elsewhere ? !!session?.playing : status.playing;
    ask(playing ? { do: 'pause' } : { do: 'play' });
  }, [ask, elsewhere, session?.playing, status.playing]);

  const pickDevice = useCallback((to: string | null) => listening.transfer(to), []);

  // The wire's spelling of the queue, once per change rather than once per
  // status tick — the report decides staleness after it is handed the list,
  // and a thousand rows converted four times a second would give back exactly
  // what maintaining the view bought.
  const wireQueue = useMemo(() => queue.map(wireOf), [queue]);

  // Silent unless it is the output, and honest about what it is doing when it
  // is. Both from the status rather than from the buttons: the platform pauses
  // itself when a stream stalls or runs out, and a device that has lost the
  // sound is *told* rather than deciding.
  useEffect(() => {
    if (elsewhere) {
      if (status.playing) {
        // The element started while the sound is somewhere else, and nothing
        // in this app asked it to — every button here goes through `ask`. So
        // it was the lock screen, whose buttons `expo-audio` wires straight to
        // its own player and does not offer to hand over.
        //
        // That is enough to forward it: go quiet, and pass the press on to
        // whichever device is actually playing. Safe even when it was *not* a
        // press — the tail of a hand-off made while playing looks the same —
        // because `play` is a verb and not a toggle, so asking a device that
        // is already playing to play is nothing.
        audio.pause();
        listening.ask({ do: 'play' });
      }
      return;
    }
    // A phone that has never played anything says nothing, so opening the app
    // does not quietly claim the sound from the laptop that is using it.
    if (!track) return;
    listening.report(
      wireQueue,
      at < 0 ? 0 : at,
      status.playing,
      Math.round(status.currentTime * 1000),
    );
  }, [elsewhere, audio, track, wireQueue, at, status.playing, status.currentTime]);

  // What the lock screen says is the *session*, so a phone watching a laptop
  // still carries the transport: the notification is the one control that is
  // reachable without unlocking, and "nothing is playing" would be a lie about
  // a track this account is in the middle of.
  //
  // Best effort, and worth saying which part. The metadata is ours to set and
  // this sets it; what the platform *draws* is its own audio session's, which
  // on a phone that is not the output has no source loaded — so a notification
  // may not appear at all until this phone has played something, and while the
  // sound is elsewhere it will show paused whatever the laptop is doing.
  // Fixing that properly means a foreground service of our own rather than
  // `expo-audio`'s, which is a native module this app does not have.
  const shown = elsewhere ? (session?.queue[session.at] ?? null) : null;
  const shownId = shown?.id ?? track?.id ?? null;
  useEffect(() => {
    const what = elsewhere
      ? shown && { title: shown.title, creator: shown.creator, album: shown.album }
      : track && { title: track.title, creator: track.creator, album: track.album };
    announce(what ?? null);
    // Keyed on which track it is rather than on the object, so a report a
    // second does not reset the notification a second.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shownId, elsewhere, announce]);

  // A scrubber drawn from somebody else's device has nothing to re-render it
  // between reports, and those are a second apart. Four times a second while
  // it matters, and not at all otherwise.
  const [tickCount, tick] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    if (!elsewhere || !session?.playing) return;
    const id = setInterval(tick, 250);
    return () => clearInterval(id);
  }, [elsewhere, session?.playing]);

  // The end of a track arrives as a status flag rather than an event, so it
  // has to be read once and not once per status update — `didJustFinish` stays
  // set until the next source replaces it, and acting on it twice skips two.
  const finished = useRef(false);
  useEffect(() => {
    if (!status.didJustFinish) {
      finished.current = false;
      return;
    }
    if (finished.current) return;
    finished.current = true;
    skip(1);
  }, [status.didJustFinish, skip]);

  const value = useMemo<Player>(() => {
    const devices = session?.devices ?? [];
    const output = devices.find((d) => d.id === session?.output) ?? null;
    // What the bar draws is the *session*, so a phone watching a laptop shows
    // the laptop's track, its clock and its state — and nothing that draws
    // this has to learn that a laptop exists. When the sound is here, or
    // nowhere yet, it is the platform, because that answer is a frame fresher
    // than any broadcast could be.
    if (elsewhere) {
      const wire = session?.queue[session.at] ?? null;
      const shown = wire ? trackOf(wire, listening.server) : null;
      return {
        track: shown,
        queue: (session?.queue ?? []).map((t) => trackOf(t, listening.server)),
        at: session?.at ?? -1,
        playing: !!session?.playing,
        buffering: false,
        position: listening.positionMs / 1000,
        duration: (wire?.duration_ms ?? 0) / 1000,
        error: null,
        play,
        toggle,
        skip,
        seek,
        devices,
        output,
        outputsHere,
        elsewhere,
        me,
        pickDevice,
      };
    }
    return {
      track,
      queue,
      at,
      playing: status.playing,
      buffering: status.isBuffering,
      position: status.currentTime,
      // The element reports its own duration once it has read enough of the
      // stream; until then — and forever, for a live one — the catalogue's
      // figure is the only one there is.
      duration:
        Number.isFinite(status.duration) && status.duration > 0
          ? status.duration
          : (track?.ms ?? 0) / 1000,
      error: status.error,
      play,
      toggle,
      skip,
      seek,
      devices,
      output,
      outputsHere,
      elsewhere,
      me,
      pickDevice,
    };
  }, [
    track,
    queue,
    at,
    status.playing,
    status.isBuffering,
    status.currentTime,
    status.duration,
    status.error,
    play,
    toggle,
    skip,
    seek,
    session,
    elsewhere,
    outputsHere,
    me,
    pickDevice,
    // The extrapolated clock moves without the session doing, so the ticker
    // above has to be a dependency or the memo would hold yesterday's second.
    tickCount,
  ]);

  return <Context.Provider value={value}>{children}</Context.Provider>;
}
