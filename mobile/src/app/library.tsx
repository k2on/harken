/**
 * The library, the browser, and the player.
 *
 * The same library the iced window shows, because it is the same `apply` — see
 * `domain`. Take it offline with the pill, heart a few tracks, heart some on
 * another peer too, come back: your hearts land *after* whatever arrived while
 * you were away, because "add to the playlist" reads the end of it rather than
 * naming a position. That is the rebase, and it is the one thing this app is
 * really demonstrating.
 *
 * What this screen owns is the arrangement. The peer owns the data and the
 * reads (`src/peer.ts`), the player owns what is sounding (`src/player.tsx`),
 * and the domain owns everything either of them means.
 */

import { useCallback, useMemo, useRef, useState } from 'react';
import { Redirect, router, useLocalSearchParams } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import type { Login } from '@petros/client';
import type { Item } from 'harken-native';
import { Alert, FlatList, Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeIn, LinearTransition } from 'react-native-reanimated';

import { recallServer, remembered, signIn, signOut } from '@/auth';
import { mediaUrl } from '@/media';
import { sourceTitle, usePeer, type Peer } from '@/peer';
import { usePlayer, type Track } from '@/player';
import { radius, space, useTheme, type Theme } from '@/theme';
import { Browse } from '@/ui/browse';
import { Debug } from '@/ui/debug';
import { Icon } from '@/ui/icon';
import { MiniPlayer } from '@/ui/miniplayer';
import { NowPlaying } from '@/ui/nowplaying';
import { TrackRow } from '@/ui/trackrow';

export default function Library() {
  const params = useLocalSearchParams<{ server?: string; online?: string }>();
  // Arriving with no parameter at all (a deep link, a restored screen) asks
  // which server this phone was last pointed at.
  const server = params.server ?? recallServer();
  const login = server ? remembered(server) : null;
  if (!server || !login) {
    // Nobody is signed in here: the library is somebody's.
    return <Redirect href="/" />;
  }
  return <Signed server={server} login={login} online={params.online !== '0'} />;
}

