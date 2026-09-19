/**
 * One composer's works, or one work's recordings.
 *
 * The two pages between a composer and a track, and they are one screen for
 * the reason `list.tsx` is one screen for four: the difference between them is
 * already expressed where it belongs — in the domain, as two queries — and two
 * files would be two copies of a list of rows with a different header.
 *
 * Neither is a list of *tracks*, which is what separates this from `list.tsx`
 * rather than making it a fifth case there. A work has no tracks of its own
 * until you have picked which performance of it you mean.
 */

import { useCallback, useEffect, useMemo } from 'react';
import { FlatList, Pressable, StyleSheet, Text, View } from 'react-native';
import { router, useLocalSearchParams } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import type { Source } from '@/peer';
import { FONT, space, radius, useTheme, type Theme, sheet } from '@/theme';
import { Icon, type IconName } from '@/ui/icon';
import { artId, SharedArt, useSharedArt } from '@/ui/shared';
import { useShell } from './_layout';

/** One row, whichever of the two lists it came from. */
type Entry = {
  key: string;
  name: string;
  under: string;
  /** What the derived square is drawn from. */
  seed: string;
  /** Which record it is, so its cover can fly to the page it opens. */
  id: string;
  onPress: () => void;
};

export default function Browse() {
  const params = useLocalSearchParams<{ kind?: string; id?: string; name?: string }>();
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { peer, inset } = useShell();
  const { lift, drop } = useSharedArt();

  // Narrowed to the two this screen draws, rather than the whole union: both
  // carry a name, and typing it as `Source` would make every use of that name
  // a case analysis about pages this file cannot show.
  const source = useMemo<Extract<Source, { kind: 'works' | 'work' }>>(() => {
    const name = params.name ?? '';
    if (params.kind === 'work' && params.id) return { kind: 'work', id: params.id, name };
    return { kind: 'works', name };
  }, [params.kind, params.id, params.name]);

  useEffect(() => {
    peer.setSource(source);
  }, [peer.setSource, source]);

  const composer = peer.composers.find((c) => c.name === source.name);
  const work = source.kind === 'work' ? peer.works.find((w) => w.id === source.id) : undefined;

  const rows: Entry[] =
    source.kind === 'works'
      ? peer.works.map((w) => ({
          key: w.id,
          name: w.title,
          // The catalogue number, which is the one name a work has that
          // survives translation — and what tells two "Ballades" apart.
          under: [w.catalogue, w.form, `${w.recordings} ${plural(w.recordings, 'recording')}`]
            .filter(Boolean)
            .join(' · '),
          seed: w.title,
          id: artId('work', w.id),
          onPress: () => {
            lift(artId('work', w.id));
            router.push({
              pathname: '/browse',
              params: { kind: 'work', id: w.id, name: w.title },
            });
          },
        }))
      : peer.recordings.map((r) => ({
          key: r.id,
          // A recording nobody is credited on, which the demo has: the honest
          // answer rather than a guess at who played it.
          name: r.performers || 'Performer not named',
          under: [
            `${r.tracks} ${plural(r.tracks, 'track')}`,
            r.recorded > 0 ? String(r.recorded) : '',
            r.label,
            r.licence,
          ]
            .filter(Boolean)
            .join(' · '),
          seed: r.performers || r.id,
          id: artId('recording', r.id),
          onPress: () => {
            // A recording's page is a track list with no cover of its own, so
            // this id has one end and never flies. Given anyway, because an
            // `Entry` with a sometimes-absent id is a field every call site
            // has to think about.
            lift(artId('recording', r.id));
            router.push({
              pathname: '/list',
              params: { kind: 'recording', id: r.id, name: r.performers },
            });
          },
        }));

  const title = source.kind === 'work' ? (work?.title ?? source.name) : source.name;
  const under =
    source.kind === 'work'
      ? [work?.catalogue, work?.composer, work?.period].filter(Boolean).join(' · ')
      : [
          lifespan(composer?.born ?? 0, composer?.died ?? 0),
          `${peer.works.length} ${plural(peer.works.length, 'work')}`,
        ]
          .filter(Boolean)
          .join(' · ');
  const s = styles(theme);

  // A composer is a person and a work is a thing, which decides both the shape
  // of the square and what is drawn on it — and both ends of the flight have
  // to agree, so it is one answer read twice rather than two literals.
  const person = source.kind === 'works';
  const mine = person ? artId('composer', source.name) : artId('work', source.id);
  const glyph: IconName = person ? 'artist' : 'note';

  const leave = useCallback(() => {
    drop(mine, title, glyph);
    router.back();
  }, [drop, glyph, mine, title]);

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

      <FlatList
        data={rows}
        keyExtractor={(row) => row.key}
        contentContainerStyle={{ paddingBottom: inset }}
        ListHeaderComponent={
          <View style={s.top}>
            <SharedArt
              id={mine}
              end="page"
              seed={title}
              size={168}
              theme={theme}
              // A person is a circle and a work is a square, the one thing
              // every music app agrees about.
              corner={radius.md}
              round={person}
              glyph={glyph}
            />
            <Text style={s.title} numberOfLines={2}>
              {title}
            </Text>
            <Text style={s.under}>{under}</Text>
          </View>
        }
        ListEmptyComponent={
          <Text style={s.empty}>
            {source.kind === 'works' ? 'Nothing by them yet.' : 'No recordings of it yet.'}
          </Text>
        }
        renderItem={({ item }) => (
          <Pressable
            onPress={item.onPress}
            style={({ pressed }) => [s.row, pressed && s.pressed]}
            accessibilityRole="button"
          >
            <SharedArt
              id={item.id}
              end="row"
              seed={item.seed}
              size={48}
              theme={theme}
              glyph={source.kind === 'works' ? 'note' : 'artist'}
            />
            <View style={s.text}>
              <Text style={s.name} numberOfLines={2}>
                {item.name}
              </Text>
              <Text style={s.rowUnder} numberOfLines={1}>
                {item.under}
              </Text>
            </View>
            <Icon name="chevron" size={16} tint={theme.faint} />
          </Pressable>
        )}
      />
    </View>
  );
}

function plural(n: number, word: string): string {
  return n === 1 ? word : `${word}s`;
}

/** "1685–1750", or nothing at all when nobody has said. */
function lifespan(born: number, died: number): string {
  if (born === 0 && died === 0) return '';
  return `${born || ''}–${died || ''}`;
}

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
    under: { fontFamily: FONT, fontSize: 12.5, color: t.dim, textAlign: 'center', paddingHorizontal: space.lg },
    empty: { fontFamily: FONT, fontSize: 13, color: t.dim, textAlign: 'center', paddingTop: space.lg },
    row: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingHorizontal: space.lg,
      paddingVertical: space.sm,
    },
    pressed: { backgroundColor: t.cardHigh },
    text: { flex: 1, gap: 2 },
    name: { fontFamily: FONT, fontSize: 15, fontWeight: '600', color: t.text },
    rowUnder: { fontFamily: FONT, fontSize: 12, color: t.dim },
  }),
);
