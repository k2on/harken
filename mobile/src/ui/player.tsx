/**
 * The player, which is one thing with two faces.
 *
 * A bar above the tab bar, and the full-screen sheet it grows into. They used
 * to be two components with two animations — a bar that faded and a sheet that
 * slid in over it — and the seam showed: what you dragged up was not what
 * appeared, and closing played a different animation from opening.
 *
 * So there is one container, full-screen tall, translated by one shared value
 * in `[0, 1]`. At 0 its top edge sits exactly where the bar belongs and the
 * sheet is off the bottom of the screen; at 1 it covers everything. The bar is
 * pinned to the container's top and fades out as the sheet fades in, so the
 * thing under your finger is the thing that arrives — and the same value, run
 * backwards, is the close.
 *
 * **Two gestures, split by axis**, which is what keeps them from fighting:
 *
 * - vertical on the container opens and closes, from anywhere, including
 *   anywhere on the expanded sheet. It fails on a horizontal drag, which is
 *   what leaves the seek bar's own gesture alone;
 * - horizontal on the *bar* skips, because a mini bar is the one control you
 *   reach for without looking and a thumb's flick is the gesture people
 *   already try.
 *
 * Both track the finger rather than firing on release: a sheet that only moves
 * after you let go is a sheet you are not sure you are dragging.
 */

import { memo, useCallback, useEffect, useMemo, useState } from 'react';
import { BackHandler, Pressable, StyleSheet, Text, View, useWindowDimensions } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import { LinearGradient } from 'expo-linear-gradient';
import Animated, {
  interpolate,
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';

import { clock } from '@/format';
import type { Player } from '@/player';
import { FONT, radius, space, type Theme } from '@/theme';
import { Artwork } from './artwork';
import { Icon } from './icon';
import { SeekBar } from './seekbar';
import { TAB_BAR } from './tabbar';

/** How tall the collapsed bar is. Everything that has to agree about where it
 *  sits is laid out before it is drawn, so this is a constant. */
export const BAR = 58;

const SPRING = { damping: 22, stiffness: 240, mass: 0.7 };

export function PlayerSheet({
  player,
  album,
  theme,
  top,
  bottom,
  onAdd,
  onDevices,
}: {
  player: Player;
  album: string;
  theme: Theme;
  top: number;
  bottom: number;
  /** Put the track on a playlist. */
  onAdd: () => void;
  /** Open the device sheet. */
  onDevices: () => void;
}) {
  const { width, height } = useWindowDimensions();
  const open = useSharedValue(0);
  const from = useSharedValue(0);
  // Where it has settled, which the animation cannot answer: React needs it to
  // decide whether the sheet takes touches at all. An `opacity: 0` view still
  // does, so without this the collapsed sheet — which hangs over the tab bar
  // on its way off the bottom of the screen — would quietly eat every tap on
  // Your Library.
  const [expanded, setExpanded] = useState(false);
  const track = player.track;

  // How far the container travels between the two faces: the bar's resting
  // top edge, measured from the top of the screen.
  const travel = Math.max(1, height - bottom - TAB_BAR - BAR);

  const settle = useCallback((to: number) => {
    setExpanded(to === 1);
  }, []);

  // Built twice from one description rather than shared, because a `Gesture`
  // belongs to the detector it is given to — and this one has to be on the
  // sheet, to close it from anywhere on it, *and* on the bar, to open it.
  const makeLift = useCallback(
    () =>
      Gesture.Pan()
        .activeOffsetY([-12, 12])
        // A horizontal drag is the bar's or the seek bar's, never this one.
        .failOffsetX([-28, 28])
        .onStart(() => {
          from.value = open.value;
        })
        .onUpdate((e) => {
          open.value = Math.min(1, Math.max(0, from.value - e.translationY / travel));
        })
        .onEnd((e) => {
          const up = e.velocityY < -650;
          const down = e.velocityY > 650;
          // A flick decides on its own; anything slower is decided by where it
          // was let go, so a drag halfway and back does not open.
          const to = up ? 1 : down ? 0 : open.value > 0.4 ? 1 : 0;
          open.value = withSpring(to, SPRING);
          runOnJS(settle)(to);
        }),
    [from, open, settle, travel],
  );

  const liftSheet = useMemo(makeLift, [makeLift]);
  const liftBar = useMemo(makeLift, [makeLift]);

  const { skip } = player;
  const flick = useMemo(
    () =>
      Gesture.Pan()
        .activeOffsetX([-24, 24])
        .failOffsetY([-18, 18])
        .onEnd((e) => {
          if (Math.abs(e.translationX) < 60 && Math.abs(e.velocityX) < 700) return;
          runOnJS(skip)(e.translationX < 0 ? 1 : -1);
        }),
    [skip],
  );

  const tap = useMemo(
    () =>
      Gesture.Tap().onEnd(() => {
        open.value = withSpring(1, SPRING);
        runOnJS(settle)(1);
      }),
    [open, settle],
  );

  const close = useCallback(() => {
    open.value = withSpring(0, SPRING);
    setExpanded(false);
  }, [open]);

  // Android's back button closes it, because a full-screen thing that ignores
  // back is a full-screen thing people think has hung.
  useEffect(() => {
    if (!expanded) return;
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      close();
      return true;
    });
    return () => sub.remove();
  }, [close, expanded]);

  const shell = useAnimatedStyle(() => ({
    transform: [{ translateY: (1 - open.value) * travel }],
  }));
  // The two faces cross over in the first third, so the sheet is legible well
  // before it has arrived and the bar is gone before it would overlap the
  // sheet's own header.
  const barFace = useAnimatedStyle(() => ({
    opacity: interpolate(open.value, [0, 0.35], [1, 0]),
  }));
  const sheetFace = useAnimatedStyle(() => ({
    opacity: interpolate(open.value, [0.1, 0.55], [0, 1]),
  }));

  if (!track) return null;

  const s = styles(theme);
  return (
    <Animated.View style={[s.shell, { height }, shell]} pointerEvents="box-none">
      {/* The sheet, filling the container. While the bar is showing it is off
          the bottom of the screen *except* for the strip that hangs over the
          tab bar — so it gives its touches up rather than relying on being
          invisible, which an `opacity: 0` view does not do. */}
      <GestureDetector gesture={liftSheet}>
        <Animated.View style={[s.sheet, sheetFace]} pointerEvents={expanded ? 'auto' : 'none'}>
          <Expanded
            player={player}
            album={album}
            theme={theme}
            top={top}
            bottom={bottom}
            width={width}
            height={height}
            onAdd={onAdd}
            onDevices={onDevices}
            onClose={close}
          />
        </Animated.View>
      </GestureDetector>

      {/* …and the bar, pinned to the container's top edge, which is exactly
          where it belongs when the container is down. */}
      <Animated.View style={[s.barWrap, barFace]} pointerEvents={expanded ? 'none' : 'auto'}>
        <GestureDetector gesture={Gesture.Race(liftBar, flick, tap)}>
          <View>
            <MiniBar player={player} album={album} theme={theme} onDevices={onDevices} />
          </View>
        </GestureDetector>
      </Animated.View>
    </Animated.View>
  );
}

