/**
 * Putting something in the library.
 *
 * Five fields, because `add_song` takes five: a title, whoever made it, the
 * album it is on, how long it runs, and the file the bytes are in. The desktop
 * asks for two of them and sends empty strings for the rest, which is fine for
 * watching the rebase and useless for listening to anything — so this asks for
 * the URL, and that is the difference between a demo of a sync engine and a
 * music app you can add a track to from a phone.
 *
 * The length is optional and says so. `expo-audio` reports what the stream
 * actually runs for once it has read enough of it, and the bar prefers that
 * figure; what is typed here is only what the list shows before anything has
 * been played.
 */

import { useState } from 'react';
import {
  KeyboardAvoidingView,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import Animated, { FadeIn, FadeOut, SlideInDown, SlideOutDown } from 'react-native-reanimated';

import type { Draft } from '@/peer';
import { radius, space, type Theme } from '@/theme';
import { Icon } from './icon';

/** `3:45`, `225`, or `3m45s` — whatever somebody types, in milliseconds. An
 *  unreadable answer is zero rather than an error: the stream knows better
 *  anyway, and refusing to add a song over a mistyped length would be absurd. */
export function lengthMs(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  const parts = trimmed.split(':');
  if (parts.length === 2) {
    const minutes = Number(parts[0]);
    const seconds = Number(parts[1]);
    if (Number.isFinite(minutes) && Number.isFinite(seconds)) {
      return Math.max(0, Math.round((minutes * 60 + seconds) * 1000));
    }
    return 0;
  }
  const seconds = Number(trimmed);
  return Number.isFinite(seconds) ? Math.max(0, Math.round(seconds * 1000)) : 0;
}

export function Composer({
  theme,
  onAdd,
  onClose,
  bottom,
}: {
  theme: Theme;
  onAdd: (draft: Draft) => void;
  onClose: () => void;
  bottom: number;
}) {
  const [title, setTitle] = useState('');
  const [artist, setArtist] = useState('');
  const [album, setAlbum] = useState('');
  const [file, setFile] = useState('');
  const [length, setLength] = useState('');

  const s = styles(theme);
  const ready = title.trim().length > 0;

  const submit = () => {
    if (!ready) return;
    onAdd({ title, artist, album, durationMs: lengthMs(length), file });
    onClose();
  };

  const field = (
    label: string,
    value: string,
    set: (next: string) => void,
    extra?: { placeholder?: string; keyboard?: 'url' | 'numeric'; hint?: string },
  ) => (
    <View style={s.field}>
      <Text style={s.label}>{label}</Text>
      <TextInput
        style={s.input}
        value={value}
        onChangeText={set}
        placeholder={extra?.placeholder}
        placeholderTextColor={theme.faint}
        autoCapitalize={extra?.keyboard === 'url' ? 'none' : 'words'}
        autoCorrect={false}
        keyboardType={extra?.keyboard === 'numeric' ? 'numbers-and-punctuation' : 'default'}
        inputMode={extra?.keyboard === 'url' ? 'url' : undefined}
        returnKeyType="next"
      />
      {extra?.hint ? <Text style={s.hint}>{extra.hint}</Text> : null}
    </View>
  );

  return (
    <Animated.View entering={FadeIn.duration(140)} exiting={FadeOut.duration(120)} style={StyleSheet.absoluteFill}>
      <Pressable style={[StyleSheet.absoluteFill, s.scrim]} onPress={onClose} />
      <KeyboardAvoidingView
        style={s.lift}
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      >
        <Animated.View
          entering={SlideInDown.duration(240)}
          exiting={SlideOutDown.duration(180)}
          style={[s.sheet, { paddingBottom: bottom + space.lg }]}
        >
          <View style={s.head}>
            <Text style={s.heading}>Add to the library</Text>
            <Pressable onPress={onClose} hitSlop={12} accessibilityLabel="close">
              <Icon name="close" size={20} tint={theme.dim} />
            </Pressable>
          </View>

          <ScrollView keyboardShouldPersistTaps="handled" contentContainerStyle={s.fields}>
            {field('title', title, setTitle, { placeholder: 'Clair de lune' })}
            {field('artist', artist, setArtist, { placeholder: 'Claude Debussy' })}
            {field('album', album, setAlbum, { placeholder: 'Suite bergamasque' })}
            {field('file', file, setFile, {
              placeholder: 'https://… .mp3',
              keyboard: 'url',
              hint: 'A URL, or a name in the server’s media store. Without one the track is still in the library — it just cannot be streamed.',
            })}
            {field('length', length, setLength, {
              placeholder: '3:45',
              keyboard: 'numeric',
              hint: 'Optional. The stream’s own length wins once it is known.',
            })}
          </ScrollView>

          <Pressable
            onPress={submit}
            disabled={!ready}
            style={({ pressed }) => [s.add, !ready && s.addOff, pressed && ready && s.addPressed]}
          >
            <Text style={[s.addText, !ready && s.addTextOff]}>
              {ready ? 'add' : 'a song needs a title'}
            </Text>
          </Pressable>
        </Animated.View>
      </KeyboardAvoidingView>
    </Animated.View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    scrim: { backgroundColor: t.scrim },
    lift: { flex: 1, justifyContent: 'flex-end' },
    sheet: {
      backgroundColor: t.raised,
      maxHeight: '86%',
      borderTopLeftRadius: radius.xl,
      borderTopRightRadius: radius.xl,
      paddingHorizontal: space.xl,
      paddingTop: space.lg,
      gap: space.md,
      borderTopWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
    },
    head: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' },
    heading: { fontSize: 18, fontWeight: '700', color: t.text },
    fields: { gap: space.md, paddingBottom: space.sm },
    field: { gap: 5 },
    label: { fontSize: 12, fontWeight: '600', color: t.dim, textTransform: 'lowercase' },
    input: {
      backgroundColor: t.card,
      borderColor: t.border,
      borderWidth: StyleSheet.hairlineWidth,
      borderRadius: radius.md,
      paddingHorizontal: space.md,
      paddingVertical: 11,
      fontSize: 15.5,
      color: t.text,
    },
    hint: { fontSize: 11, lineHeight: 15, color: t.faint },
    add: {
      backgroundColor: t.accent,
      borderRadius: radius.pill,
      paddingVertical: 15,
      alignItems: 'center',
    },
    addPressed: { opacity: 0.85 },
    addOff: { backgroundColor: t.cardHigh },
    addText: { color: t.onAccent, fontSize: 15.5, fontWeight: '700' },
    addTextOff: { color: t.faint },
  });
