/**
 * The full-screen player.
 *
 * Everything the desktop's bar does — previous, play/pause, next, seek — and
 * beside it the way onto a playlist, because on a phone this is where you are
 * when you decide you like something.
 *
 * It slides up over the list rather than being a route of its own, for a
 * reason that is about the engine and not about navigation: the library screen
 * holds the peer, and a second route holding a second `usePeer` would run the
 * read model twice against one session for as long as both were mounted. A
 * sheet is one screen with two faces.
 *
 * Dragging it down dismisses it. That is not decoration either — it is the
 * gesture every music app has trained people to try first, and a sheet that
 * only closes from a chevron reads as stuck.
 */

import { useMemo } from 'react';
import { Pressable, StyleSheet, Text, View, useWindowDimensions } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import { LinearGradient } from 'expo-linear-gradient';
import Animated, {
  SlideInDown,
  SlideOutDown,
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';

import { clock } from '@/format';
import type { Player } from '@/player';
import { radius, space, type Theme } from '@/theme';
import { Artwork } from './artwork';
import { Icon } from './icon';
import { SeekBar } from './seekbar';

export function NowPlaying({
  player,
  album,
  onAdd,
  onClose,
  theme,
  status,
  top,
  bottom,
  scrub,
  onScrub,
}: {
  player: Player;
  album: string;
  onAdd: () => void;
  onClose: () => void;
  theme: Theme;
  /** The engine, showing through: cursor, pending, the module's generation. */
  status: string;
  top: number;
  bottom: number;
  /** Where the finger is while the bar is being dragged, in seconds. */
  scrub: number | null;
  onScrub: (seconds: number | null) => void;
}) {
  const { width, height } = useWindowDimensions();
  const y = useSharedValue(0);
  const track = player.track;

  const drag = useMemo(
    () =>
      Gesture.Pan()
        .activeOffsetY(12)
        .onUpdate((e) => {
          y.value = Math.max(0, e.translationY);
        })
        .onEnd((e) => {
          if (e.translationY > 140 || e.velocityY > 900) runOnJS(onClose)();
          else y.value = withSpring(0, { damping: 20, stiffness: 220 });
        }),
    [onClose, y],
  );

  const sheet = useAnimatedStyle(() => ({ transform: [{ translateY: y.value }] }));

  if (!track) return null;

  // Big, but not so big that the transport is off the bottom of a small phone.
  const art = Math.min(width - space.xl * 4, height * 0.4);
  const s = styles(theme);
  const shown = scrub ?? player.position;

  return (
    <Animated.View
      entering={SlideInDown.duration(260)}
      exiting={SlideOutDown.duration(200)}
      style={[StyleSheet.absoluteFill, s.sheet, sheet]}
    >
      <LinearGradient
        pointerEvents="none"
        colors={[...theme.glow]}
        locations={[0, 0.45, 1]}
        style={StyleSheet.absoluteFill}
      />

      <GestureDetector gesture={drag}>
        <View style={[s.head, { paddingTop: top + space.sm }]}>
          <Pressable onPress={onClose} hitSlop={12} style={s.headButton} accessibilityLabel="close">
            <Icon name="down" size={24} tint={theme.text} />
          </Pressable>
          <Text style={s.headLabel} numberOfLines={1}>
            {album || 'Now playing'}
          </Text>
          <View style={s.headButton} />
        </View>
      </GestureDetector>

      <View style={s.art}>
        <Artwork seed={album || track.title} size={art} theme={theme} corner={radius.lg} />
      </View>

      <View style={[s.body, { paddingBottom: bottom + space.lg }]}>
        <View style={s.titles}>
          <View style={s.titleText}>
            <Text style={s.title} numberOfLines={2}>
              {track.title}
            </Text>
            <Text style={s.creator} numberOfLines={1}>
              {track.creator || '—'}
            </Text>
          </View>
          <Pressable
            onPress={onAdd}
            hitSlop={12}
            style={s.addTo}
            accessibilityRole="button"
            accessibilityLabel="add to a playlist"
          >
            <Icon name="addTo" size={26} tint={theme.dim} />
          </Pressable>
        </View>

        <SeekRow
          player={player}
          theme={theme}
          shown={shown}
          onScrub={onScrub}
        />

        <View style={s.transport}>
          <Pressable
            onPress={() => player.skip(-1)}
            hitSlop={10}
            style={s.side}
            accessibilityLabel="previous"
          >
            <Icon name="previous" size={30} tint={theme.text} />
          </Pressable>
          <Pressable
            onPress={player.toggle}
            disabled={!track.url}
            style={[s.big, !track.url && s.bigOff]}
            accessibilityLabel={player.playing ? 'pause' : 'play'}
          >
            <Icon
              name={player.playing ? 'pause' : 'play'}
              size={30}
              tint={track.url ? theme.onAccent : theme.faint}
            />
          </Pressable>
          <Pressable
            onPress={() => player.skip(1)}
            hitSlop={10}
            style={s.side}
            accessibilityLabel="next"
          >
            <Icon name="next" size={30} tint={theme.text} />
          </Pressable>
        </View>

        {/* Said plainly rather than by a dead button: a track typed into the
            library with no file is a real thing to have, and it is still
            worth selecting. */}
        {track.url ? null : (
          <Text style={s.trouble}>
            nothing to stream — this one has no file. Add one with a URL, or point the peer at a
            server that has the bytes.
          </Text>
        )}
        {player.error ? <Text style={s.trouble}>{player.error}</Text> : null}

        <Text style={s.status} numberOfLines={2}>
          {status}
        </Text>
      </View>
    </Animated.View>
  );
}

/** The bar and the two clocks under it. Its own component so that the 250ms
 *  status tick redraws four text nodes rather than the whole sheet. */
function SeekRow({
  player,
  theme,
  shown,
  onScrub,
}: {
  player: Player;
  theme: Theme;
  shown: number;
  onScrub: (seconds: number | null) => void;
}) {
  const s = styles(theme);
  return (
    <View>
      <SeekBar
        position={player.position}
        duration={player.duration}
        onSeek={player.seek}
        onScrub={onScrub}
        theme={theme}
        scrubbable={Boolean(player.track?.url)}
      />
      <View style={s.clocks}>
        <Text style={s.clock}>{clock(shown)}</Text>
        <Text style={s.clock}>{clock(player.duration)}</Text>
      </View>
    </View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    sheet: { backgroundColor: t.bg },
    head: {
      flexDirection: 'row',
      alignItems: 'center',
      paddingHorizontal: space.md,
      paddingBottom: space.md,
    },
    headButton: { width: 40, alignItems: 'center' },
    headLabel: {
      flex: 1,
      textAlign: 'center',
      fontSize: 12,
      letterSpacing: 0.6,
      textTransform: 'uppercase',
      color: t.dim,
    },
    art: { flex: 1, alignItems: 'center', justifyContent: 'center', paddingHorizontal: space.xl },
    body: { paddingHorizontal: space.xl, gap: space.sm },
    titles: { flexDirection: 'row', alignItems: 'center', gap: space.md },
    titleText: { flex: 1, gap: 3 },
    title: { fontSize: 24, fontWeight: '700', color: t.text },
    creator: { fontSize: 15, color: t.dim },
    addTo: { padding: space.xs },
    clocks: { flexDirection: 'row', justifyContent: 'space-between', marginTop: -6 },
    clock: { fontSize: 11, color: t.faint, fontVariant: ['tabular-nums'] },
    transport: {
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'center',
      gap: space.xl,
      paddingVertical: space.md,
    },
    side: { padding: space.sm },
    big: {
      width: 68,
      height: 68,
      borderRadius: radius.pill,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.accent,
    },
    bigOff: { backgroundColor: t.cardHigh },
    trouble: { fontSize: 12, lineHeight: 17, color: t.danger },
    status: { fontSize: 11, color: t.faint, textAlign: 'center' },
  });
