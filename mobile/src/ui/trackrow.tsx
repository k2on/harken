/**
 * One line of the list.
 *
 * The desktop draws a table — Name, Artist, Album, Time under headings — and a
 * phone is too narrow for four columns, so the same four facts are stacked:
 * the title on one line, the creator and the album on the next, the length at
 * the end. The rules that made the table work carry over unchanged:
 *
 * - **The whole line is the target.** The background belongs to the row, not
 *   to a button wrapped around the title. A stripe that stops where the text
 *   does is not a row.
 * - **The one playing is the only thing drawn in the accent colour**, so it is
 *   findable at a glance in a list of twenty near-identical rows.
 * - **Nothing is a literal colour.** Every one is asked of the theme.
 */

import { memo } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withSequence, withTiming } from 'react-native-reanimated';

import { clockMs } from '@/format';
import { radius, space, type Theme } from '@/theme';
import type { Item } from 'harken-native';
import { Artwork } from './artwork';
import { Icon } from './icon';

/** The heart, with a pop.
 *
 *  Worth the six lines: a tap that changes a colour and nothing else reads as
 *  a tap that might not have registered, and this is the one control in the
 *  app whose effect is invisible until the server agrees. */
function Heart({ on, onPress, theme }: { on: boolean; onPress: () => void; theme: Theme }) {
  const pop = useSharedValue(1);
  const style = useAnimatedStyle(() => ({ transform: [{ scale: pop.value }] }));
  return (
    <Pressable
      hitSlop={12}
      accessibilityRole="button"
      accessibilityLabel={on ? 'remove from the playlist' : 'add to the playlist'}
      onPress={() => {
        pop.value = withSequence(
          withTiming(1.32, { duration: 110 }),
          withTiming(1, { duration: 160 }),
        );
        onPress();
      }}
      style={{ padding: space.sm }}
    >
      <Animated.View style={style}>
        <Icon name={on ? 'heartFilled' : 'heart'} size={19} tint={on ? theme.accent : theme.faint} />
      </Animated.View>
    </Pressable>
  );
}

export type TrackRowProps = {
  item: Item;
  album: string;
  /** The one this row is, if it is the one playing. */
  playing: boolean;
  theme: Theme;
  // Each takes the row's own item, so the screen can hold one callback for
  // the whole list instead of making three per row per render — which is what
  // makes the memo below worth having at all.
  onPress: (item: Item) => void;
  onHeart: (item: Item) => void;
};

function Row({ item, album, playing, theme, onPress, onHeart }: TrackRowProps) {
  const s = styles(theme);
  return (
    <Pressable
      onPress={() => onPress(item)}
      style={({ pressed }) => [s.row, pressed && s.pressed]}
    >
      <View>
        <Artwork seed={album || item.title} size={46} theme={theme} />
        {playing ? (
          <View style={s.badge}>
            <Icon name="playing" size={13} tint={theme.onAccent} />
          </View>
        ) : null}
      </View>

      <View style={s.text}>
        <Text style={[s.title, playing && s.titlePlaying]} numberOfLines={1}>
          {item.title}
        </Text>
        <Text style={s.meta} numberOfLines={1}>
          {[item.creator, album].filter(Boolean).join(' · ') || '—'}
        </Text>
      </View>

      {/* Blank rather than `0:00` for a length nothing knows yet: an unknown
          duration should read as absent, not as a track of no length. */}
      <Text style={s.time}>{item.durationMs > 0n ? clockMs(item.durationMs) : ''}</Text>
      <Heart on={item.onPlaylist} onPress={() => onHeart(item)} theme={theme} />
    </Pressable>
  );
}

/** Memoised on what is drawn. A list of a thousand re-renders once per change
 *  otherwise, and the whole reason the library is a maintained view is that a
 *  change should cost the rows that moved. */
export const TrackRow = memo(
  Row,
  (a, b) =>
    a.item === b.item &&
    a.album === b.album &&
    a.playing === b.playing &&
    a.theme === b.theme &&
    a.onPress === b.onPress &&
    a.onHeart === b.onHeart,
);

const styles = (t: Theme) =>
  StyleSheet.create({
    row: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.lg,
      paddingVertical: space.sm,
      backgroundColor: 'transparent',
    },
    pressed: { backgroundColor: t.cardHigh },
    badge: {
      position: 'absolute',
      right: -4,
      bottom: -4,
      width: 20,
      height: 20,
      borderRadius: radius.pill,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.accent,
    },
    text: { flex: 1, gap: 2 },
    title: { fontSize: 15.5, fontWeight: '600', color: t.text },
    titlePlaying: { color: t.accent },
    meta: { fontSize: 12.5, color: t.dim },
    time: { fontSize: 12, color: t.faint, fontVariant: ['tabular-nums'] },
  });
