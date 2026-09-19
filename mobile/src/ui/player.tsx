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
 * **The collapsed bar is a card, not a shelf.** It used to be a full-bleed
 * strip with a hairline along its top, which is the one arrangement that makes
 * a bar look like part of the tab bar rather than like the thing that is
 * playing: two full-width bands stacked, sharing an edge, reading as one lump
 * of chrome. It is inset from both sides and rounded now, with a gap under it,
 * so it floats over the list — and `PlayerScrim` in the shell fades the page
 * out underneath it rather than cutting it off with a rule. A card has to be
 * *over* something for that to read, which is what the fade is for.
 *
 * **Three gestures on it, split by axis and by what they are about:**
 *
 * - vertical opens and closes, from anywhere, including anywhere on the
 *   expanded sheet. It fails on a horizontal drag, which is what leaves the
 *   seek bar's own gesture alone;
 * - horizontal on the bar **shows the track before and after** — the faces sit
 *   in a row three wide and the row follows the finger, so a drag reveals the
 *   neighbour rather than firing a skip you cannot see coming. Let go past a
 *   third of a face and that neighbour becomes the track; let go short and it
 *   springs back with nothing changed. At either end of the queue there is
 *   nothing to reveal, so the row leans against `wall` and returns;
 * - a tap opens it.
 *
 * All three track the finger rather than firing on release: a sheet that only
 * moves after you let go is a sheet you are not sure you are dragging.
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
import { useClock, type Player, type Track } from '@/player';
import { FONT, radius, space, type Theme, sheet } from '@/theme';
import { Artwork } from './artwork';
import { Icon } from './icon';
import { wall } from './rubber';
import { SeekBar } from './seekbar';
import { TAB_BAR } from './tabbar';

/** How tall the collapsed card is. Everything that has to agree about where it
 *  sits is laid out before it is drawn, so this is a constant. */
export const BAR = 60;
/** …how far it is held off each side, so it reads as a card on the page. */
export const BAR_SIDE = space.md;
/** …and off the tab bar, which is the other half of the same idea: a card
 *  touching the thing under it is not floating over anything. */
export const BAR_GAP = space.sm;
/** How far above the card the page is faded out. Enough for a track title to
 *  dissolve rather than be cut in half by an edge. */
export const BAR_FADE = 36;

const SPRING = { damping: 22, stiffness: 240, mass: 0.7 };
/** The carousel's own, which is tighter: this one lands on a track rather than
 *  settling into a resting place, so overshoot would read as a bounce past the
 *  thing you asked for. */
const SLIDE = { damping: 26, stiffness: 320, mass: 0.7 };

/**
 * A colour with its alpha taken off, for the far end of a fade.
 *
 * `'transparent'` is not the answer: it is `rgba(0,0,0,0)` everywhere, so a
 * gradient running to it passes through darkening greys on a light theme —
 * a grubby shadow under the card instead of a fade. What is wanted is *this*
 * background at zero opacity, which is a different colour on each theme and
 * the same hue as what it is fading from.
 */
