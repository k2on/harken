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
 */

import { type ReactNode, useMemo } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, {
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated';

import { space, type Theme } from '@/theme';
import { Icon, type IconName } from './icon';

/** How far it has to go before letting go does anything. */
const THRESHOLD = 88;
const SPRING = { damping: 22, stiffness: 260, mass: 0.6 };

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

  const pan = useMemo(
    () =>
      Gesture.Pan()
        // Horizontal only, and late enough that a scroll never becomes a
        // swipe: a list that swipes while you are flicking down it is a list
        // that does things you did not ask for.
        .activeOffsetX([-20, 20])
        .failOffsetY([-14, 14])
        .onUpdate((e) => {
          x.value = e.translationX;
        })
        .onEnd((e) => {
          if (e.translationX > THRESHOLD) runOnJS(onLeft)();
          else if (e.translationX < -THRESHOLD) runOnJS(onRight)();
          // Back either way. What it did, it did — there is nothing left open
          // to close.
          x.value = withSpring(0, SPRING);
        }),
    [onLeft, onRight, x],
  );

  const row = useAnimatedStyle(() => ({ transform: [{ translateX: x.value }] }));
  // Each panel is only drawn on its own side, so the label under the row is
  // always the one that would happen.
  const left = useAnimatedStyle(() => ({
    opacity: withTiming(x.value > 24 ? 1 : 0, { duration: 90 }),
  }));
  const right = useAnimatedStyle(() => ({
    opacity: withTiming(x.value < -24 ? 1 : 0, { duration: 90 }),
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

const styles = (t: Theme) =>
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
    label: { fontSize: 13, fontWeight: '600', color: t.dim },
  });
