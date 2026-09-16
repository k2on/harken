/**
 * One page for a playlist, an album, an artist or the whole library.
 *
 * Four screens would be four copies of the same list with a different header,
 * and the difference between them is already expressed where it belongs — in
 * the domain, as four queries. So this is one route that carries which of them
 * it is, and the header is the only thing that branches.
 *
 * The queue is what is on screen, taken when play was pressed: skipping
 * follows the list you started in, even after you have walked somewhere else.
 * The desktop's rule, for the desktop's reason — a live reference gets
 * silently redirected by somebody else's edit arriving.
 */

import { useEffect, useMemo } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { router, useLocalSearchParams } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { asId } from '@/mutators.gen';
import { usePlayer } from '@/player';
import type { Source } from '@/peer';
import { space, radius, useTheme, type Theme } from '@/theme';
import { Artwork } from '@/ui/artwork';
import { Icon } from '@/ui/icon';
import { TrackList } from '@/ui/tracklist';
import { useShell } from './_layout';

export default function List() {
  const params = useLocalSearchParams<{ kind?: string; id?: string; name?: string }>();
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { peer, trackOf, addTo, soon, inset } = useShell();
  const player = usePlayer();

  const source = useMemo<Source>(() => {
    const name = params.name ?? '';
    if (params.kind === 'playlist' && params.id) {
      return { kind: 'playlist', id: asId('playlist', params.id), name };
    }
    if (params.kind === 'album') return { kind: 'album', name };
    if (params.kind === 'artist') return { kind: 'artist', name };
    return { kind: 'library' };
  }, [params.kind, params.id, params.name]);

  // The peer reads one list at a time, so arriving here is what points it.
  // Which also means leaving does not have to unpoint it: nothing else reads
  // `shown`, and re-pointing costs one query when the next page arrives.
  useEffect(() => {
    peer.setSource(source);
  }, [peer.setSource, source]);

  const rows = peer.shown;
  const title = source.kind === 'library' ? 'All tracks' : (params.name ?? '');
  const s = styles(theme);

  const start = (at: number) => {
    const row = rows[at];
    if (!row) return;
    player.play(trackOf(row), rows.map(trackOf));
  };

  return (
    <View style={s.page}>
      <View style={[s.head, { paddingTop: insets.top + space.sm }]}>
        <Pressable onPress={() => router.back()} hitSlop={12} accessibilityLabel="back">
          <Icon name="back" size={24} tint={theme.text} />
        </Pressable>
        <Text style={s.headLabel} numberOfLines={1}>
          {title}
        </Text>
        <View style={s.headSpace} />
      </View>

      <TrackList
        rows={rows}
        albumOf={peer.albumOf}
        playingId={player.track?.id}
        theme={theme}
        bottom={inset}
        onPress={(item) => start(rows.findIndex((r) => r.id === item.id))}
        onAdd={addTo}
        onSoon={soon}
        empty={
          source.kind === 'playlist'
            ? 'Nothing on this playlist yet. Swipe a track right to put it here.'
            : 'Nothing here.'
        }
        header={
          <View style={s.top}>
            <Artwork seed={title} size={168} theme={theme} corner={radius.md} />
            <Text style={s.title} numberOfLines={2}>
              {title}
            </Text>
            <Text style={s.under}>
              {KIND[source.kind]} · {rows.length} {rows.length === 1 ? 'track' : 'tracks'}
            </Text>
            <View style={s.actions}>
              <Pressable
                onPress={() => soon('Shuffle is not built yet')}
                hitSlop={10}
                style={s.ghost}
                accessibilityLabel="shuffle"
              >
                <Icon name="shuffle" size={20} tint={theme.dim} />
              </Pressable>
              <Pressable
                onPress={() => start(0)}
                disabled={rows.length === 0}
                style={[s.play, rows.length === 0 && s.playOff]}
                accessibilityLabel="play"
              >
                <Icon name="play" size={26} tint={theme.onAccent} />
              </Pressable>
            </View>
          </View>
        }
      />
    </View>
  );
}

const KIND: Record<Source['kind'], string> = {
  library: 'Library',
  playlist: 'Playlist',
  album: 'Album',
  artist: 'Artist',
};

const styles = (t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    head: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.lg,
      paddingBottom: space.sm,
    },
    headLabel: { flex: 1, fontSize: 15, fontWeight: '600', color: t.text },
    headSpace: { width: 24 },
    top: { alignItems: 'center', gap: space.xs, paddingVertical: space.lg },
    title: {
      fontSize: 22,
      fontWeight: '800',
      color: t.text,
      textAlign: 'center',
      paddingHorizontal: space.lg,
      paddingTop: space.md,
    },
    under: { fontSize: 12.5, color: t.dim },
    actions: {
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'flex-end',
      alignSelf: 'stretch',
      gap: space.lg,
      paddingHorizontal: space.lg,
      paddingTop: space.md,
    },
    ghost: { padding: space.sm },
    play: {
      width: 54,
      height: 54,
      borderRadius: radius.pill,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.accent,
    },
    playOff: { opacity: 0.4 },
  });
