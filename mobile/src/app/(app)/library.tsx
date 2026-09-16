/**
 * Your Library: playlists, albums, artists, composers.
 *
 * Sections of one scroll rather than tabs, because they are answers to one
 * question — "what have I got" — and the lists are short enough to see at
 * once. The order is the desktop sidebar's, from the same queries, because the
 * folding lives in the domain: a client that grouped the library itself would
 * group it differently, and the two would disagree about what an album is the
 * first time one met a track with two artists.
 *
 * Every row here is a *page*, so tapping one pushes. Playing is what the pages
 * are for.
 *
 * **A section is drawn only when it has rows.** That is what lets one schema
 * serve every genre: a library of pop has no works, so it has no Composers
 * section, and a library of podcasts has no Albums either. The desktop
 * sidebar follows the same rule, and an empty section is an answer to a
 * question the library cannot answer.
 */

import { useState } from 'react';
import { ScrollView, StyleSheet, Text, View, Pressable } from 'react-native';
import { router } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { space, radius, useTheme, type Theme } from '@/theme';
import { Artwork } from '@/ui/artwork';
import { Debug } from '@/ui/debug';
import { Icon, type IconName } from '@/ui/icon';
import { useShell } from './_layout';

export default function Library() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { peer, login, server, addTo, inset, signOut } = useShell();
  // The one screen that can say what the peer actually holds, and the one that
  // can re-point it. Reached from here because this is the screen about *this
  // phone's* copy of things.
  const [debug, setDebug] = useState(false);
  const s = styles(theme);

  if (debug) {
    return (
      <Debug
        peer={peer}
        login={login}
        server={server}
        theme={theme}
        top={insets.top}
        bottom={inset}
        onClose={() => setDebug(false)}
      />
    );
  }

  return (
    <ScrollView
      style={s.page}
      contentContainerStyle={{ paddingTop: insets.top + space.md, paddingBottom: inset + space.lg }}
    >
      <View style={s.head}>
        <Text style={s.title}>Your Library</Text>
        <View style={s.headButtons}>
          <Pressable onPress={() => setDebug(true)} hitSlop={10} accessibilityLabel="debug">
            <Icon name="debug" size={20} tint={peer.online ? theme.dim : theme.danger} />
          </Pressable>
          <Pressable onPress={signOut} hitSlop={10} accessibilityLabel="sign out">
            <Icon name="signOut" size={20} tint={theme.dim} />
          </Pressable>
        </View>
      </View>

      <Row
        theme={theme}
        icon="library"
        name="All tracks"
        under={`${peer.items.length} in the log`}
        onPress={() => router.push({ pathname: '/list', params: { kind: 'library' } })}
      />

      <Section theme={theme} label="Playlists">
        <Pressable
          onPress={() => addTo(null)}
          style={({ pressed }) => [s.row, pressed && s.pressed]}
          accessibilityRole="button"
        >
          <View style={[s.square, s.plus]}>
            <Icon name="plus" size={20} tint={theme.dim} />
          </View>
          <Text style={s.name}>New playlist</Text>
        </Pressable>
        {peer.playlists.map((list) => (
          <Row
            key={list.id}
            theme={theme}
            icon="playlist"
            name={list.name}
            under="Playlist"
            onPress={() =>
              router.push({
                pathname: '/list',
                params: { kind: 'playlist', id: list.id, name: list.name },
              })
            }
          />
        ))}
      </Section>

      <Section theme={theme} label="Albums" show={peer.albums.length > 0}>
        {peer.albums.map((album) => (
          <Row
            key={album.name}
            theme={theme}
            art={album.name}
            name={album.name}
            under={`${album.creator || 'Album'} · ${album.tracks} ${
              album.tracks === 1 ? 'track' : 'tracks'
            }`}
            onPress={() =>
              router.push({ pathname: '/list', params: { kind: 'album', name: album.name } })
            }
          />
        ))}
      </Section>

      <Section theme={theme} label="Artists" show={peer.artists.length > 0}>
        {peer.artists.map((artist) => (
          <Row
            key={artist.name}
            theme={theme}
            icon="artist"
            round
            name={artist.name}
            under={`${artist.tracks} ${artist.tracks === 1 ? 'track' : 'tracks'}`}
            onPress={() =>
              router.push({ pathname: '/list', params: { kind: 'artist', name: artist.name } })
            }
          />
        ))}
      </Section>

      {/* Whose music this is, as opposed to who played it — which is what
          `Artists` above answers. A work is by somebody and a recording is by
          somebody else, and for three hundred years of music those are
          different people. */}
      <Section theme={theme} label="Composers" show={peer.composers.length > 0}>
        {peer.composers.map((composer) => (
          <Row
            key={composer.name}
            theme={theme}
            icon="artist"
            round
            name={composer.name}
            under={[
              lifespan(composer.born, composer.died),
              `${composer.works} ${composer.works === 1 ? 'work' : 'works'}`,
            ]
              .filter(Boolean)
              .join(' · ')}
            onPress={() =>
              router.push({ pathname: '/browse', params: { kind: 'works', name: composer.name } })
            }
          />
        ))}
      </Section>
    </ScrollView>
  );
}

