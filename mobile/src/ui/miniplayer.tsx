/**
 * The bar that is always there once something has been played.
 *
 * The desktop's now-playing bar is deliberately not a pane the cursor can be
 * in: everything it does has a key of its own. The same reasoning holds here
 * for a different reason — this is one thumb's width from the bottom of the
 * screen and is the only control that must be reachable from anywhere, so it
 * carries exactly three things: what is playing, play/pause, and next. The
 * rest is a tap away, on the sheet this opens.
 */

import { Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeInDown, FadeOutDown } from 'react-native-reanimated';

import type { Player } from '@/player';
import { radius, space, type Theme } from '@/theme';
import { Artwork } from './artwork';
import { Icon } from './icon';

export function MiniPlayer({
  player,
  album,
  theme,
  onOpen,
  bottom,
}: {
  player: Player;
  album: string;
  theme: Theme;
  onOpen: () => void;
  /** The safe-area inset to sit above. */
  bottom: number;
}) {
  const track = player.track;
  if (!track) return null;

  const s = styles(theme);
  const pct =
    player.duration > 0 ? Math.min(100, (player.position / player.duration) * 100) : 0;

  return (
    <Animated.View
      entering={FadeInDown.duration(220)}
      exiting={FadeOutDown.duration(180)}
      style={[s.wrap, { paddingBottom: bottom }]}
    >
      <View style={s.line}>
        <View style={[s.lineFill, { width: `${pct}%` }]} />
      </View>
      <Pressable onPress={onOpen} style={({ pressed }) => [s.bar, pressed && s.pressed]}>
        <Artwork seed={album || track.title} size={40} theme={theme} />
        <View style={s.text}>
          <Text style={s.title} numberOfLines={1}>
            {track.title}
          </Text>
          <Text style={s.meta} numberOfLines={1}>
            {player.buffering ? 'buffering…' : track.url ? track.creator : 'nothing to stream'}
          </Text>
        </View>
        {/* One glyph, and only when the sound is not here. This bar has room
            for exactly three things and this is not a fourth control — it is
            the answer to "why is this phone silent", which without it is a
            mute button somebody has to go looking for. The sheet this opens
            is where the list lives; this only says there is one. */}
        {player.elsewhere ? (
          <View
            style={s.away}
            accessibilityLabel={`playing on ${player.output?.name ?? 'another device'}`}
          >
            <Icon name="devices" size={16} tint={theme.accent} />
          </View>
        ) : null}
        <Pressable
          hitSlop={10}
          onPress={player.toggle}
          disabled={!track.url}
          style={s.button}
          accessibilityRole="button"
          accessibilityLabel={player.playing ? 'pause' : 'play'}
        >
          <Icon
            name={player.playing ? 'pause' : 'play'}
            size={22}
            tint={track.url ? theme.text : theme.faint}
          />
        </Pressable>
        <Pressable
          hitSlop={10}
          onPress={() => player.skip(1)}
          style={s.button}
          accessibilityRole="button"
          accessibilityLabel="next"
        >
          <Icon name="next" size={20} tint={theme.dim} />
        </Pressable>
      </Pressable>
    </Animated.View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    wrap: {
      backgroundColor: t.raised,
      borderTopWidth: StyleSheet.hairlineWidth,
      borderTopColor: t.border,
    },
    // A hairline of progress along the top edge. Enough to say the thing is
    // moving without becoming a second control.
    line: { height: 2, backgroundColor: t.border },
    lineFill: { height: 2, backgroundColor: t.accent },
    bar: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.md,
      paddingVertical: space.sm,
    },
    pressed: { backgroundColor: t.cardHigh },
    text: { flex: 1, gap: 1 },
    away: { paddingHorizontal: space.xs },
    title: { fontSize: 14, fontWeight: '600', color: t.text },
    meta: { fontSize: 12, color: t.dim },
    button: { padding: space.sm, borderRadius: radius.pill },
  });
