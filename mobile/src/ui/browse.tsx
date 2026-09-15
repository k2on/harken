/**
 * The sidebar, on a phone.
 *
 * The desktop has a column down the left: Library, then the playlists, then
 * the albums, then the artists, each under its heading. A phone has no column
 * to spare, so the same flat list of choices becomes two rows of chips — the
 * headings on the first row, what is under the chosen heading on the second.
 *
 * The grouping is not this file's idea of how to fold a library. `playlists`,
 * `albums` and `artists` are queries in `domain/src/functions.rs`, and both
 * clients ask them rather than each inventing a way to group by album — which
 * is the only way two clients show the same library the same way.
 */

import { memo, useEffect, useState } from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeIn, FadeOut, LinearTransition } from 'react-native-reanimated';

import { asId } from '@/mutators.gen';
import type { Source } from '@/peer';
import { radius, space, type Theme } from '@/theme';
import type { Album, Artist, Playlist } from 'harken-native';

type Kind = Source['kind'];

function Chip({
  label,
  count,
  on,
  theme,
  onPress,
}: {
  label: string;
  count?: number;
  on: boolean;
  theme: Theme;
  onPress: () => void;
}) {
  const s = styles(theme);
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [s.chip, on && s.chipOn, pressed && !on && s.chipPressed]}
    >
      <Text style={[s.chipText, on && s.chipTextOn]} numberOfLines={1}>
        {label}
      </Text>
      {count === undefined ? null : (
        <Text style={[s.chipCount, on && s.chipCountOn]}>{count}</Text>
      )}
    </Pressable>
  );
}

function BrowseRows({
  source,
  onSelect,
  playlists,
  albums,
  artists,
  libraryCount,
  onNewPlaylist,
  theme,
}: {
  source: Source;
  onSelect: (next: Source) => void;
  playlists: Playlist[];
  albums: Album[];
  artists: Artist[];
  libraryCount: number;
  /** Opens the playlist sheet with nothing to add: making one, and picking
   *  from a list too long for a strip of chips. */
  onNewPlaylist: () => void;
  theme: Theme;
}) {
  // Which heading is open. Follows the source when that changes underneath —
  // picking a track's album from somewhere else should open Albums here.
  const [kind, setKind] = useState<Kind>(source.kind);
  useEffect(() => setKind(source.kind), [source.kind]);

  const s = styles(theme);
  const headings: { kind: Kind; label: string; count?: number }[] = [
    { kind: 'library', label: 'Library', count: libraryCount },
    { kind: 'playlist' as const, label: 'Playlists' },
    ...(albums.length ? [{ kind: 'album' as const, label: 'Albums' }] : []),
    ...(artists.length ? [{ kind: 'artist' as const, label: 'Artists' }] : []),
  ];

  return (
    <View style={s.wrap}>
      <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={s.strip}>
        {headings.map((h) => (
          <Chip
            key={h.kind}
            label={h.label}
            count={h.count}
            on={kind === h.kind}
            theme={theme}
            onPress={() => {
              // "Library" is a choice and not a heading: there is nothing
              // under it, so tapping it shows it.
              if (h.kind === 'library') onSelect({ kind: 'library' });
              setKind(h.kind);
            }}
          />
        ))}
      </ScrollView>

      {kind === 'library' ? null : (
        <Animated.View
          entering={FadeIn.duration(160)}
          exiting={FadeOut.duration(120)}
          layout={LinearTransition.duration(180)}
        >
          <ScrollView
            horizontal
            showsHorizontalScrollIndicator={false}
            contentContainerStyle={s.strip}
          >
            {kind === 'playlist' && (
              <>
                {playlists.map((p) => (
                  <Chip
                    key={p.id}
                    label={p.name}
                    on={source.kind === 'playlist' && source.id === p.id}
                    theme={theme}
                    onPress={() =>
                      onSelect({ kind: 'playlist', id: asId('playlist', p.id), name: p.name })
                    }
                  />
                ))}
                {/* Last rather than first: the chips are a list you read
                    left to right, and the way to add one belongs at the end
                    of it, not in front of what is already there. */}
                <Chip label="+ New" on={false} theme={theme} onPress={onNewPlaylist} />
              </>
            )}
            {kind === 'album' &&
              albums.map((a) => (
                <Chip
                  key={a.name}
                  label={a.name}
                  count={Number(a.tracks)}
                  on={source.kind === 'album' && source.name === a.name}
                  theme={theme}
                  onPress={() => onSelect({ kind: 'album', name: a.name })}
                />
              ))}
            {kind === 'artist' &&
              artists.map((a) => (
                <Chip
                  key={a.name}
                  label={a.name}
                  count={Number(a.tracks)}
                  on={source.kind === 'artist' && source.name === a.name}
                  theme={theme}
                  onPress={() => onSelect({ kind: 'artist', name: a.name })}
                />
              ))}
          </ScrollView>
        </Animated.View>
      )}
    </View>
  );
}

/** Memoised: the player's status ticks four times a second and the screen
 *  holding this re-renders with it, while none of these props move unless the
 *  library does. */
export const Browse = memo(BrowseRows);

const styles = (t: Theme) =>
  StyleSheet.create({
    wrap: { gap: space.sm },
    strip: { gap: space.sm, paddingHorizontal: space.lg },
    chip: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.sm,
      paddingHorizontal: space.md,
      paddingVertical: 7,
      borderRadius: radius.pill,
      backgroundColor: t.card,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
      maxWidth: 220,
    },
    chipPressed: { backgroundColor: t.cardHigh },
    chipOn: { backgroundColor: t.accent, borderColor: t.accent },
    chipText: { fontSize: 13.5, fontWeight: '600', color: t.text, flexShrink: 1 },
    chipTextOn: { color: t.onAccent },
    chipCount: { fontSize: 11, color: t.faint, fontVariant: ['tabular-nums'] },
    chipCountOn: { color: t.onAccent, opacity: 0.75 },
  });
