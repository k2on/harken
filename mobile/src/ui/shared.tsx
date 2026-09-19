/**
 * Album and playlist art that flies between the row and the page it opens.
 *
 * A record's cover is the one thing on screen that is the *same object* on
 * both sides of a navigation: the 48px square in Your Library and the 168px
 * one at the top of the page are one album. A cross-fade says they are two
 * pictures that happened to follow each other; carrying it across says the
 * page came out of the row you touched, and it is the cheapest way to make a
 * push feel like a place rather than a screen.
 *
 * **Written here rather than taken from a library, because there is no
 * library to take.** Reanimated's `sharedTransitionTag` was experimental in 3
 * and is gone in 4, which is what this app pins; `react-native-shared-element`
 * is a native module, and a native module here moves `nodeModulesHash` *and*
 * `mobile/gradle-deps.json`, the second of which cannot be re-recorded on a
 * laptop. So it is a hundred lines over what is already pinned, and nothing
 * about the build changes.
 *
 * How it works, which is the whole of it:
 *
 * - every `SharedArt` registers a way to measure itself, under its record's id
 *   and which end it is — the `row` in a list, or the `page` it opens;
 * - touching a row measures it and remembers the rectangle, then navigates;
 * - the page's own `SharedArt` says so when it has been laid out, and if there
 *   is a remembered rectangle waiting the flight starts: a copy of the square
 *   is drawn over everything and animated from the one rectangle to the other,
 *   with both real ends hidden until it lands;
 * - going back runs it the other way.
 *
 * **Nothing here can fail loudly.** A measurement that comes back empty, a
 * page with no matching row, an id nobody registered — each of them simply
 * means no flight, and the push looks exactly as it did before. That is
 * deliberate: an animation is the one kind of code where being absent is a
 * far better failure than being wrong.
 *
 * **Not verified on a device** — nothing in this container can build an APK.
 * What is verified is that it type-checks and that every path through it ends
 * in "draw nothing extra".
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
import { StyleSheet, View } from 'react-native';
import Animated, {
  Easing,
  interpolate,
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from 'react-native-reanimated';

import { radius, useTheme, type Theme } from '@/theme';
import { Artwork } from './artwork';
import type { IconName } from './icon';

/** Where a square is, in window coordinates, and what shape it is there. */
type ArtRect = { x: number; y: number; size: number; corner: number };

/** Which end of the journey a square is. Two `SharedArt`s can carry the same
 *  id at once — the row is still mounted under the page that came out of it —
 *  so the registry is keyed by both. */
type End = 'row' | 'page';

type Flight = { id: string; seed: string; glyph: IconName; from: ArtRect; to: ArtRect };

/** Long enough to follow with your eye, short enough not to be in the way of
 *  the next tap. Matched to the stack's own push by feel rather than by a
 *  number the router exposes, because it does not expose one. */
const FLIGHT = 300;
/** Out fast, in slow: the standard "emphasized" shape, which reads as a thing
 *  with weight being set down rather than a value being tweened. */
const EASE = Easing.bezier(0.2, 0, 0, 1);

type Shared = {
  register: (id: string, end: End, measure: () => Promise<ArtRect | null>) => () => void;
  /** Remember where a row is, because something is about to open it. */
  lift: (id: string) => void;
  /** A page's square has been laid out; fly to it if a row is waiting. */
  claim: (id: string, seed: string, glyph: IconName) => void;
  /** Fly back to the row this page came out of. Call it *before* navigating. */
  drop: (id: string, seed: string, glyph: IconName) => void;
  /** The record whose two ends must not draw themselves right now. */
  hidden: string | null;
};

const Ctx = createContext<Shared | null>(null);

/** A stable name for one record, so both ends agree without either knowing
 *  about the other. The kind is in it because an album and an artist can share
 *  a name and do not share a cover. */
export function artId(kind: string, key: string): string {
  return `${kind}:${key}`;
}

