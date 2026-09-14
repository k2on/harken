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
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
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
};

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
    (next: Track) => {
      setTrack(next);
      if (!next.url) {
        // Selected, not playing. The bar says there is nothing to stream,
        // which is what the library looks like when a song was typed in
        // rather than seeded.
        audio.pause();
        return;
      }
      audio.replace(next.url);
      audio.play();
      try {
        audio.setActiveForLockScreen(true, {
          title: next.title,
          artist: next.creator,
          albumTitle: next.album,
        });
      } catch {
        // A platform with no lock-screen transport is a platform with no
        // lock-screen transport. It is not a reason to stop the music.
      }
    },
    [audio],
  );

  const play = useCallback(
    (next: Track, list: Track[]) => {
      setQueue(list);
      start(next);
    },
    [start],
  );

  const at = useMemo(
    () => (track ? queue.findIndex((t) => t.id === track.id) : -1),
    [queue, track],
  );

  // Read through a ref so `skip` does not change identity on every tick, which
  // would re-render the bar and every row under it.
  const state = useRef({ queue, at });
  state.current = { queue, at };

  const skip = useCallback(
    (delta: number) => {
      const { queue: list, at: here } = state.current;
      if (here < 0) return;
      const next = here + delta;
      if (next < 0 || next >= list.length) return;
      start(list[next]);
    },
    [start],
  );

  const toggle = useCallback(() => {
    if (!track?.url) return;
    if (status.playing) audio.pause();
    else audio.play();
  }, [audio, status.playing, track]);

  const seek = useCallback(
    (seconds: number) => {
      if (!track?.url) return;
      void audio.seekTo(Math.max(0, seconds)).catch(() => {});
    },
    [audio, track],
  );

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

  const value = useMemo<Player>(
    () => ({
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
    }),
    [
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
    ],
  );

  return <Context.Provider value={value}>{children}</Context.Provider>;
}