function Signed(props: { server: string; login: Login; online: boolean }) {
  const { server } = props;
  // The login can change under a running peer — turned away and signed in
  // again — and the new token reconnects the same database.
  const [login, setLogin] = useState(props.login);
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  // "offline" is a choice and not a failed connection: no socket at all,
  // rather than a URL nothing answers on.
  const peer = usePeer(login, props.online ? server : null);
  const player = usePlayer();

  const [open, setOpen] = useState(false);
  const [debug, setDebug] = useState(false);
  const [scrub, setScrub] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  const rows = peer.shown;

  /**
   * A library row as the player needs it.
   *
   * `file` is a name in the media store, or a whole URL for something already
   * on the web — `src/media.ts` is the one place that is decided.
   */
  const trackOf = useCallback(
    (item: Item): Track => ({
      id: item.id,
      title: item.title,
      creator: item.creator,
      album: peer.albumOf[item.id] ?? '',
      ms: Number(item.durationMs),
      url: mediaUrl(item.file, server),
    }),
    [peer.albumOf, server],
  );

  // The three row callbacks are built once and read the current list through
  // refs. Built per render instead, every row in the list would re-render on
  // every tick of the player — which is the opposite of what maintaining the
  // view was for.
  const latest = useRef({ rows, trackOf, peer });
  latest.current = { rows, trackOf, peer };

  const onPress = useCallback(
    (item: Item) => {
      const { rows: list, trackOf: make } = latest.current;
      // The queue is what is on screen, taken now: skipping follows the list
      // you pressed play in, even after the browser moves somewhere else.
      player.play(make(item), list.map(make));
    },
    [player.play],
  );

  const onHeart = useCallback((item: Item) => {
    latest.current.peer.setOnPlaylist(item.id, !item.onPlaylist);
  }, []);

  // Stable, because the player sheet builds its drag gesture from it and the
  // status ticks four times a second.
  const closePlayer = useCallback(() => setOpen(false), []);

  const again = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const got = await signIn(server);
      if (got) setLogin(got);
    } finally {
      setBusy(false);
    }
  };

  const leave = () => {
    Alert.alert('Sign out?', `${login.user.name || login.user.id} on ${server}`, [
      { text: 'Stay', style: 'cancel' },
      {
        text: 'Sign out',
        style: 'destructive',
        onPress: async () => {
          await signOut(server);
          router.replace('/');
        },
      },
    ]);
  };

  // The engine showing through: `cursor` is how much of the server's log has
  // been applied, `pending` is what this peer has done that no server has
  // confirmed yet.
  const status = useMemo(
    () =>
      [
        `cursor ${peer.cursor}`,
        `${peer.pending} pending`,
        `mutators v${peer.mutators}`,
        peer.lastMutationMs === null ? null : `${peer.lastMutationMs.toFixed(1)}ms in rust`,
        peer.note || null,
      ]
        .filter(Boolean)
        .join('  ·  '),
    [peer.cursor, peer.pending, peer.mutators, peer.lastMutationMs, peer.note],
  );

  const playingId = player.track?.id;
  const heartedNow = useMemo(
    () => (playingId ? (peer.items.find((i) => i.id === playingId)?.onPlaylist ?? false) : false),
    [peer.items, playingId],
  );

  const s = styles(theme);
  return (
    <View style={[s.page, { paddingTop: insets.top }]}>
      <Header
        peer={peer}
        theme={theme}
        who={login.user.name || login.user.id}
        tracks={rows.length}
        busy={busy}
        onSignIn={again}
        onSignOut={leave}
        onDebug={() => setDebug(true)}
      />

      <Browse
        source={peer.source}
        onSelect={peer.setSource}
        playlists={peer.playlists}
        albums={peer.albums}
        artists={peer.artists}
        libraryCount={peer.items.length}
        theme={theme}
      />

      <Animated.View style={s.list} layout={LinearTransition.duration(180)}>
        <FlatList
          data={rows}
          keyExtractor={(item) => item.id}
          contentContainerStyle={rows.length === 0 ? s.emptyBox : s.rows}
          ListEmptyComponent={<Empty theme={theme} />}
          // The whole point of the maintained view is that a change costs the
          // rows that moved; a list that re-measures everything on every change
          // would give it all back.
          removeClippedSubviews
          initialNumToRender={14}
          windowSize={11}
          renderItem={({ item }) => (
            <TrackRow
              item={item}
              album={peer.albumOf[item.id] ?? ''}
              playing={playingId === item.id}
              theme={theme}
              onPress={onPress}
              onHeart={onHeart}
            />
          )}
        />
      </Animated.View>

      <MiniPlayer
        player={player}
        album={playingId ? (peer.albumOf[playingId] ?? '') : ''}
        theme={theme}
        bottom={insets.bottom}
        onOpen={() => setOpen(true)}
      />

      {debug ? (
        <Debug
          peer={peer}
          login={login}
          server={server}
          theme={theme}
          onClose={() => setDebug(false)}
          top={insets.top}
          bottom={insets.bottom}
        />
      ) : null}

      {open && player.track ? (
        <NowPlaying
          player={player}
          album={playingId ? (peer.albumOf[playingId] ?? '') : ''}
          hearted={heartedNow}
          onHeart={() => playingId && peer.setOnPlaylist(playingId, !heartedNow)}
          onClose={closePlayer}
          theme={theme}
          status={status}
          top={insets.top}
          bottom={insets.bottom}
          scrub={scrub}
          onScrub={setScrub}
        />
      ) : null}
    </View>
  );
}

