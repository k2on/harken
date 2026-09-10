/**
 * Where you say who you are and which server to join.
 *
 * Whatever you said last time, because a peer that forgets its server on every
 * launch makes you retype a LAN address before you can look at your own data.
 * `@petros/client` keeps that; this screen only asks.
 *
 * The default, the first time and only then, is guessed from whatever host
 * Metro is being served from, because that is almost always the machine running
 * `just serve` too — and a phone cannot reach that machine's `127.0.0.1`. In a
 * release build there is no Metro and the guess is worth little, which is the
 * other half of why the answer is remembered.
 *
 * "use offline" is a real answer rather than a failed connection: it opens the
 * library with no socket at all, and the pill in there links up when you want.
 */

import { useMemo, useState } from 'react';
import { router } from 'expo-router';
import Constants from 'expo-constants';
import { recallServer, rememberServer } from '@petros/client';
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

import { storage, LAST_ACTOR } from '@/storage';
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
  const [user, setUser] = useState(() => storage.get(LAST_ACTOR) ?? 'phone');
  // Read once, for the peer this device was last used as. It does not follow
  // the name field as you type: a server appearing and disappearing under the
  // cursor is worse than one that is occasionally the wrong guess.
  const [server, setServer] = useState(() => {
    const remembered = recallServer(storage, storage.get(LAST_ACTOR) ?? 'phone');
    // `null` is a peer that chose to work alone last time — leave the field
    // empty and let it choose again. `undefined` has never been asked.
    if (remembered === null) return '';
    return remembered ?? guessServer();
  });

  const nameProblem = user.trim() === '' ? 'a peer needs a name' : null;
  const urlProblem = useMemo(
    () =>
      /^wss?:\/\/[^\s/]+/.test(server.trim())
        ? null
        : 'the server should look like ws://host:port',
    [server],
  );
  const problem = nameProblem ?? urlProblem;

  /** Remember who, and where — including that "nowhere" was chosen. */
  const go = (target: string | null) => {
    const actor = user.trim();
    storage.set(LAST_ACTOR, actor);
    rememberServer(storage, actor, target);
    router.push({ pathname: '/library', params: { user: actor, server: target ?? '' } });
  };

  const join = () => {
    if (problem) return;
    go(server.trim());
  };

  const offline = () => {
    if (nameProblem) return;
    go(null);
  };

  const s = styles(theme);
  return (
    <KeyboardAvoidingView
      style={{ flex: 1 }}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <ScrollView contentContainerStyle={s.page} keyboardShouldPersistTaps="handled">
        <Text style={s.title}>petros</Text>
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

        {/* Not a fallback for a server that would not answer: no socket is
            opened at all, and nothing retries in the background. Edits are kept
            and offered whenever you link up, which is the pill in the library. */}
        <Pressable
          style={({ pressed }) => [s.ghost, (nameProblem !== null || pressed) && s.buttonMuted]}
          disabled={nameProblem !== null}
          onPress={offline}
        >
          <Text style={s.ghostText}>use offline</Text>
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
    ghost: {
      marginTop: 10,
      borderColor: t.border,
      borderWidth: 1,
      borderRadius: 10,
      paddingVertical: 15,
      alignItems: 'center',
    },
    ghostText: { color: t.text, fontSize: 16, fontWeight: '600' },
  });