/**
 * The bar: what is playing, where it is playing, and play/pause.
 *
 * Three things and no more. This is one thumb's width from the bottom of the
 * screen and is the only control that has to be reachable from anywhere, so
 * everything else is a swipe or a tap away — next and previous are the flick,
 * and the rest is the sheet.
 */
const MiniBar = memo(function MiniBar({
  player,
  album,
  theme,
  onDevices,
}: {
  player: Player;
  album: string;
  theme: Theme;
  onDevices: () => void;
}) {
  const track = player.track;
  const s = styles(theme);
  if (!track) return null;
  const pct = player.duration > 0 ? Math.min(100, (player.position / player.duration) * 100) : 0;
  return (
    <View style={s.bar}>
      <View style={s.line}>
        <View style={[s.lineFill, { width: `${pct}%` }]} />
      </View>
      <View style={s.barRow}>
        <Artwork seed={album || track.title} size={40} theme={theme} />
        <View style={s.text}>
          <Text style={s.title} numberOfLines={1}>
            {track.title}
          </Text>
          {/* Where it is playing takes the second line when it is not here,
              because that is the more surprising fact: a phone that is silent
              with a full bar is a phone somebody thinks is broken. */}
          {player.elsewhere ? (
            <Playing on={player.output?.name ?? 'another device'} theme={theme} small />
          ) : (
            <Text style={s.meta} numberOfLines={1}>
              {player.buffering ? 'buffering…' : track.url ? track.creator : 'nothing to stream'}
            </Text>
          )}
        </View>
        {player.devices.length > 0 ? (
          <Pressable
            hitSlop={10}
            onPress={onDevices}
            style={s.button}
            accessibilityRole="button"
            accessibilityLabel="which device is playing"
          >
            <Icon name="devices" size={20} tint={player.elsewhere ? theme.accent : theme.dim} />
          </Pressable>
        ) : null}
        <Pressable
          hitSlop={10}
          onPress={player.toggle}
          style={s.button}
          accessibilityRole="button"
          accessibilityLabel={player.playing ? 'pause' : 'play'}
        >
          <Icon name={player.playing ? 'pause' : 'play'} size={24} tint={theme.text} />
        </Pressable>
      </View>
    </View>
  );
});

