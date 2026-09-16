/**
 * A list of tracks, with the two gestures every row has.
 *
 * One component rather than four, because four screens draw the same list and
 * the swipes have to mean the same thing on all of them — a gesture that adds
 * to a playlist here and queues there is a gesture nobody trusts.
 *
 * **Drag right to put it on a playlist.** The same sheet the `+` opens, and
 * the same sheet the desktop's `a` opens, because it is the same question.
 *
 * **Drag left for what is not built yet.** Playing something next means a
 * queue the session can carry and a verb to insert into it, and neither
 * exists — so it says so rather than springing back for no visible reason. A
 * swipe that appears to do nothing is a gesture people try once.
 */

import { useCallback } from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';
import type { Item } from 'harken-native';

import { space, type Theme } from '@/theme';
import { Icon } from './icon';
import { Swipe } from './swipe';
import { TrackRow } from './trackrow';

export function TrackList({
  rows,
  albumOf,
  playingId,
  theme,
  bottom,
  header,
  empty,
  onPress,
  onAdd,
  onSoon,
}: {
  rows: Item[];
  albumOf: Record<string, string>;
  playingId: string | undefined;
  theme: Theme;
  /** What the player and the tab bar take, so the last row is reachable. */
  bottom: number;
  header?: React.ReactElement;
  empty?: string;
  onPress: (item: Item) => void;
  onAdd: (item: Item) => void;
  onSoon: (what: string) => void;
}) {
  const s = styles(theme);
  const render = useCallback(
    ({ item }: { item: Item }) => (
      <Swipe
        theme={theme}
        leftIcon="addTo"
        leftLabel="Add to playlist"
        onLeft={() => onAdd(item)}
        rightIcon="playlist"
        rightLabel="Play next"
        onRight={() => onSoon('Play next is not built yet')}
      >
        <TrackRow
          item={item}
          album={albumOf[item.id] ?? ''}
          playing={item.id === playingId}
          theme={theme}
          onPress={onPress}
          onAdd={onAdd}
        />
      </Swipe>
    ),
    [albumOf, onAdd, onPress, onSoon, playingId, theme],
  );

  return (
    <FlatList
      data={rows}
      keyExtractor={(item) => item.id}
      renderItem={render}
      ListHeaderComponent={header}
      contentContainerStyle={{ paddingBottom: bottom + space.lg }}
      ListEmptyComponent={
        <View style={s.empty}>
          <Icon name="note" size={22} tint={theme.faint} />
          <Text style={s.emptyText}>{empty ?? 'Nothing here.'}</Text>
        </View>
      }
      // The whole point of the maintained view is that a change costs the rows
      // that moved; a list that re-measures everything on every change would
      // give it all back.
      removeClippedSubviews
      windowSize={11}
      keyboardShouldPersistTaps="handled"
    />
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    empty: { alignItems: 'center', gap: space.sm, paddingVertical: space.xl * 2 },
    emptyText: { fontSize: 13, color: t.dim },
  });