export function SharedArtProvider({ children }: { children: ReactNode }) {
  // Refs rather than state for everything except `hidden`: a registry that
  // re-rendered on every mounted square would re-render the shell on every
  // scroll that recycled a row.
  const ends = useRef(new Map<string, () => Promise<ArtRect | null>>());
  // Two different questions, which is why they are two refs. `waiting` is
  // "a page is about to open and should fly to itself", answered once and
  // then spent. `home` is "where the row for this record was when it was
  // touched", which has to survive the forward flight because it is the
  // destination of the one back — and it is the remembered rectangle rather
  // than a fresh measurement for the reason `drop` gives.
  const waiting = useRef<{ id: string; rect: ArtRect } | null>(null);
  const home = useRef<{ id: string; rect: ArtRect } | null>(null);
  const [flight, setFlight] = useState<Flight | null>(null);

  const register = useCallback((id: string, end: End, measure: () => Promise<ArtRect | null>) => {
    const key = `${end}\u0000${id}`;
    ends.current.set(key, measure);
    return () => {
      // Only if it is still ours: a row remounting during a recycle registers
      // the new one before the old one's cleanup runs.
      if (ends.current.get(key) === measure) ends.current.delete(key);
    };
  }, []);

  const measure = useCallback(async (id: string, end: End) => {
    const at = ends.current.get(`${end}\u0000${id}`);
    return at ? at() : null;
  }, []);

  const lift = useCallback(
    (id: string) => {
      void measure(id, 'row').then((rect) => {
        waiting.current = rect ? { id, rect } : null;
        home.current = rect ? { id, rect } : null;
      });
    },
    [measure],
  );

  const claim = useCallback(
    (id: string, seed: string, glyph: IconName) => {
      const from = waiting.current;
      if (!from || from.id !== id) return;
      waiting.current = null;
      void measure(id, 'page').then((to) => {
        if (to) setFlight({ id, seed, glyph, from: from.rect, to });
      });
    },
    [measure],
  );

  const drop = useCallback(
    (id: string, seed: string, glyph: IconName) => {
      // Back to where the row *was* when it was touched, rather than where it
      // is now. The list is still mounted underneath, but a screen mid-pop is
      // parked off to one side or under a parallax offset, so measuring it
      // here answers with a rectangle nobody can see. The remembered one is
      // right unless the list has been scrolled since — and it cannot have
      // been, because it has been behind this page the whole time.
      const back = home.current;
      if (!back || back.id !== id) return;
      void measure(id, 'page').then((from) => {
        if (from) setFlight({ id, seed, glyph, from, to: back.rect });
      });
    },
    [measure],
  );

  const land = useCallback(() => setFlight(null), []);

  const value = useMemo<Shared>(
    () => ({ register, lift, claim, drop, hidden: flight?.id ?? null }),
    [register, lift, claim, drop, flight],
  );

  return (
    <Ctx.Provider value={value}>
      {/* `children` keeps its identity across a flight, so React skips the
          whole app's subtree and only the two squares — which read `hidden`
          through the context — actually re-render. */}
      {children}
      {flight ? <InFlight flight={flight} onLand={land} /> : null}
    </Ctx.Provider>
  );
}

/**
 * The copy that does the travelling.
 *
 * It is drawn at the *destination's* size and scaled down to the source's,
 * rather than having its width and height animated: a `LinearGradient` whose
 * box changes every frame is re-laid-out and re-rendered natively every frame,
 * where a transform is a matrix on a layer that is already there. The corner
 * is then divided by that scale, so what is on screen is the radius the two
 * ends agreed on rather than the destination's shrunk by a fifth.
 */
