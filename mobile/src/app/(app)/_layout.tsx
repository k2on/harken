/**
 * The shell everything signed in is drawn inside.
 *
 * One `Stack`, with the player and the tab bar drawn over it. Not
 * `expo-router`'s `Tabs`, and the reason decides the layout: the player bar
 * has to sit *above* the tab bar and *below* every screen, including the ones
 * pushed over the tabs — and a router-owned tab bar is a sibling of its
 * screens with nowhere between them to put a third thing. So a tab is a
 * `replace` and a record is a push, which is also what makes the back gesture
 * mean "out of this album" rather than "back to the tab I was on".
 *
 * **The peer is here and nowhere else.** It used to be opened by the library
 * screen, which was fine while that screen was the whole app; six screens
 * later, each one opening its own would run the read model six times against
 * one session. `@petros/client` keys the session by actor and would hand them
 * all the same database, so it would not be *wrong* — just paid for six times
 * per change, which is the thing maintaining the view was for.
 *
 * The two sheets live here too, for the same reason: a row on any screen can
 * ask to be put on a playlist, and the player can ask where it is playing.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { Redirect, Stack, router } from 'expo-router';
import { Alert, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import Animated, { FadeIn, FadeOut } from 'react-native-reanimated';
import type { Login } from '@petros/client';
import type { Item } from 'harken-native';

import { offline, recallServer, remembered, signIn, signOut } from '@/auth';
import { listening } from '@/listening';
import { mediaUrl } from '@/media';
import { usePeer, type Peer } from '@/peer';
import { usePlayer, type Track } from '@/player';
import { FONT, radius, space, useTheme, type Theme, sheet } from '@/theme';
import { Devices } from '@/ui/devices';
import { PlayerScrim, PlayerSheet, BAR, BAR_FADE, BAR_GAP } from '@/ui/player';
import { Playlists } from '@/ui/playlists';
import { SharedArtProvider } from '@/ui/shared';
import { TabBar, TAB_BAR } from '@/ui/tabbar';

/** What every screen under this shell can ask for. */
export type Shell = {
  peer: Peer;
  login: Login;
  server: string;
  /** A library row as the player needs it. */
  trackOf: (item: Item) => Track;
  /** Open the playlist sheet for a track, or for the playlists themselves. */
  addTo: (item: Item | null) => void;
  /** Say something that has not been built yet, out loud and briefly. */
  soon: (what: string) => void;
  signInAgain: () => void;
  signOut: () => void;
  /** How much room the player and the tab bar take at the bottom, so a list
   *  can end above them rather than under them. */
  inset: number;
};

const Context = createContext<Shell | null>(null);

export function useShell(): Shell {
  const shell = useContext(Context);
  if (!shell) throw new Error('useShell outside the app shell');
  return shell;
}

export default function AppLayout() {
  const server = recallServer();
  const login = server ? remembered(server) : null;
  if (!server || !login) {
    // Nobody is signed in here: the library is somebody's.
    return <Redirect href="/" />;
  }
  return <Signed server={server} login={login} />;
}

