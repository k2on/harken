/**
 * Which playlists a track is on, and the two things you can do about it.
 *
 * A sheet rather than a menu, because the answer is a list of unknown length:
 * a phone has no room for a dropdown that might hold thirty names, and a
 * menu that scrolls is a sheet that is pretending.
 *
 * It is a **toggle**, not an add. A list of every playlist with nothing marked
 * is a list you can put the same track on twice and never take it off again —
 * so it opens by asking `playlistsOf`, and every row says which it is. That
 * question is one query for one track, asked when the sheet opens; carrying
 * every track's memberships in the shelf would make every change in the app
 * pay for something only this screen asks.
 *
 * Making a playlist lives here too, and deliberately: the moment you want one
 * is the moment you have a track that does not belong on any of the others.
 */

import { useEffect, useState } from 'react';
import {
  ActivityIndicator,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import Animated, { FadeIn, FadeOut, SlideInDown } from 'react-native-reanimated';

import { asId } from '@/mutators.gen';
import type { Peer } from '@/peer';
import { radius, space, type Theme } from '@/theme';
import type { Item, Playlist } from 'harken-native';
import { Icon } from './icon';

export function Playlists({
  item,
  peer,
  theme,
  bottom,
  onClose,
}: {
  /** The track this is about, or null when it is about the playlists
   *  themselves — the same sheet, reached from the browser's `+`. */
  item: Item | null;
  peer: Peer;
  theme: Theme;
  bottom: number;
  onClose: () => void;
}) {
  const s = styles(theme);
  // What the track is on, held here and kept in step by hand. The alternative
  // is re-asking after every tap, which is a query and a render for something
  // this already knows the answer to.
  const [on, setOn] = useState<Set<string> | null>(null);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState('');

  useEffect(() => {
    setOn(item === null ? new Set() : new Set(peer.playlistsOf(item.id).map((p) => p.id)));
    // The track is what this is about; a change to the playlists themselves
    // comes back through `peer.playlists` below.
  }, [item?.id]);

  const pick = (list: Playlist) => {
    if (item === null) {
      // No track to add: the rows are the browser's, so picking one shows it.
      peer.setSource({ kind: 'playlist', id: asId('playlist', list.id), name: list.name });
      onClose();
      return;
    }
    if (on === null) return;
    const has = on.has(list.id);
    const next = new Set(on);
    if (has) next.delete(list.id);
    else next.add(list.id);
    setOn(next);
    peer.setOnPlaylist(asId('playlist', list.id), item.id, !has);
  };

  const make = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    peer.newPlaylist(trimmed);
    setName('');
    setNaming(false);
    // Not added to the new list here: `create_playlist` and `add_to_playlist`
    // are two entries in the log and the id of the first is not known until it
    // has been applied. The new row appears below, one tap away.
  };

  return (
    <Animated.View style={s.scrim} entering={FadeIn.duration(140)} exiting={FadeOut.duration(120)}>
      {/* The backdrop closes it. A sheet with only a button to dismiss it is
          a sheet people feel trapped by. */}
      <Pressable style={s.backdrop} onPress={onClose} accessibilityLabel="close" />

      <Animated.View style={[s.sheet, { paddingBottom: bottom + space.lg }]} entering={SlideInDown}>
        <View style={s.grabber} />
        <Text style={s.title} numberOfLines={1}>
          {item ? item.title : 'Playlists'}
        </Text>
        <Text style={s.sub} numberOfLines={1}>
          {item ? item.creator || '—' : 'pick one, or make another'}
        </Text>

        <ScrollView style={s.list} contentContainerStyle={s.listInner}>
          {on === null ? (
            <ActivityIndicator color={theme.accent} style={{ paddingVertical: space.lg }} />
          ) : peer.playlists.length === 0 ? (
            <Text style={s.empty}>No playlists yet.</Text>
          ) : (
            peer.playlists.map((list) => {
              const has = on.has(list.id);
              return (
                <Pressable
                  key={list.id}
                  onPress={() => pick(list)}
                  style={({ pressed }) => [s.row, pressed && s.rowPressed]}
                  accessibilityRole={item ? 'checkbox' : 'button'}
                  accessibilityState={item ? { checked: has } : undefined}
                >
                  <Icon
                    name={item === null ? 'playlist' : has ? 'ticked' : 'untick'}
                    size={21}
                    tint={has ? theme.accent : theme.faint}
                  />
                  <Text style={[s.rowText, has && s.rowTextOn]} numberOfLines={1}>
                    {list.name}
                  </Text>
                </Pressable>
              );
            })
          )}

          {naming ? (
            <View style={s.naming}>
              <TextInput
                value={name}
                onChangeText={setName}
                placeholder="Playlist name"
                placeholderTextColor={theme.faint}
                style={s.input}
                autoFocus
                returnKeyType="done"
                onSubmitEditing={make}
              />
              <Pressable onPress={make} style={s.make} accessibilityRole="button">
                <Text style={s.makeText}>Create</Text>
              </Pressable>
            </View>
          ) : (
            <Pressable
              onPress={() => setNaming(true)}
              style={({ pressed }) => [s.row, pressed && s.rowPressed]}
              accessibilityRole="button"
            >
              <Icon name="plus" size={21} tint={theme.dim} />
              <Text style={s.rowText}>New playlist</Text>
            </Pressable>
          )}
        </ScrollView>
      </Animated.View>
    </Animated.View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    // Written out rather than spread from `absoluteFill`: these typings
    // expose it as a registered style and not as an object, so it cannot be
    // spread into one. The same four edges either way.
    scrim: {
      position: 'absolute',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      justifyContent: 'flex-end',
    },
    backdrop: {
      position: 'absolute',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      backgroundColor: t.scrim,
    },
    sheet: {
      backgroundColor: t.raised,
      borderTopLeftRadius: radius.xl,
      borderTopRightRadius: radius.xl,
      paddingHorizontal: space.lg,
      paddingTop: space.sm,
      borderTopWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
      maxHeight: '72%',
    },
    grabber: {
      alignSelf: 'center',
      width: 38,
      height: 4,
      borderRadius: radius.pill,
      backgroundColor: t.border,
      marginBottom: space.md,
    },
    title: { fontSize: 16, fontWeight: '700', color: t.text },
    sub: { fontSize: 12.5, color: t.dim, marginTop: 2 },
    list: { marginTop: space.md },
    listInner: { paddingBottom: space.sm },
    empty: { color: t.faint, fontSize: 13, paddingVertical: space.md },
    row: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingVertical: space.md,
    },
    rowPressed: { backgroundColor: t.cardHigh, borderRadius: radius.md },
    rowText: { fontSize: 15, color: t.text, flex: 1 },
    rowTextOn: { color: t.accent, fontWeight: '600' },
    naming: { flexDirection: 'row', alignItems: 'center', gap: space.sm, paddingVertical: space.sm },
    input: {
      flex: 1,
      color: t.text,
      fontSize: 15,
      backgroundColor: t.card,
      borderRadius: radius.md,
      paddingHorizontal: space.md,
      paddingVertical: space.sm,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
    },
    make: {
      backgroundColor: t.accent,
      borderRadius: radius.md,
      paddingHorizontal: space.md,
      paddingVertical: space.sm,
    },
    makeText: { color: t.onAccent, fontWeight: '700', fontSize: 14 },
  });