function InFlight({ flight, onLand }: { flight: Flight; onLand: () => void }) {
  // Asked for here rather than carried on the flight: the square is derived
  // from the name and the palette, and both ends of a navigation are in the
  // same one. Threading it through would be a third thing to keep in step.
  const theme = useTheme();
  const p = useSharedValue(0);
  useEffect(() => {
    p.value = 0;
    p.value = withTiming(1, { duration: FLIGHT, easing: EASE }, (done) => {
      if (done) runOnJS(onLand)();
    });
  }, [flight, onLand, p]);

  const { from, to } = flight;
  const style = useAnimatedStyle(() => {
    const k = to.size > 0 ? from.size / to.size : 1;
    const scale = interpolate(p.value, [0, 1], [k, 1]);
    const dx = from.x + from.size / 2 - (to.x + to.size / 2);
    const dy = from.y + from.size / 2 - (to.y + to.size / 2);
    return {
      transform: [
        { translateX: interpolate(p.value, [0, 1], [dx, 0]) },
        { translateY: interpolate(p.value, [0, 1], [dy, 0]) },
        { scale },
      ],
      borderRadius: interpolate(p.value, [0, 1], [from.corner, to.corner]) / Math.max(scale, 0.01),
    };
  });

  return (
    <Animated.View
      pointerEvents="none"
      style={[
        styles.flying,
        { left: to.x, top: to.y, width: to.size, height: to.size },
        style,
      ]}
    >
      {/* The square inside draws square corners and the wrapper does the
          clipping, so one animated radius decides the shape. */}
      <Artwork seed={flight.seed} size={to.size} theme={theme} corner={0} glyph={flight.glyph} />
    </Animated.View>
  );
}

/**
 * One end of a shared square: draw the cover, and be ready to be measured.
 *
 * Used exactly as `Artwork` is, with two extra words — which record it is, and
 * which end. Outside a `SharedArtProvider` it is `Artwork` and nothing else,
 * which is what makes it safe to use anywhere.
 */
export function SharedArt({
  id,
  end,
  seed,
  size,
  theme,
  corner = radius.sm,
  round = false,
  glyph = 'note',
}: {
  id: string;
  end: End;
  seed: string;
  size: number;
  theme: Theme;
  corner?: number;
  glyph?: IconName;
  /** An artist is a circle. Its radius travels like any other. */
  round?: boolean;
}) {
  const shared = useContext(Ctx);
  const box = useRef<View>(null);
  const shape = round ? size / 2 : corner;

  const measure = useCallback(
    () =>
      new Promise<ArtRect | null>((answer) => {
        const node = box.current;
        if (!node) return answer(null);
        node.measureInWindow((x, y, w, h) => {
          // A view that is not laid out yet measures as zero, and flying from
          // a zero-sized rectangle is a square erupting out of a point.
          if (!(w > 0 && h > 0)) return answer(null);
          answer({ x, y, size: w, corner: round ? w / 2 : corner });
        });
      }),
    [corner, round],
  );

  useEffect(() => shared?.register(id, end, measure), [shared, id, end, measure]);

  const arrive = useCallback(() => {
    if (end !== 'page') return;
    // One frame after layout: on Android `measureInWindow` inside `onLayout`
    // can answer with where the view was during the transition rather than
    // where it has landed.
    requestAnimationFrame(() => shared?.claim(id, seed, glyph));
  }, [end, glyph, id, seed, shared]);

  const away = shared?.hidden === id;
  return (
    <View ref={box} onLayout={end === 'page' ? arrive : undefined} style={away && styles.away}>
      <Artwork seed={seed} size={size} theme={theme} corner={shape} glyph={glyph} />
    </View>
  );
}

/**
 * What a screen calls on its way out of, or into, a record.
 *
 * Outside a provider both are no-ops, so a caller never has to ask whether
 * there is one.
 */
export function useSharedArt(): {
  lift: (id: string) => void;
  drop: (id: string, seed: string, glyph: IconName) => void;
} {
  const shared = useContext(Ctx);
  return useMemo(
    () => ({
      lift: (id: string) => shared?.lift(id),
      drop: (id: string, seed: string, glyph: IconName) => shared?.drop(id, seed, glyph),
    }),
    [shared],
  );
}

const styles = StyleSheet.create({
  flying: { position: 'absolute', overflow: 'hidden', zIndex: 40 },
  away: { opacity: 0 },
});