function Signed({ server, login: first }: { server: string; login: Login }) {
  // The login can change under a running peer — turned away and signed in
  // again — and the new token reconnects the same database.
  const [login, setLogin] = useState(first);
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  // "Offline" is a choice and not a failed connection: no socket at all,
  // rather than a URL nothing answers on.
  const peer = usePeer(login, offline() ? null : server);
  const player = usePlayer();

  const [adding, setAdding] = useState<{ item: Item | null } | null>(null);
  const [picking, setPicking] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  // Where this phone's listening session is, and who it is there. The socket
  // outlives every screen — the provider that drives it is at the root — so
  // this points it rather than owning it, exactly as the peer's session is
  // pointed.
  useEffect(() => {
    if (offline()) {
      listening.close();
      return;
    }
    // A device *is* a login: `login.session` is one login on one device,
    // already issued, already stable across a relaunch, and honestly new when
    // somebody signs out and back in.
    listening.point(server, login.token, login.session);
  }, [server, login.token, login.session]);

  const trackOf = useCallback(
    (item: Item): Track => ({
      id: item.id,
      title: item.title,
      creator: item.creator,
      album: peer.albumOf[item.id] ?? '',
      ms: Number(item.durationMs),
      url: mediaUrl(item.file, server),
      // Beside the URL rather than instead of it: the URL is this phone's
      // answer, and the path is what another device is handed so that it can
      // resolve one of its own.
      file: item.file,
    }),
    [peer.albumOf, server],
  );

  const soon = useCallback((what: string) => {
    setNote(what);
    // Long enough to read and short enough not to be in the way. It clears
    // itself rather than needing a dismiss, because nothing here is a
    // decision.
    setTimeout(() => setNote((now) => (now === what ? null : now)), 1900);
  }, []);

  const again = useCallback(async () => {
    const got = await signIn(server);
    if (got) setLogin(got);
  }, [server]);

  const leave = useCallback(() => {
    Alert.alert('Sign out?', `${login.user.name || login.user.id} on ${server}`, [
      { text: 'Stay', style: 'cancel' },
      {
        text: 'Sign out',
        style: 'destructive',
        onPress: async () => {
          await signOut(server);
          // A device is a login, so signing out is this phone *leaving* the
          // session rather than going quiet inside it.
          listening.close();
          router.replace('/');
        },
      },
    ]);
  }, [login.user.id, login.user.name, server]);

  // What the player and the tab bar take up. A list ends above it rather than
  // under it, which is the one number every screen needs from here.
  //
  // Three things, and each is really taken: the tab bar with its safe area,
  // the card and the air under it, and the fade above the card. The fade
  // counts because content is *washed* inside it — a last row ending halfway
  // up it is a last row you can see and cannot quite read, which is worse
  // than one hidden outright. So a list ends where the fade begins and every
  // screen adds nothing of its own to this.
  const inset = TAB_BAR + insets.bottom + (player.track ? BAR + BAR_GAP : 0) + BAR_FADE;

  const shell = useMemo<Shell>(
    () => ({
      peer,
      login,
      server,
      trackOf,
      addTo: (item) => setAdding({ item }),
      soon,
      signInAgain: () => void again(),
      signOut: leave,
      inset,
    }),
    [peer, login, server, trackOf, soon, again, leave, inset],
  );

  const album = player.track ? (peer.albumOf[player.track.id] ?? player.track.album) : '';
  const s = styles(theme);

  return (
    <Context.Provider value={shell}>
      <SharedArtProvider>
        <View style={s.page}>
          <Stack
            screenOptions={{
              headerShown: false,
              contentStyle: { backgroundColor: theme.bg },
              animation: 'slide_from_right',
            }}
          >
            {/* The three tabs move sideways, so they do not animate. A tab is a
                `replace` between peers and `slide_from_right` would say one of
                them had come out of the other — which is the push a *record*
                earns and nothing else should borrow. The incoming screen's
                options are what a replace is drawn with, so it is said here,
                once per tab, rather than at the call site. */}
            <Stack.Screen name="home" options={{ animation: 'none' }} />
            <Stack.Screen name="search" options={{ animation: 'none' }} />
            <Stack.Screen name="library" options={{ animation: 'none' }} />
          </Stack>

          {/* The page fades out into the bottom of the screen rather than being
              cut off by the bar's edge — which is what lets the bar be a card
              floating on the list instead of a shelf bolted to the tab bar. It
              is the *page's*, so it is drawn here and does not move when the
              sheet is dragged. */}
          <PlayerScrim theme={theme} bottom={insets.bottom} playing={Boolean(player.track)} />

          {/* The tab bar next, so the expanded player covers it — a full-screen
              player with a tab bar across the bottom of it is two apps.
              Collapsed, the player's card sits a gap above it, because that is
              what `travel` is measured from. */}
          <View style={s.tabs}>
            <TabBar theme={theme} bottom={insets.bottom} />
          </View>

          <PlayerSheet
            player={player}
            album={album}
            theme={theme}
            top={insets.top}
            bottom={insets.bottom}
            onAdd={() => {
              const now = peer.items.find((i) => i.id === player.track?.id);
              setAdding({ item: now ?? null });
            }}
            onDevices={() => setPicking(true)}
          />

          {note ? (
            <Animated.View
              entering={FadeIn.duration(120)}
              exiting={FadeOut.duration(200)}
              style={[s.note, { bottom: inset + space.lg }]}
              pointerEvents="none"
            >
              <Text style={s.noteText}>{note}</Text>
            </Animated.View>
          ) : null}

          {adding ? (
            <Playlists
              item={adding.item}
              peer={peer}
              theme={theme}
              bottom={insets.bottom}
              onClose={() => setAdding(null)}
            />
          ) : null}

          {/* Last, so it is over the player it was opened from. */}
          {picking ? (
            <Devices
              devices={player.devices}
              output={player.output?.id ?? null}
              me={player.me}
              theme={theme}
              bottom={insets.bottom}
              onPick={player.pickDevice}
              onClose={() => setPicking(false)}
            />
          ) : null}
        </View>
      </SharedArtProvider>
    </Context.Provider>
  );
}

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    tabs: { position: 'absolute', left: 0, right: 0, bottom: 0 },
    note: {
      position: 'absolute',
      left: space.lg,
      right: space.lg,
      backgroundColor: t.cardHigh,
      borderRadius: radius.md,
      paddingHorizontal: space.md,
      paddingVertical: space.sm,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
    },
    noteText: { fontFamily: FONT, fontSize: 13, color: t.text, textAlign: 'center' },
  }),
);
