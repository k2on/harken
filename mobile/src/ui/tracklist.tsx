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
 *
 * **Every row is exactly `ROW` tall, and that is a scrolling decision.** A
 * `FlatList` whose rows are whatever their contents came to has to lay each
 * one out before it knows where the next goes, so a fast flick is a queue of
 * measurements arriving a frame late — which is the stutter. Told the height
 * up front it can place a screenful without measuring anything, and the row
 * keeps a shape that does not depend on whether a track happens to have a
 * length or a second line.
 */

import { useCallback, useMemo } from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';
import type { Item } from 'harken-native';

import { FONT, space, type Theme, sheet } from '@/theme';
import { Icon } from './icon';
import { Swipe } from './swipe';
import { ROW, TrackRow } from './trackrow';

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
  // One function for the whole list, so a row that has not changed is a row
  // that does not re-render. Its identity depends on nothing that moves while
  // music is playing — which is why both screens that draw this hold their
  // `onPress` in a ref.
  const soon = useCallback(() => onSoon('Play next is not built yet'), [onSoon]);
  const render = useCallback(
    ({ item }: { item: Item }) => (
      <Swipe
        theme={theme}
        leftIcon="addTo"
        leftLabel="Add to playlist"
        onLeft={() => onAdd(item)}
        rightIcon="playlist"
        rightLabel="Play next"
        onRight={soon}
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
    [albumOf, onAdd, onPress, playingId, soon, theme],
  );

  // `bottom` is already everything the chrome and its fade take — see the
  // shell's `inset` — so nothing is added to it here.
  const pad = useMemo(() => ({ paddingBottom: bottom }), [bottom]);

  return (
    <FlatList
      data={rows}
      keyExtractor={key}
      renderItem={render}
      ListHeaderComponent={header}
      contentContainerStyle={pad}
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
      // A screen and a bit either way. Bigger is a longer first paint and a
      // heavier memory ceiling; smaller is blank rows arriving under a fast
      // thumb, which is the thing this is for.
      windowSize={9}
      initialNumToRender={12}
      maxToRenderPerBatch={8}
      // The gap the renderer gets to itself between batches. At the default 50
      // a flick competes with the work of filling in what it just revealed;
      // this leaves each frame alone and fills in a beat later.
      updateCellsBatchingPeriod={70}
      keyboardShouldPersistTaps="handled"
      keyboardDismissMode="on-drag"
    />
  );
}

function key(item: Item): string {
  return item.id;
}

/** Exported so a screen that wants to know how tall a list will be can ask
 *  rather than guess. */
export { ROW };

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    empty: { alignItems: 'center', gap: space.sm, paddingVertical: space.xl * 2 },
    emptyText: { fontFamily: FONT, fontSize: 13, color: t.dim },
  }),
);
