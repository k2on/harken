/**
 * The progress bar, and the only thing on the screen you can drag.
 *
 * Seeking is the platform's job — `expo-audio` asks ExoPlayer or AVPlayer to
 * move, exactly as the desktop's slider asks an `<audio>` element — so what is
 * left here is the gesture and the drawing, and both are worth doing properly.
 *
 * Two rules make it feel right rather than merely work:
 *
 * - **The bar follows the finger, not the player.** While a drag is in flight
 *   the fill is the finger's position; the player is told once, on release.
 *   Wired the other way round, a drag fights the 250ms status tick — the fill
 *   snaps back to where playback still is, twice a second, and reads as a bar
 *   that will not be moved.
 * - **It advances on the UI thread.** The fill is a Reanimated shared value
 *   eased between ticks, so a dropped JavaScript frame — a list re-rendering
 *   because a mutation arrived, which is a thing that happens here — does not
 *   show up as a stutter in the one element that is always moving.
 */

import { useCallback, useEffect, useMemo, useRef } from 'react';
import { View } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, {
  Easing,
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from 'react-native-reanimated';

import { radius, type Theme } from '@/theme';

export function SeekBar({
  position,
  duration,
  onSeek,
  onScrub,
  theme,
  /** False for a track with nothing to stream: there is no position to move
   *  to, so the bar is drawn and not dragged. */
  scrubbable,
  height = 4,
}: {
  position: number;
  duration: number;
  onSeek: (seconds: number) => void;
  onScrub?: (seconds: number | null) => void;
  theme: Theme;
  scrubbable: boolean;
  height?: number;
}) {
  const width = useSharedValue(0);
  const frac = useSharedValue(0);
  const scrubbing = useSharedValue(false);
  const thumb = useSharedValue(0);

  // The player's own figures, for the two callbacks that turn a fraction back
  // into seconds. In a ref because the gesture is built once.
  const span = useRef(duration);
  span.current = duration;

  useEffect(() => {
    if (scrubbing.value) return;
    const target = duration > 0 ? Math.min(1, Math.max(0, position / duration)) : 0;
    // Linear, and just longer than the 250ms status interval: the fill should
    // arrive at the next tick's value exactly as that tick lands.
    frac.value = withTiming(target, { duration: 260, easing: Easing.linear });
  }, [position, duration, frac, scrubbing]);

  const commit = useCallback(
    (f: number) => {
      onSeek(f * span.current);
      onScrub?.(null);
    },
    [onSeek, onScrub],
  );

  const report = useCallback(
    (f: number) => {
      onScrub?.(f * span.current);
    },
    [onScrub],
  );

  // Built once per configuration rather than per render: the status ticks four
  // times a second and every one of them would otherwise hand
  // `GestureDetector` a new gesture to reconcile.
  const pan = useMemo(
    () =>
      Gesture.Pan()
        .minDistance(0)
        .enabled(scrubbable)
        .onBegin((e) => {
          scrubbing.value = true;
          thumb.value = withTiming(1, { duration: 120 });
          frac.value = width.value > 0 ? Math.min(1, Math.max(0, e.x / width.value)) : 0;
          runOnJS(report)(frac.value);
        })
        .onUpdate((e) => {
          frac.value = width.value > 0 ? Math.min(1, Math.max(0, e.x / width.value)) : 0;
          runOnJS(report)(frac.value);
        })
        .onEnd(() => {
          runOnJS(commit)(frac.value);
        })
        .onFinalize(() => {
          scrubbing.value = false;
          thumb.value = withTiming(0, { duration: 160 });
        }),
    [scrubbable, commit, report, frac, scrubbing, thumb, width],
  );

  const fill = useAnimatedStyle(() => ({ width: width.value * frac.value }));
  const knob = useAnimatedStyle(() => ({
    transform: [
      { translateX: width.value * frac.value },
      { scale: 0.7 + thumb.value * 0.5 },
    ],
    opacity: 0.85 + thumb.value * 0.15,
  }));

  return (
    <GestureDetector gesture={pan}>
      {/* Taller than the bar it draws, so the target is a thumb rather than a
          hairline. The padding is the hit area and the bar is the picture. */}
      <View
        style={{ paddingVertical: 14, justifyContent: 'center' }}
        onLayout={(e) => {
          width.value = e.nativeEvent.layout.width;
        }}
      >
        <View
          style={{
            height,
            borderRadius: radius.pill,
            backgroundColor: theme.dark ? theme.cardHigh : theme.border,
            overflow: 'hidden',
          }}
        >
          <Animated.View
            style={[
              { height, borderRadius: radius.pill, backgroundColor: theme.accent },
              fill,
            ]}
          />
        </View>
        <Animated.View
          pointerEvents="none"
          style={[
            {
              position: 'absolute',
              left: -7,
              width: 14,
              height: 14,
              borderRadius: 7,
              backgroundColor: theme.accent,
            },
            knob,
          ]}
        />
      </View>
    </GestureDetector>
  );
}
