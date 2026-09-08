/**
 * The list. The same to-do list the TUI and the iced window show, because it is
 * the same `apply` — see `crates/todo`.
 *
 * Take it offline with the button, add something, add something in another peer
 * too, come back: your item slides down the list as the entries the server
 * ordered ahead of it land underneath. That is the rebase.
 */

import { useState } from 'react';
import { Stack, useLocalSearchParams } from 'expo-router';
import type { TodoItem } from 'exo-todo';
import {
  FlatList,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';

import { usePeer } from '@/peer';
import { useTheme } from '@/theme';

export default function Todo() {
  const params = useLocalSearchParams<{ user?: string; server?: string }>();
  const user = params.user ?? 'phone';
  const server = params.server ?? 'ws://localhost:8787';

  const theme = useTheme();
  const peer = usePeer(user, server);
  const [draft, setDraft] = useState('');

  const submit = () => {
    const text = draft;
    setDraft('');
    peer.add(text);
  };

  const s = styles(theme);
  return (
    <KeyboardAvoidingView
      style={s.page}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      keyboardVerticalOffset={Platform.OS === 'ios' ? 90 : 0}
    >
      <Stack.Screen options={{ title: `exo · ${user}` }} />

      <View style={s.entry}>
        <TextInput
          style={s.input}
          value={draft}
          onChangeText={setDraft}
          placeholder="a new to-do…"
          placeholderTextColor={theme.dim}
          onSubmitEditing={submit}
          returnKeyType="done"
          blurOnSubmit={false}
        />
        <Pressable style={s.add} onPress={submit}>
          <Text style={s.addText}>add</Text>
        </Pressable>
      </View>

      <FlatList
        data={peer.items}
        keyExtractor={(item) => item.id}
        contentContainerStyle={peer.items.length === 0 && s.emptyBox}
        ListEmptyComponent={<Text style={s.empty}>nothing here yet</Text>}
        renderItem={({ item }: { item: TodoItem }) => (
          <View style={s.row}>
            <Pressable
              style={[s.check, item.done && s.checked]}
              onPress={() => peer.setDone(item.id, !item.done)}
              hitSlop={8}
            >
              {item.done ? <Text style={s.tick}>✓</Text> : null}
            </Pressable>
            <View style={s.rowText}>
              <Text style={[s.text, item.done && s.done]} numberOfLines={2}>
                {item.text}
              </Text>
              {/* `pos` is recomputed from `MAX(pos) + 1` on every replay, so
                  watching it change is watching the rebase happen. */}
              <Text style={s.meta}>
                {item.actor} · pos {String(item.pos)}
              </Text>
            </View>
            <Pressable onPress={() => peer.remove(item.id)} hitSlop={8}>
              <Text style={s.remove}>remove</Text>
            </Pressable>
          </View>
        )}
      />

      {/* The engine showing through: `cursor` is how much of the server's log
          has been applied, `pending` is what this peer has done that no server
          has confirmed yet. */}
      <View style={s.status}>
        <Pressable
          style={[s.pill, { backgroundColor: peer.online ? theme.good : theme.dim }]}
          onPress={peer.toggleLink}
        >
          <Text style={s.pillText}>{peer.online ? 'online' : 'offline'}</Text>
        </Pressable>
        <Text style={s.statusText} numberOfLines={2}>
          cursor {peer.cursor} · {peer.pending} pending · mutators v{peer.mutators}
          {peer.note ? `  ·  ${peer.note}` : ''}
        </Text>
      </View>
    </KeyboardAvoidingView>
  );
}

const styles = (t: ReturnType<typeof useTheme>) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    entry: { flexDirection: 'row', gap: 10, padding: 16, alignItems: 'center' },
    input: {
      flex: 1,
      backgroundColor: t.card,
      borderColor: t.border,
      borderWidth: 1,
      borderRadius: 10,
      paddingHorizontal: 14,
      paddingVertical: 11,
      fontSize: 16,
      color: t.text,
    },
    add: {
      backgroundColor: t.accent,
      borderRadius: 10,
      paddingHorizontal: 18,
      paddingVertical: 12,
    },
    addText: { color: '#fff', fontWeight: '600' },
    row: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: 12,
      backgroundColor: t.card,
      borderColor: t.border,
      borderTopWidth: StyleSheet.hairlineWidth,
      paddingHorizontal: 16,
      paddingVertical: 12,
    },
    check: {
      width: 22,
      height: 22,
      borderRadius: 6,
      borderWidth: 2,
      borderColor: t.dim,
      alignItems: 'center',
      justifyContent: 'center',
    },
    checked: { backgroundColor: t.good, borderColor: t.good },
    tick: { color: '#fff', fontSize: 14, lineHeight: 18, fontWeight: '700' },
    rowText: { flex: 1 },
    text: { fontSize: 16, color: t.text },
    done: { textDecorationLine: 'line-through', color: t.dim },
    meta: { fontSize: 11, color: t.dim, marginTop: 2 },
    remove: { color: t.danger, fontSize: 14 },
    emptyBox: { flexGrow: 1, alignItems: 'center', justifyContent: 'center' },
    empty: { color: t.dim },
    status: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: 10,
      padding: 14,
      borderTopColor: t.border,
      borderTopWidth: StyleSheet.hairlineWidth,
      backgroundColor: t.card,
    },
    pill: { borderRadius: 20, paddingHorizontal: 12, paddingVertical: 5 },
    pillText: { color: '#fff', fontSize: 12, fontWeight: '600' },
    statusText: { flex: 1, fontSize: 12, color: t.dim },
  });
