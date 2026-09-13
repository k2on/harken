/**
 * Where you say which server to join, and sign in to it.
 *
 * Whatever you said last time, because a peer that forgets its server on every
 * launch makes you retype a LAN address before you can look at your own data.
 * The login is remembered too, so signing in is once per server per phone.
 *
 * The default, the first time and only then, is guessed from whatever host
 * Metro is being served from, because that is almost always the machine running
 * `nix run .#serve` too — and a phone cannot reach that machine's `127.0.0.1`. In a
 * release build there is no Metro and the guess is worth little, which is the
 * other half of why the answer is remembered.
 *
 * Who you are is not asked here: the server says, after you sign in. Against
 * `nix run .#serve` the sheet that opens asks for a name, because that server
 * has no provider and takes your word for it.
 *
 * "use offline" is a real answer rather than a failed connection: it opens the
 * library with no socket at all, and the pill in there links up when you want.
 * It needs a login from before, because the library is somebody's.
 */

import { useMemo, useState } from 'react';
import { router } from 'expo-router';
import Constants from 'expo-constants';
import type { Login } from '@petros/client';
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

import { recallServer, remembered, rememberServer, signIn } from '@/auth';
import { useTheme } from '@/theme';

const PORT = 8787;

function guessServer(): string {
  const fromMetro = Constants.expoConfig?.hostUri?.split(':')[0];
  const fromBrowser =
    typeof location !== 'undefined' && location.hostname ? location.hostname : undefined;
  return `http://${fromMetro ?? fromBrowser ?? 'localhost'}:${PORT}`;
}

export default function Connect() {
  const theme = useTheme();
  const [server, setServer] = useState(() => recallServer() ?? guessServer());
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const urlProblem = useMemo(
    () =>
      /^https?:\/\/[^\s/]+/.test(server.trim())
        ? null
        : 'the server should look like http://host:port',
    [server],
  );
  // Read as you type, so the button says who you would continue as.
  const login: Login | null = useMemo(
    () => (urlProblem ? null : remembered(server.trim())),
    [server, urlProblem],
  );

  const go = (target: string, online: boolean) => {
    rememberServer(target);
    router.push({ pathname: '/library', params: { server: target, online: online ? '1' : '0' } });
  };

  const join = async () => {
    if (urlProblem || busy) return;
    const target = server.trim();
    setBusy(true);
    setProblem(null);
    try {
      const got = await signIn(target);
      if (got) go(target, true);
    } catch (e) {
      setProblem(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const s = styles(theme);
  return (
    <KeyboardAvoidingView
      style={{ flex: 1 }}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <ScrollView contentContainerStyle={s.page} keyboardShouldPersistTaps="handled">
        <Text style={s.title}>harken</Text>
        <Text style={s.blurb}>
          An offline-first peer of the same server the desktop and the browser join. Add
          something here and it appears there; pull the plug and it waits.
        </Text>

        <Text style={s.label}>server</Text>
        <TextInput
          style={s.input}
          value={server}
          onChangeText={setServer}
          autoCapitalize="none"
          autoCorrect={false}
          keyboardType="url"
          placeholder={`http://host:${PORT}`}
          placeholderTextColor={theme.dim}
          returnKeyType="go"
          onSubmitEditing={join}
        />
        <Text style={s.hint}>
          `nix run .#serve` listens on {PORT}. An Android emulator reaches the host as 10.0.2.2; a real
          device needs the machine&apos;s address on your network.
        </Text>

        {login ? (
          <Pressable
            style={({ pressed }) => [s.button, pressed && s.buttonMuted]}
            onPress={() => go(server.trim(), true)}
          >
            <Text style={s.buttonText}>continue as {login.user.name || login.user.id}</Text>
          </Pressable>
        ) : null}

        <Pressable
          style={({ pressed }) => [
            login ? s.ghost : s.button,
            (urlProblem !== null || busy || pressed) && s.buttonMuted,
          ]}
          disabled={urlProblem !== null || busy}
          onPress={join}
        >
          <Text style={login ? s.ghostText : s.buttonText}>
            {urlProblem ?? (busy ? 'signing in…' : login ? 'sign in as someone else' : 'sign in')}
          </Text>
        </Pressable>

        {/* Not a fallback for a server that would not answer: no socket is
            opened at all, and nothing retries in the background. Edits are kept
            and offered whenever you link up, which is the pill in the library. */}
        {login ? (
          <Pressable
            style={({ pressed }) => [s.ghost, pressed && s.buttonMuted]}
            onPress={() => go(server.trim(), false)}
          >
            <Text style={s.ghostText}>use offline</Text>
          </Pressable>
        ) : null}

        {problem ? <Text style={s.problem}>{problem}</Text> : null}
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
    problem: { marginTop: 12, fontSize: 13, lineHeight: 18, color: t.accent },
  });