/**
 * Where the sound is coming from, in the accent, with a speaker beside it.
 *
 * The one line in this app that is drawn in a colour because of *what it says*
 * rather than what it is: a device name is ordinary text until it is somewhere
 * other than the phone in your hand, and then it is the most important thing
 * on the screen.
 */
function Playing({ on, theme, small }: { on: string; theme: Theme; small?: boolean }) {
  const s = styles(theme);
  return (
    <View style={s.playingOn}>
      <Icon name="devices" size={small ? 12 : 15} tint={theme.accent} />
      <Text style={[s.playingText, small && s.playingSmall]} numberOfLines={1}>
        {on}
      </Text>
    </View>
  );
}

/** The full-screen face. */
function Expanded({
  player,
  album,
  theme,
  top,
  bottom,
  width,
  height,
  onAdd,
  onDevices,
  onClose,
}: {
  player: Player;
  album: string;
  theme: Theme;
  top: number;
  bottom: number;
  width: number;
  height: number;
  onAdd: () => void;
  onDevices: () => void;
  onClose: () => void;
}) {
  const track = player.track;
  const s = styles(theme);
  if (!track) return null;
  // Big, but not so big that the transport is off the bottom of a small phone.
  const art = Math.min(width - space.xl * 4, height * 0.38);
  return (
    <View style={s.expanded}>
      <LinearGradient
        pointerEvents="none"
        colors={[...theme.glow]}
        locations={[0, 0.45, 1]}
        style={StyleSheet.absoluteFill}
      />

      <View style={[s.head, { paddingTop: top + space.sm }]}>
        <Pressable onPress={onClose} hitSlop={12} style={s.headButton} accessibilityLabel="close">
          <Icon name="down" size={24} tint={theme.text} />
        </Pressable>
        <Text style={s.headLabel} numberOfLines={1}>
          {album || 'Now playing'}
        </Text>
        <View style={s.headButton} />
      </View>

      <View style={s.art}>
        <Artwork seed={album || track.title} size={art} theme={theme} corner={radius.lg} />
      </View>

      <View style={[s.body, { paddingBottom: bottom + space.lg }]}>
        <View style={s.titles}>
          <View style={s.titleText}>
            <Text style={s.bigTitle} numberOfLines={2}>
              {track.title}
            </Text>
            <Text style={s.creator} numberOfLines={1}>
              {track.creator || '—'}
            </Text>
          </View>
          <Pressable
            onPress={onAdd}
            hitSlop={12}
            accessibilityRole="button"
            accessibilityLabel="add to a playlist"
          >
            <Icon name="addTo" size={26} tint={theme.dim} />
          </Pressable>
        </View>

        <SeekBar
          position={player.position}
          duration={player.duration}
          onSeek={player.seek}
          theme={theme}
          scrubbable={Boolean(track.url) || player.elsewhere}
        />
        <View style={s.clocks}>
          <Text style={s.clock}>{clock(player.position)}</Text>
          <Text style={s.clock}>{clock(player.duration)}</Text>
        </View>

        <View style={s.transport}>
          <Pressable onPress={() => player.skip(-1)} hitSlop={10} accessibilityLabel="previous">
            <Icon name="previous" size={30} tint={theme.text} />
          </Pressable>
          <Pressable
            onPress={player.toggle}
            style={s.big}
            accessibilityLabel={player.playing ? 'pause' : 'play'}
          >
            <Icon name={player.playing ? 'pause' : 'play'} size={30} tint={theme.onAccent} />
          </Pressable>
          <Pressable onPress={() => player.skip(1)} hitSlop={10} accessibilityLabel="next">
            <Icon name="next" size={30} tint={theme.text} />
          </Pressable>
        </View>

        {/* Where it is playing, under the transport — the place Spotify put it
            and the place people look, because it is the answer to a question
            asked with a thumb already on the buttons. */}
        {player.devices.length > 0 ? (
          <Pressable
            onPress={onDevices}
            style={({ pressed }) => [s.devices, pressed && s.devicesPressed]}
            accessibilityRole="button"
            accessibilityLabel="which device is playing"
          >
            {player.elsewhere ? (
              <Playing on={player.output?.name ?? 'another device'} theme={theme} />
            ) : (
              <>
                <Icon name="devices" size={15} tint={theme.dim} />
                <Text style={s.devicesText}>
                  {player.outputsHere ? 'Playing on this phone' : 'Choose a device'}
                </Text>
              </>
            )}
          </Pressable>
        ) : null}

        {track.url ? null : (
          <Text style={s.trouble}>
            nothing to stream — this one has no file. Point the peer at a server that has the
            bytes.
          </Text>
        )}
        {player.error ? <Text style={s.trouble}>{player.error}</Text> : null}
      </View>
    </View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    shell: { position: 'absolute', left: 0, right: 0, top: 0 },
    // The four edges written out: these typings expose `absoluteFill` as a
    // registered style rather than an object, so it cannot be spread into one.
    sheet: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0 },
    expanded: { flex: 1, backgroundColor: t.bg },
    barWrap: { position: 'absolute', top: 0, left: 0, right: 0 },
    bar: {
      backgroundColor: t.raised,
      borderTopWidth: StyleSheet.hairlineWidth,
      borderTopColor: t.border,
      height: BAR,
    },
    // A hairline of progress along the top edge. Enough to say the thing is
    // moving without becoming a second control.
    line: { height: 2, backgroundColor: t.border },
    lineFill: { height: 2, backgroundColor: t.accent },
    barRow: {
      flex: 1,
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.md,
    },
    text: { flex: 1, gap: 1 },
    title: { fontFamily: FONT, fontSize: 14, fontWeight: '600', color: t.text },
    meta: { fontFamily: FONT, fontSize: 12, color: t.dim },
    button: { padding: space.xs },
    playingOn: { flexDirection: 'row', alignItems: 'center', gap: 5 },
    playingText: { fontFamily: FONT, fontSize: 12.5, color: t.accent, fontWeight: '600', flexShrink: 1 },
    playingSmall: { fontFamily: FONT, fontSize: 11.5 },

    head: {
      flexDirection: 'row',
      alignItems: 'center',
      paddingHorizontal: space.lg,
      paddingBottom: space.sm,
    },
    headButton: { width: 34, alignItems: 'flex-start' },
    headLabel: { flex: 1, textAlign: 'center', fontFamily: FONT, fontSize: 12.5, color: t.dim },
    art: { flex: 1, alignItems: 'center', justifyContent: 'center' },
    body: { paddingHorizontal: space.xl, gap: space.md },
    titles: { flexDirection: 'row', alignItems: 'center', gap: space.md },
    titleText: { flex: 1, gap: 2 },
    bigTitle: { fontFamily: FONT, fontSize: 22, fontWeight: '800', color: t.text },
    creator: { fontFamily: FONT, fontSize: 14, color: t.dim },
    clocks: { flexDirection: 'row', justifyContent: 'space-between' },
    clock: { fontFamily: FONT, fontSize: 11, color: t.faint, fontVariant: ['tabular-nums'] },
    transport: {
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'center',
      gap: space.xl,
      paddingVertical: space.sm,
    },
    big: {
      width: 62,
      height: 62,
      borderRadius: radius.pill,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.accent,
    },
    devices: {
      flexDirection: 'row',
      alignItems: 'center',
      alignSelf: 'center',
      gap: space.sm,
      paddingHorizontal: space.md,
      paddingVertical: space.sm,
      borderRadius: radius.pill,
    },
    devicesPressed: { backgroundColor: t.cardHigh },
    devicesText: { fontFamily: FONT, fontSize: 12.5, color: t.dim },
    trouble: { fontFamily: FONT, fontSize: 12, lineHeight: 17, color: t.danger, textAlign: 'center' },
  });
