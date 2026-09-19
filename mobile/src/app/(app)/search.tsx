/**
 * Search, over what this peer already has.
 *
 * Locally, against the maintained list, and that is the whole design: every
 * peer holds the entire log, so a search is a filter over a list that is
 * already in memory rather than a request to a server that might not be
 * reachable. It works offline for the same reason everything else does.
 *
 * It matches the title and the creator, which are the two things on a row. Not
 * the album — an album is a *page*, and the browse screen is where you go
 * looking for one; a search that returned every track on an album because the
 * album matched would bury the track you actually typed.
 */

import { useCallback, useMemo, useRef, useState } from 'react';
import { Pressable, StyleSheet, Text, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import type { Item } from 'harken-native';

import { usePlayer } from '@/player';
import { FONT, space, radius, useTheme, type Theme, sheet } from '@/theme';
import { Icon } from '@/ui/icon';
import { TrackList } from '@/ui/tracklist';
import { useShell } from './_layout';

export default function Search() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { peer, trackOf, addTo, soon, inset } = useShell();
  const player = usePlayer();
  const [query, setQuery] = useState('');

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return [];
    // Every word has to appear somewhere, so "bach air" finds the Air on the
    // G string without either word being the start of anything.
    const words = needle.split(/\s+/);
    return peer.items.filter((item: Item) => {
      const hay = `${item.title} ${item.creator}`.toLowerCase();
      return words.every((word) => hay.includes(word));
    });
  }, [peer.items, query]);

  // The same trick `list.tsx` uses, for the same reason: `onPress` must not
  // be a new function every time something else on the screen moves, or every
  // visible row re-renders with it.
  const showing = useRef(rows);
  showing.current = rows;
  const play = useRef(player.play);
  play.current = player.play;
  const of = useRef(trackOf);
  of.current = trackOf;
  const press = useCallback((item: Item) => {
    const list = showing.current;
    play.current(of.current(item), list.map(of.current));
  }, []);

  const s = styles(theme);
  return (
    <View style={[s.page, { paddingTop: insets.top + space.md }]}>
      <Text style={s.title}>Search</Text>
      <View style={s.field}>
        <Icon name="search" size={18} tint={theme.faint} />
        <TextInput
          value={query}
          onChangeText={setQuery}
          placeholder="What do you want to listen to?"
          placeholderTextColor={theme.faint}
          style={s.input}
          autoCorrect={false}
          returnKeyType="search"
          clearButtonMode="while-editing"
        />
        {query ? (
          <Pressable onPress={() => setQuery('')} hitSlop={10} accessibilityLabel="clear">
            <Icon name="close" size={16} tint={theme.dim} />
          </Pressable>
        ) : null}
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
          query.trim()
            ? `Nothing in this library matches “${query.trim()}”.`
            : 'Search this peer’s whole library — it works offline, because the log is already here.'
        }
      />
    </View>
  );
}

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    title: { fontFamily: FONT, fontSize: 27, fontWeight: '800', color: t.text, paddingHorizontal: space.lg },
    field: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.sm,
      margin: space.lg,
      paddingHorizontal: space.md,
      paddingVertical: space.sm,
      backgroundColor: t.card,
      borderRadius: radius.md,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
    },
    input: { flex: 1, fontFamily: FONT, fontSize: 15, color: t.text, paddingVertical: 2 },
  }),
);
