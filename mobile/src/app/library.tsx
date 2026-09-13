/**
 * The library, and the hearts.
 *
 * The same library the iced window shows, because it is the same `apply` — see
 * `domain`. Take it offline with the pill, heart a few songs, heart some
 * on another peer too, come back: your favourites land *after* whatever arrived
 * while you were away, because "add to favourites" reads the end of the
 * playlist rather than naming a position. That is the rebase.
 */

import { useState } from 'react';
import { Redirect, router, Stack, useLocalSearchParams } from 'expo-router';
import type { Login } from '@petros/client';
import type { Song } from 'harken-native';
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

import { recallServer, remembered, signIn, signOut } from '@/auth';
import { usePeer } from '@/peer';
import { useTheme } from '@/theme';

export default function Library() {
  const params = useLocalSearchParams<{ server?: string; online?: string }>();
  // Arriving with no parameter at all (a deep link, a restored screen) asks
  // which server this phone was last pointed at.
  const server = params.server ?? recallServer();
  const login = server ? remembered(server) : null;
  if (!server || !login) {
    // Nobody is signed in here: the library is somebody's.
    return <Redirect href="/" />;
  }
  return <Signed server={server} login={login} online={params.online !== '0'} />;
}

function Signed(props: { server: string; login: Login; online: boolean }) {
  const { server } = props;
  // The login can change under a running peer — turned away and signed in
  // again — and the new token reconnects the same database.
  const [login, setLogin] = useState(props.login);
  // "offline" is a choice and not a failed connection: no socket at all,
  // rather than a URL nothing answers on.
  const theme = useTheme();
  const peer = usePeer(login, props.online ? server : null);
  const [title, setTitle] = useState('');
  const [artist, setArtist] = useState('');
  const [busy, setBusy] = useState(false);

  const again = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const got = await signIn(server);
      if (got) setLogin(got);
    } finally {
      setBusy(false);
    }
  };

  const leave = async () => {
    await signOut(server);
    router.replace('/');
  };

  const submit = () => {
    if (!title.trim()) return;
    peer.addSong(title, artist);
    setTitle('');
    setArtist('');
  };

  const favourites = peer.songs.filter((s) => s.favorited).length;
  const s = styles(theme);
  return (
    <KeyboardAvoidingView
      style={s.page}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      keyboardVerticalOffset={Platform.OS === 'ios' ? 90 : 0}
    >
      <Stack.Screen options={{ title: `harken · ${login.user.name || login.user.id}` }} />

      <View style={s.entry}>
        <View style={s.inputs}>
          <TextInput
            style={s.input}
            value={title}
            onChangeText={setTitle}
            placeholder="title"
            placeholderTextColor={theme.dim}
            onSubmitEditing={submit}
            returnKeyType="next"
            blurOnSubmit={false}
          />
          <TextInput
            style={s.input}
            value={artist}
            onChangeText={setArtist}
            placeholder="artist"
            placeholderTextColor={theme.dim}
            onSubmitEditing={submit}
            returnKeyType="done"
            blurOnSubmit={false}
          />
        </View>
        <Pressable style={s.add} onPress={submit}>
          <Text style={s.addText}>add</Text>
        </Pressable>
      </View>

      {/* Not a special case in the engine or in this file. It is a verb name
          the wasm module understands, reached through the one generic entry
          point — so adding it needed a module rebuild and this line, and no
          native build at all. */}
      <View style={s.verbs}>
        {peer.songs.length > favourites ? (
          <Pressable onPress={() => peer.mutate('FavoriteAll')}>
            <Text style={s.verb}>heart everything</Text>
          </Pressable>
        ) : null}
        <Pressable onPress={leave}>
          <Text style={s.verb}>sign out</Text>
        </Pressable>
      </View>

      <FlatList
        data={peer.songs}
        keyExtractor={(song) => song.id}
        contentContainerStyle={peer.songs.length === 0 && s.emptyBox}
        ListEmptyComponent={<Text style={s.empty}>nothing here yet</Text>}
        renderItem={({ item }: { item: Song }) => (
          <View style={s.row}>
            <Pressable
              onPress={() => peer.setFavorite(item.id, !item.favorited)}
              hitSlop={10}
              accessibilityRole="button"
              accessibilityLabel={item.favorited ? `unheart ${item.title}` : `heart ${item.title}`}
            >
              <Text style={[s.heart, item.favorited ? s.hearted : s.unhearted]}>♥</Text>
            </Pressable>
            <View style={s.rowText}>
              <Text style={s.title} numberOfLines={1}>
                {item.title}
              </Text>
              <Text style={s.meta} numberOfLines={1}>
                {item.artist} · {item.actor}
                {/* Its place in the playlist, recomputed from `MAX(pos) + 1` on
                    every replay — so watching it move is watching the rebase. */}
                {item.favorited ? ` · #${String(item.favoritePos)}` : ''}
              </Text>
            </View>
            <Pressable onPress={() => peer.removeSong(item.id)} hitSlop={8}>
              <Text style={s.remove}>remove</Text>
            </Pressable>
          </View>
        )}
      />

      {/* The engine showing through: `cursor` is how much of the server's log
          has been applied, `pending` is what this peer has done that no server
          has confirmed yet. */}
      <View style={s.status}>
        {/* Turned away by the server — an expired token, a revoked session.
            The database and the pending edits stay; signing in again as the
            same person offers them. Not the online pill, because knocking
            again with the same token is only refused again. */}
        {peer.denied !== null ? (
          <Pressable style={[s.pill, { backgroundColor: theme.danger }]} onPress={again}>
            <Text style={s.pillText}>{busy ? 'signing in…' : 'sign in again'}</Text>
          </Pressable>
        ) : (
          <Pressable
            style={[s.pill, { backgroundColor: peer.online ? theme.good : theme.dim }]}
            onPress={peer.toggleLink}
          >
            <Text style={s.pillText}>{peer.online ? 'online' : 'offline'}</Text>
          </Pressable>
        )}
        <Text style={s.statusText} numberOfLines={2}>
          {peer.songs.length} songs · {favourites} hearted · cursor {peer.cursor} ·{' '}
          {peer.pending} pending · mutators v{peer.mutators}
          {peer.lastMutationMs !== null ? ` · ${peer.lastMutationMs.toFixed(1)}ms in rust` : ''}
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
    inputs: { flex: 1, gap: 8 },
    input: {
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
    verbs: { flexDirection: 'row', gap: 18, paddingHorizontal: 16, paddingBottom: 10 },
    verb: { color: t.accent, fontSize: 14, fontWeight: '600' },
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
    // A character here, unlike the desktop peer: React Native draws with the
    // system font, which has U+2665. The font iced embeds does not, which is
    // why that one draws the heart as a path.
    heart: { fontSize: 22, lineHeight: 26 },
    hearted: { color: t.danger },
    unhearted: { color: t.dim },
    rowText: { flex: 1 },
    title: { fontSize: 16, color: t.text },
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
