/**
 * Where you say who you are and which server to join.
 *
 * The default server is guessed from whatever host Metro is being served from,
 * because that is almost always the machine running `just serve` too — and a
 * phone cannot reach that machine's `127.0.0.1`. It is a text field because the
 * guess is only a guess: an emulator wants `10.0.2.2`, a simulator wants
 * `localhost`, and a real device wants the LAN address.
 */

import { useMemo, useState } from 'react';
import { router } from 'expo-router';
import Constants from 'expo-constants';
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

import { useTheme } from '@/theme';

const PORT = 8787;

function guessServer(): string {
  const fromMetro = Constants.expoConfig?.hostUri?.split(':')[0];
  const fromBrowser =
    typeof location !== 'undefined' && location.hostname ? location.hostname : undefined;
  return `ws://${fromMetro ?? fromBrowser ?? 'localhost'}:${PORT}`;
}

export default function Connect() {
  const theme = useTheme();
  const [user, setUser] = useState('phone');
  const [server, setServer] = useState(guessServer);

  const problem = useMemo(() => {
    if (user.trim() === '') return 'a peer needs a name';
    if (!/^wss?:\/\/[^\s/]+/.test(server.trim())) return 'the server should look like ws://host:port';
    return null;
  }, [user, server]);

  const join = () => {
    if (problem) return;
    router.push({
      pathname: '/todo',
      params: { user: user.trim(), server: server.trim() },
    });
  };

  const s = styles(theme);
  return (
    <KeyboardAvoidingView
      style={{ flex: 1 }}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <ScrollView contentContainerStyle={s.page} keyboardShouldPersistTaps="handled">
        <Text style={s.title}>exo</Text>
        <Text style={s.blurb}>
          An offline-first peer of the same server the terminal and desktop examples join. Add
          something here and it appears there; pull the plug and it waits.
        </Text>

        <Text style={s.label}>who you are</Text>
        <TextInput
          style={s.input}
          value={user}
          onChangeText={setUser}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder="phone"
          placeholderTextColor={theme.dim}
          returnKeyType="next"
        />
        <Text style={s.hint}>
          Stamped on every row this peer authors, and it picks the database — the same thing
          `just peer alice` means.
        </Text>

        <Text style={s.label}>server</Text>
        <TextInput
          style={s.input}
          value={server}
          onChangeText={setServer}
          autoCapitalize="none"
          autoCorrect={false}
          keyboardType="url"
          placeholder={`ws://host:${PORT}`}
          placeholderTextColor={theme.dim}
          returnKeyType="go"
          onSubmitEditing={join}
        />
        <Text style={s.hint}>
          `just serve` listens on {PORT}. An Android emulator reaches the host as 10.0.2.2; a real
          device needs the machine&apos;s address on your network.
        </Text>

        <Pressable
          style={({ pressed }) => [s.button, (problem !== null || pressed) && s.buttonMuted]}
          disabled={problem !== null}
          onPress={join}
        >
          <Text style={s.buttonText}>{problem ?? 'join'}</Text>
        </Pressable>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}

const styles = (t: ReturnType<typeof useTheme>) =>
  StyleSheet.create({
    page: { padding: 24, gap: 8, flexGrow: 1, justifyContent: 'center' },
    title: { fontSize: 40, fontWeight: '700', color: t.text },
    blurb: { fontSize: 15, lineHeight: 21, color: t.dim, marginBottom: 20 },
    label: { fontSize: 13, fontWeight: '600', color: t.text, marginTop: 12 },
    input: {
      backgroundColor: t.card,
      borderColor: t.border,
      borderWidth: 1,
      borderRadius: 10,
      paddingHorizontal: 14,
      paddingVertical: 12,
      fontSize: 16,
      color: t.text,
    },
    hint: { fontSize: 12, lineHeight: 17, color: t.dim },
    button: {
      marginTop: 28,
      backgroundColor: t.accent,
      borderRadius: 10,
      paddingVertical: 15,
      alignItems: 'center',
    },
    buttonMuted: { opacity: 0.5 },
    buttonText: { color: '#fff', fontSize: 16, fontWeight: '600' },
  });