/** "1685–1750", or nothing at all when nobody has said. */
function lifespan(born: number, died: number): string {
  if (born === 0 && died === 0) return '';
  return `${born || ''}–${died || ''}`;
}

function Section({
  theme,
  label,
  show = true,
  children,
}: {
  theme: Theme;
  label: string;
  /** Drawn only when there is something under it. See the note at the top. */
  show?: boolean;
  children: React.ReactNode;
}) {
  const s = styles(theme);
  if (!show) return null;
  return (
    <View style={s.section}>
      <Text style={s.sectionLabel}>{label}</Text>
      {children}
    </View>
  );
}

function Row({
  theme,
  name,
  under,
  icon,
  art,
  round,
  onPress,
}: {
  theme: Theme;
  name: string;
  under: string;
  icon?: IconName;
  /** Draw the generated cover for this name instead of a glyph. */
  art?: string;
  round?: boolean;
  onPress: () => void;
}) {
  const s = styles(theme);
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [s.row, pressed && s.pressed]}
      accessibilityRole="button"
    >
      {art ? (
        <Artwork seed={art} size={48} theme={theme} />
      ) : (
        <View style={[s.square, round && s.round]}>
          <Icon name={icon ?? 'note'} size={20} tint={theme.dim} />
        </View>
      )}
      <View style={s.text}>
        <Text style={s.name} numberOfLines={1}>
          {name}
        </Text>
        <Text style={s.under} numberOfLines={1}>
          {under}
        </Text>
      </View>
      <Icon name="chevron" size={16} tint={theme.faint} />
    </Pressable>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    head: {
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'space-between',
      paddingHorizontal: space.lg,
      paddingBottom: space.sm,
    },
    title: { fontSize: 27, fontWeight: '800', color: t.text },
    headButtons: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
    section: { paddingTop: space.lg },
    sectionLabel: {
      fontSize: 12,
      fontWeight: '700',
      color: t.dim,
      textTransform: 'uppercase',
      letterSpacing: 0.7,
      paddingHorizontal: space.lg,
      paddingBottom: space.xs,
    },
    row: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.lg,
      paddingVertical: space.sm,
    },
    pressed: { backgroundColor: t.cardHigh },
    square: {
      width: 48,
      height: 48,
      borderRadius: radius.sm,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.card,
    },
    round: { borderRadius: radius.pill },
    plus: { borderWidth: StyleSheet.hairlineWidth, borderColor: t.border },
    text: { flex: 1, gap: 2 },
    name: { fontSize: 15, fontWeight: '600', color: t.text },
    under: { fontSize: 12, color: t.dim },
  });
