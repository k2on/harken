/**
 * A row you can push aside to do one of two things to it.
 *
 * The gesture every list on a phone has: drag left or right, a coloured panel
 * appears behind with what would happen written on it, let go past the
 * threshold and it happens. Under the threshold it springs back, which is the
 * part that makes it safe to try.
 *
 * **It snaps back rather than staying open.** A row that stays open is a row
 * with a second state to close, and the two actions here are both immediate —
 * there is nothing to reveal and then tap. So the gesture *is* the button.
 *
 * The right-hand action is deliberately allowed to be one that does not exist
 * yet. A swipe with nothing behind it is not a gesture people discover twice,
 * and saying "not implemented" out loud is a better answer than a row that
 * springs back for no visible reason — see `onOther`.
 *
 * **It is rubber-banded, and it has a wall.** The finger used to move the row
 * one pixel for one pixel, for ever — so a drag across the screen took the row
 * with it and left a list of blank stripes with a label stranded at the far
 * edge, and nothing on screen said the gesture had already done everything it
 * was going to. Past `THRESHOLD` the row follows at an exponentially shrinking
 * rate and asymptotes at `LIMIT`: it stiffens exactly where letting go starts
 * to mean something, which is what tells a thumb "that is enough" without a
 * word, and it can never be dragged past the panel that is explaining it.
 * `rubber` is where that curve lives, because the play bar wants the same one.
 */

import { type ReactNode, useCallback, useMemo, useRef } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, {
  interpolate,
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';

import { FONT, space, type Theme, sheet } from '@/theme';
import { Icon, type IconName } from './icon';
import { rubber } from './rubber';

/** How far it has to go before letting go does anything. Up to here the row
 *  tracks the finger exactly, because a gesture that lies about where your
 *  thumb is before it has committed to anything is a gesture that feels
 *  broken. */
const THRESHOLD = 88;
/** …and how far it can go at all. Approached, never reached: the resistance
 *  below is asymptotic, so there is no frame where the row stops dead. */
const LIMIT = 132;
const SPRING = { damping: 20, stiffness: 300, mass: 0.55 };

export function Swipe({
  children,
  theme,
  onLeft,
  onRight,
  leftLabel,
  rightLabel,
  leftIcon,
  rightIcon,
  leftTint,
}: {
  children: ReactNode;
  theme: Theme;
  /** Dragging the row *right*, which reveals the panel on the left. */
  onLeft: () => void;
  onRight: () => void;
  leftLabel: string;
  rightLabel: string;
  leftIcon: IconName;
  rightIcon: IconName;
  leftTint?: string;
}) {
  const x = useSharedValue(0);
  const s = styles(theme);

  // The two callbacks are new objects on every render of the list that draws
  // this — `onLeft={() => onAdd(item)}` is a fresh closure per row per frame —
  // so a gesture built from them is rebuilt per row per frame too, and
  // rebuilding a gesture means tearing down and re-attaching a native
  // recogniser. Held in a ref and reached through one stable function, the
  // gesture below is built once for the life of the row.
  const acts = useRef({ onLeft, onRight });
  acts.current = { onLeft, onRight };
  const fire = useCallback((dir: 1 | -1) => {
    if (dir === 1) acts.current.onLeft();
    else acts.current.onRight();
  }, []);

  const pan = useMemo(
    () =>
      Gesture.Pan()
        // Horizontal only, and late enough that a scroll never becomes a
        // swipe: a list that swipes while you are flicking down it is a list
        // that does things you did not ask for.
        .activeOffsetX([-20, 20])
        .failOffsetY([-14, 14])
        .onUpdate((e) => {
          x.value = rubber(e.translationX, THRESHOLD, LIMIT);
        })
        .onEnd((e) => {
          // Against the *finger*, not against the resisted row: past the wall
          // the row barely moves, so asking where the row got to would make
          // the threshold impossible to cross by pushing harder.
          if (e.translationX > THRESHOLD) runOnJS(fire)(1);
          else if (e.translationX < -THRESHOLD) runOnJS(fire)(-1);
          // Back either way. What it did, it did — there is nothing left open
          // to close.
          x.value = withSpring(0, SPRING);
        })
        // A gesture the system takes away mid-drag — a back swipe winning, a
        // call arriving — never reaches `onEnd`, and the row would stay parked
        // where the finger left it. `success` is false exactly then, which is
        // what keeps this from re-starting the spring `onEnd` just began.
        .onFinalize((_e, success) => {
          if (!success) x.value = withSpring(0, SPRING);
        }),
    [fire, x],
  );

  const row = useAnimatedStyle(() => ({ transform: [{ translateX: x.value }] }));
  // Each panel is only drawn on its own side, so the label under the row is
  // always the one that would happen — and it *fades in with the drag* rather
  // than being switched on by a `withTiming` started from inside a style. That
  // shape starts a fresh animation on every frame the style is evaluated,
  // which on a list is one per visible row per frame, and it is the single
  // cheapest thing to get wrong in Reanimated.
  const left = useAnimatedStyle(() => ({
    opacity: interpolate(x.value, [10, THRESHOLD * 0.7], [0, 1], 'clamp'),
  }));
  const right = useAnimatedStyle(() => ({
    opacity: interpolate(x.value, [-THRESHOLD * 0.7, -10], [1, 0], 'clamp'),
  }));

  return (
    <View style={s.wrap}>
      <Animated.View style={[s.behind, s.behindLeft, left]} pointerEvents="none">
        <Icon name={leftIcon} size={19} tint={leftTint ?? theme.accent} />
        <Text style={[s.label, { color: leftTint ?? theme.accent }]}>{leftLabel}</Text>
      </Animated.View>
      <Animated.View style={[s.behind, s.behindRight, right]} pointerEvents="none">
        <Text style={s.label}>{rightLabel}</Text>
        <Icon name={rightIcon} size={19} tint={theme.dim} />
      </Animated.View>
      <GestureDetector gesture={pan}>
        <Animated.View style={[s.row, row]}>{children}</Animated.View>
      </GestureDetector>
    </View>
  );
}

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    wrap: { backgroundColor: t.bg },
    row: { backgroundColor: t.bg },
    behind: {
      position: 'absolute',
      top: 0,
      bottom: 0,
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.sm,
      paddingHorizontal: space.lg,
    },
    behindLeft: { left: 0 },
    behindRight: { right: 0 },
    label: { fontFamily: FONT, fontSize: 13, fontWeight: '600', color: t.dim },
  }),
);