function Header({
  peer,
  theme,
  who,
  tracks,
  busy,
  onSignIn,
  onSignOut,
  onDebug,
}: {
  peer: Peer;
  theme: Theme;
  who: string;
  tracks: number;
  busy: boolean;
  onSignIn: () => void;
  onSignOut: () => void;
  onDebug: () => void;
}) {
  const s = styles(theme);
  // Turned away by the server — an expired token, a revoked session. The
  // database and the pending edits stay; signing in again as the same person
  // offers them. Not the online pill, because knocking again with the same
  // token is only refused again.
  const denied = peer.denied !== null;
  return (
    <View style={s.head}>
      <View style={s.headText}>
        <Text style={s.kicker} numberOfLines={1}>
          {who}
        </Text>
        <Text style={s.h1} numberOfLines={1}>
          {sourceTitle(peer.source)}
        </Text>
        <Text style={s.sub} numberOfLines={1}>
          {tracks} {tracks === 1 ? 'track' : 'tracks'} · {peer.onPlaylist} hearted
          {peer.pending > 0 ? ` · ${peer.pending} pending` : ''}
        </Text>
      </View>

      <View style={s.actions}>
        <Pressable
          onPress={denied ? onSignIn : peer.toggleLink}
          style={[s.pill, denied ? s.pillDenied : peer.online ? s.pillOn : s.pillOff]}
          accessibilityRole="button"
        >
          <Icon
            name={peer.online && !denied ? 'online' : 'offline'}
            size={13}
            tint={denied || peer.online ? theme.onAccent : theme.dim}
          />
          <Text style={[s.pillText, (denied || peer.online) && s.pillTextOn]}>
            {denied ? (busy ? 'signing in…' : 'sign in again') : peer.online ? 'online' : 'offline'}
          </Text>
        </Pressable>

        {/* Beside the pill rather than buried, because the readings behind it
            are what the pill is refusing to explain. */}
        <Pressable onPress={onDebug} style={s.round} accessibilityLabel="debug">
          <Icon name="debug" size={18} tint={theme.dim} />
        </Pressable>

        <Pressable onPress={onSignOut} style={s.round} accessibilityLabel="sign out">
          <Icon name="signOut" size={18} tint={theme.dim} />
        </Pressable>
      </View>
    </View>
  );
}

/** Nothing to press here: the library is what the server's scanner found, so
 *  an empty one is a question for whoever runs the server. */
function Empty({ theme }: { theme: Theme }) {
  const s = styles(theme);
  return (
    <Animated.View entering={FadeIn.duration(200)} style={s.empty}>
      <Icon name="note" size={34} tint={theme.faint} />
      <Text style={s.emptyTitle}>Nothing here yet</Text>
      <Text style={s.emptyBlurb}>
        The library is filled by the server&apos;s media directory. Put something in it, and it
        appears here, on the desktop and in a browser — one log, not three.
      </Text>
    </Animated.View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    head: {
      paddingHorizontal: space.lg,
      paddingTop: space.md,
      paddingBottom: space.md,
      gap: space.md,
    },
    headText: { gap: 2 },
    kicker: {
      fontSize: 11,
      letterSpacing: 0.9,
      textTransform: 'uppercase',
      color: t.accent,
      fontWeight: '700',
    },
    h1: { fontSize: 30, fontWeight: '800', color: t.text, letterSpacing: -0.5 },
    sub: { fontSize: 12.5, color: t.dim },
    actions: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
    pill: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: 6,
      paddingHorizontal: space.md,
      paddingVertical: 6,
      borderRadius: radius.pill,
      marginRight: 'auto',
    },
    pillOn: { backgroundColor: t.accent },
    pillOff: { backgroundColor: t.card, borderWidth: StyleSheet.hairlineWidth, borderColor: t.border },
    pillDenied: { backgroundColor: t.danger },
    pillText: { fontSize: 12, fontWeight: '700', color: t.dim },
    pillTextOn: { color: t.onAccent },
    round: {
      width: 36,
      height: 36,
      borderRadius: radius.pill,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.card,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
    },
    list: { flex: 1 },
    rows: { paddingTop: space.sm, paddingBottom: space.lg },
    emptyBox: { flexGrow: 1, justifyContent: 'center' },
    empty: { alignItems: 'center', gap: space.sm, paddingHorizontal: space.xl },
    emptyTitle: { fontSize: 18, fontWeight: '700', color: t.text, marginTop: space.sm },
    emptyBlurb: { fontSize: 13.5, lineHeight: 19, color: t.dim, textAlign: 'center' },
    emptyButton: {
      marginTop: space.md,
      paddingHorizontal: space.xl,
      paddingVertical: 12,
      borderRadius: radius.pill,
      backgroundColor: t.accent,
    },
    emptyButtonText: { color: t.onAccent, fontWeight: '700', fontSize: 14.5 },
  });
