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

import { useCallback, useEffect, useMemo, useRef } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { router, useLocalSearchParams } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { asId } from '@/mutators.gen';
import { usePlayer } from '@/player';
import type { Source } from '@/peer';
import { FONT, space, radius, useTheme, type Theme, sheet } from '@/theme';
import { Artwork } from '@/ui/artwork';
import { Icon, type IconName } from '@/ui/icon';
import { artId, SharedArt, useSharedArt } from '@/ui/shared';
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
    // One performance, in the order the work goes. The id is the derived key
    // `recording_key` made; the name is who played it, which is the only thing
    // that tells two recordings of one work apart.
    if (params.kind === 'recording' && params.id) {
      return { kind: 'recording', id: params.id, name };
    }
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

  /**
   * The cover this page shares with the row that opened it.
   *
   * `null` for the library and for a recording, which are pages nothing in a
   * list draws a cover for — and a flight with only one end is no flight,
   * which the provider already treats as the ordinary case.
   */
  const art = useMemo<{ id: string; glyph: IconName; round: boolean } | null>(() => {
    if (source.kind === 'album') return { id: artId('album', source.name), glyph: 'note', round: false };
    if (source.kind === 'artist') return { id: artId('artist', source.name), glyph: 'artist', round: true };
    if (source.kind === 'playlist') {
      return { id: artId('playlist', source.id), glyph: 'playlist', round: false };
    }
    return null;
  }, [source]);

  // The rows the list is drawing, reachable from a callback that never has to
  // change because of them. Without this every tick of the player rebuilt
  // `onPress`, which is a new prop on every visible row, which defeats the
  // memo each row is wrapped in — and that is most of what made a long list
  // feel heavy while something was playing.
  const showing = useRef(rows);
  showing.current = rows;
  const play = useRef(player.play);
  play.current = player.play;
  const of = useRef(trackOf);
  of.current = trackOf;

  const start = useCallback((at: number) => {
    const list = showing.current;
    const row = list[at];
    if (!row) return;
    play.current(of.current(row), list.map(of.current));
  }, []);

  const press = useCallback(
    (item: { id: string }) => start(showing.current.findIndex((r) => r.id === item.id)),
    [start],
  );

  const { drop } = useSharedArt();
  const leave = useCallback(() => {
    // Before the pop rather than after it: the page is still where it is and
    // can still be measured, which is the only moment a flight out of it can
    // be started from.
    if (art) drop(art.id, title, art.glyph);
    router.back();
  }, [art, drop, title]);

  return (
    <View style={s.page}>
      <View style={[s.head, { paddingTop: insets.top + space.sm }]}>
        <Pressable onPress={leave} hitSlop={12} accessibilityLabel="back">
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
        onPress={press}
        onAdd={addTo}
        onSoon={soon}
        empty={
          source.kind === 'playlist'
            ? 'Nothing on this playlist yet. Swipe a track right to put it here.'
            : 'Nothing here.'
        }
        header={
          <View style={s.top}>
            {art ? (
              <SharedArt
                id={art.id}
                end="page"
                seed={title}
                size={168}
                theme={theme}
                corner={radius.md}
                round={art.round}
                glyph={art.glyph}
              />
            ) : (
              <Artwork seed={title} size={168} theme={theme} corner={radius.md} />
            )}
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
  // `browse.tsx` draws the first two; they are here because `Source` is one
  // type and a `Record` over it has to be total — which is what caught this
  // file the moment the three were added.
  works: 'Composer',
  work: 'Work',
  recording: 'Recording',
};

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    head: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.lg,
      paddingBottom: space.sm,
    },
    headLabel: { flex: 1, fontFamily: FONT, fontSize: 15, fontWeight: '600', color: t.text },
    headSpace: { width: 24 },
    top: { alignItems: 'center', gap: space.xs, paddingVertical: space.lg },
    title: {
      fontFamily: FONT, fontSize: 22,
      fontWeight: '800',
      color: t.text,
      textAlign: 'center',
      paddingHorizontal: space.lg,
      paddingTop: space.md,
    },
    under: { fontFamily: FONT, fontSize: 12.5, color: t.dim },
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
  }),
);