function clear(hex: string): string {
  const h = hex.replace('#', '');
  const full = h.length === 3 ? h.split('').map((c) => c + c).join('') : h;
  const n = parseInt(full.slice(0, 6), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, 0)`;
}

/**
 * The fade under the bar, drawn by the shell rather than by the player.
 *
 * It belongs to the *page*, not to the card: it is what the list disappears
 * into, it is there whether or not anything is playing (the tab bar needs the
 * same treatment), and it must not move when the sheet is dragged. Put inside
 * the player's own container it would slide up with it and take the page's
 * bottom edge along for the ride.
 */
export function PlayerScrim({
  theme,
  bottom,
  playing,
}: {
  theme: Theme;
  /** The safe area. */
  bottom: number;
  /** Whether the card is there to make room for. */
  playing: boolean;
}) {
  const height = BAR_FADE + (playing ? BAR + BAR_GAP : 0) + TAB_BAR + bottom;
  // Solid from the card's own top edge down — the card floats on the page's
  // background, and a gradient still running underneath it would leave the
  // tab bar sitting on a wash.
  const solid = BAR_FADE / height;
  return (
    <LinearGradient
      pointerEvents="none"
      colors={[clear(theme.bg), theme.bg, theme.bg]}
      locations={[0, solid, 1]}
      style={[s0.scrim, { height }]}
    />
  );
}

const s0 = StyleSheet.create({
  scrim: { position: 'absolute', left: 0, right: 0, bottom: 0 },
});

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

  // How far the container travels between the two faces: the card's resting
  // top edge, measured from the top of the screen. The gap is part of it —
  // the card sits a gap above the tab bar, not against it.
  const travel = Math.max(1, height - bottom - TAB_BAR - BAR_GAP - BAR);

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

      {/* …and the card, pinned to the container's top edge, which is exactly
          where it belongs when the container is down. */}
      <Animated.View
        style={[s.barWrap, barFace]}
        pointerEvents={expanded ? 'none' : 'box-none'}
      >
        <MiniBar
          player={player}
          album={album}
          theme={theme}
          lift={liftBar}
          tap={tap}
          onDevices={onDevices}
        />
      </Animated.View>
    </Animated.View>
  );
}

/**
 * The card: what is playing, where it is playing, and play/pause.
 *
 * Three things and no more. This is one thumb's width from the bottom of the
 * screen and is the only control that has to be reachable from anywhere, so
 * everything else is a swipe or a tap away — next and previous are the drag,
 * and the rest is the sheet.
 *
 * **What the drag moves is the faces, not the whole card.** The transport
 * stays exactly where a thumb left it: a play button that slides away under a
 * gesture about *which track* is a play button you have to look at to press.
 */
const MiniBar = memo(function MiniBar({
  player,
  album,
  theme,
  lift,
  tap,
  onDevices,
}: {
  player: Player;
  album: string;
  theme: Theme;
  lift: ReturnType<typeof Gesture.Pan>;
  tap: ReturnType<typeof Gesture.Tap>;
  onDevices: () => void;
}) {
  const s = styles(theme);
  const { track, queue, at, skip } = player;
  // How wide one face is, which the drag is measured in. Measured rather than
  // computed, because what is left for the faces depends on whether there is a
  // device button — and a carousel whose stride is a guess lands between two
  // tracks.
  const [face, setFace] = useState(0);
  const dx = useSharedValue(0);

  const before = at > 0 ? (queue[at - 1] ?? null) : null;
  const after = at >= 0 && at + 1 < queue.length ? (queue[at + 1] ?? null) : null;

  const slide = useMemo(
    () =>
      Gesture.Pan()
        .activeOffsetX([-16, 16])
        .failOffsetY([-16, 16])
        .onUpdate((e) => {
          const t = e.translationX;
          // Nothing to reveal on that side, so it leans and comes back. The
          // queue stops at both ends rather than wrapping — a list that loops
          // silently is hard to tell from one that is stuck — and the bar says
          // so by refusing to travel.
          const room = t > 0 ? before !== null : after !== null;
          dx.value = room ? t : wall(t, 36);
        })
        .onEnd((e) => {
          const far = face > 0 && Math.abs(e.translationX) > face * 0.35;
          const quick = Math.abs(e.velocityX) > 550;
          const dir = e.translationX < 0 ? 1 : -1;
          const has = dir === 1 ? after !== null : before !== null;
          if (has && (far || quick)) runOnJS(skip)(dir);
          // Home either way, and the skip is *not* waited for.
          //
          // The obvious version parks the reel on the neighbour until the new
          // track arrives, and it cannot be made to work: `skip` is a message
          // to whichever device is playing, so when the sound is on a laptop
          // the answer comes back over a socket — or not at all — and a bar
          // frozen mid-slide is the failure. Springing home immediately is
          // right in both cases. Playing here, React has swapped the middle
          // face by the time the spring has moved a few pixels, so what
          // settles into the centre *is* the track you dragged towards; on
          // another device the bar returns and the title changes when the
          // broadcast says it has, which is the honest answer rather than a
          // guess about somebody else's queue.
          dx.value = withSpring(0, SLIDE);
        })
        // A gesture the system takes away mid-drag — a back swipe winning, a
        // call arriving — never reaches `onEnd`. `success` is false exactly
        // then, which is what keeps this from re-starting the spring `onEnd`
        // has already begun.
        .onFinalize((_e, success) => {
          if (!success) dx.value = withSpring(0, SLIDE);
        }),
    [after, before, dx, face, skip],
  );

  const row = useAnimatedStyle(() => ({ transform: [{ translateX: dx.value }] }));

  if (!track) return null;
  return (
    <GestureDetector gesture={Gesture.Race(lift, slide, tap)}>
      <View style={s.bar}>
        <View style={s.barRow}>
          <View
            style={s.faces}
            onLayout={(e) => {
              const w = e.nativeEvent.layout.width;
              if (w > 0 && Math.abs(w - face) > 0.5) setFace(w);
            }}
          >
            <Animated.View style={[s.reel, { width: face * 3, left: -face }, row]}>
              <Face track={before} width={face} theme={theme} />
              <Face
                track={track}
                width={face}
                theme={theme}
                album={album}
                meta={<Now player={player} theme={theme} />}
              />
              <Face track={after} width={face} theme={theme} />
            </Animated.View>
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
        <BarProgress theme={theme} />
      </View>
    </GestureDetector>
  );
});

/**
 * One track as the card draws it, drawn three times over.
 *
 * `null` is a real case and draws nothing: it is the empty space past either
 * end of the queue, and something there would be a track that does not exist.
 */
function Face({
  track,
  width,
  theme,
  album,
  meta,
}: {
  track: Track | null;
  width: number;
  theme: Theme;
  /** The album as the library knows it now, which can be fresher than the copy
   *  frozen into the queue. Only the middle face is given one. */
  album?: string;
  /** What the second line says, when it is something other than the creator. */
  meta?: React.ReactNode;
}) {
  const s = styles(theme);
  if (!track) return <View style={{ width }} />;
  return (
    <View style={[s.face, { width }]}>
      <Artwork seed={album || track.album || track.title} size={40} theme={theme} />
      <View style={s.text}>
        <Text style={s.title} numberOfLines={1}>
          {track.title}
        </Text>
        {meta ?? (
          <Text style={s.meta} numberOfLines={1}>
            {track.creator || '—'}
          </Text>
        )}
      </View>
    </View>
  );
}

/**
 * The second line of the middle face.
 *
 * Where it is playing takes it when it is not here, because that is the more
 * surprising fact: a phone that is silent with a full bar is a phone somebody
 * thinks is broken. Its own component so that `buffering` — which is the
 * clock's, and moves — does not re-render the reel around it.
 */
function Now({ player, theme }: { player: Player; theme: Theme }) {
  const { buffering } = useClock();
  const s = styles(theme);
  const at = where(player);
  if (at) {
    return <Playing on={at.name} connecting={at.connecting} theme={theme} small />;
  }
  const track = player.track;
  return (
    <Text style={s.meta} numberOfLines={1}>
      {buffering ? 'buffering…' : track?.url ? track.creator : 'nothing to stream'}
    </Text>
  );
}

/**
 * A hairline of progress along the card's bottom edge.
 *
 * Enough to say the thing is moving without becoming a second control — and
 * along the *bottom* rather than the top, because the card is rounded now and
 * a bar across the top would be a line drawn through two corners. It is its
 * own component for the reason `Now` is: it reads the clock, and the clock
 * moves four times a second.
 */
function BarProgress({ theme }: { theme: Theme }) {
  const { position, duration } = useClock();
  const s = styles(theme);
  const pct = duration > 0 ? Math.min(100, (position / duration) * 100) : 0;
  return (
    <View style={s.line} pointerEvents="none">
      <View style={[s.lineFill, { width: `${pct}%` }]} />
    </View>
  );
}

/**
 * Where the sound is coming from, in the accent, with a speaker beside it.
 *
 * The one line in this app that is drawn in a colour because of *what it says*
 * rather than what it is: a device name is ordinary text until it is somewhere
 * other than the phone in your hand, and then it is the most important thing
 * on the screen.
 */
function Playing({
  on,
  theme,
  small,
  connecting,
}: {
  on: string;
  theme: Theme;
  small?: boolean;
  connecting?: boolean;
}) {
  const s = styles(theme);
  return (
    <View style={s.playingOn}>
      <Icon name="devices" size={small ? 12 : 15} tint={theme.accent} />
      <Text style={[s.playingText, small && s.playingSmall]} numberOfLines={1}>
        {connecting ? `connecting to ${on}…` : on}
      </Text>
    </View>
  );
}

/**
 * Which device the bar names, and whether the sound is on its way there.
 *
 * A hand-off in flight outranks the output, and has to: pressing a speaker in
 * the house starts a second or two of it clearing its queue and fetching the
 * first track, and for that second or two the sound is still *here*. Drawing
 * the old device through it says the press did nothing, and drawing the new
 * one as though it were playing is a bar counting along silence. So the one
 * line says what is actually happening, in the place the thumb already is.
 *
 * `null` is "here, and nothing on its way" — which is when the second line
 * goes back to being the artist's.
 */
function where(player: Player): { name: string; connecting: boolean } | null {
  const named = (id: string | null) => player.devices.find((d) => d.id === id)?.name;
  if (player.moving) {
    return { name: named(player.moving) ?? 'another device', connecting: true };
  }
  if (!player.elsewhere) return null;
  return { name: player.output?.name ?? 'another device', connecting: false };
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
  const at = where(player);
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

        <Scrubber player={player} theme={theme} />

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
            {at ? (
              <Playing on={at.name} connecting={at.connecting} theme={theme} />
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

/** The seek bar and the two clocks, which are the sheet's moving part and
 *  therefore the sheet's only subscriber to the clock. */
function Scrubber({ player, theme }: { player: Player; theme: Theme }) {
  const { position, duration } = useClock();
  const s = styles(theme);
  return (
    <>
      <SeekBar
        position={position}
        duration={duration}
        onSeek={player.seek}
        theme={theme}
        scrubbable={Boolean(player.track?.url) || player.elsewhere}
      />
      <View style={s.clocks}>
        <Text style={s.clock}>{clock(position)}</Text>
        <Text style={s.clock}>{clock(duration)}</Text>
      </View>
    </>
  );
}

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    shell: { position: 'absolute', left: 0, right: 0, top: 0 },
    // The four edges written out: these typings expose `absoluteFill` as a
    // registered style rather than an object, so it cannot be spread into one.
    sheet: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0 },
    expanded: { flex: 1, backgroundColor: t.bg },
    barWrap: { position: 'absolute', top: 0, left: 0, right: 0 },
    bar: {
      marginHorizontal: BAR_SIDE,
      height: BAR,
      backgroundColor: t.raised,
      borderRadius: radius.md,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
      // So the progress line follows the corners rather than squaring them
      // off, and so a face sliding through cannot spill out of the card.
      overflow: 'hidden',
      // A card is only floating if something says so. Android takes the
      // elevation and iOS the shadow; both are deliberately slight, because
      // this sits on a fade that is already doing most of the lifting.
      elevation: 6,
      shadowColor: '#000',
      shadowOpacity: t.dark ? 0.45 : 0.14,
      shadowRadius: 12,
      shadowOffset: { width: 0, height: 4 },
    },
    barRow: {
      flex: 1,
      flexDirection: 'row',
      alignItems: 'center',
      paddingRight: space.sm,
    },
    // The window the reel of faces moves through.
    faces: { flex: 1, height: '100%', overflow: 'hidden', justifyContent: 'center' },
    reel: { position: 'absolute', flexDirection: 'row', alignItems: 'center' },
    face: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.md,
    },
    text: { flex: 1, gap: 1 },
    title: { fontFamily: FONT, fontSize: 14, fontWeight: '600', color: t.text },
    meta: { fontFamily: FONT, fontSize: 12, color: t.dim },
    button: { padding: space.xs },
    line: { position: 'absolute', left: 0, right: 0, bottom: 0, height: 2, backgroundColor: t.border },
    lineFill: { height: 2, backgroundColor: t.accent },
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
  }),
);
