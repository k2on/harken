/**
 * Where you say which server to join, and sign in to it.
 *
 * Whatever you said last time, because a peer that forgets its server on every
 * launch makes you retype a LAN address before you can look at your own data.
 * The login is remembered too, so signing in is once per server per phone.
 *
 * The default, the first time and only then, is guessed from whatever host
 * Metro is being served from, because that is almost always the machine
 * running `nix run .#serve` too — and a phone cannot reach that machine's
 * `127.0.0.1`. In a release build there is no Metro and the guess is worth
 * little, which is the other half of why the answer is remembered.
 *
 * Who you are is not asked here: the server says, after you sign in. Against
 * `nix run .#serve` the sheet that opens asks for a name, because that server
 * has no provider and takes your word for it.
 *
 * "Use offline" is a real answer rather than a failed connection: it opens the
 * library with no socket at all, and the pill in there links up when you want.
 * It needs a login from before, because the library is somebody's.
 */

import { useMemo, useState } from 'react';
import { router } from 'expo-router';
import Constants from 'expo-constants';
import { LinearGradient } from 'expo-linear-gradient';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import Animated, { FadeInDown } from 'react-native-reanimated';
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
import { radius, space, useTheme, type Theme } from '@/theme';
import { Icon } from '@/ui/icon';

const PORT = 8787;

function guessServer(): string {
  const fromMetro = Constants.expoConfig?.hostUri?.split(':')[0];
  const fromBrowser =
    typeof location !== 'undefined' && location.hostname ? location.hostname : undefined;
  return `http://${fromMetro ?? fromBrowser ?? 'localhost'}:${PORT}`;
}

export default function Connect() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
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
    <View style={s.page}>
      {/* The gold, once, behind the name — so the first screen says what the
          rest of the app is coloured with before anything is on it. */}
      <LinearGradient
        pointerEvents="none"
        colors={[...theme.glow]}
        locations={[0, 0.5, 1]}
        style={s.wash}
      />
      <KeyboardAvoidingView
        style={{ flex: 1 }}
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      >
        <ScrollView
          contentContainerStyle={[
            s.body,
            { paddingTop: insets.top + space.xl, paddingBottom: insets.bottom + space.xl },
          ]}
          keyboardShouldPersistTaps="handled"
        >
          <Animated.View entering={FadeInDown.duration(320)} style={s.brand}>
            <View style={s.mark}>
              <Icon name="note" size={26} tint={theme.onAccent} />
            </View>
            <Text style={s.title}>harken</Text>
            <Text style={s.blurb}>
              An offline-first peer of the same server the desktop and the browser join. Play
              something here and it is in your library there; pull the plug and it waits.
            </Text>
          </Animated.View>

          <Animated.View entering={FadeInDown.delay(80).duration(320)} style={s.card}>
            <Text style={s.label}>server</Text>
            <TextInput
              style={s.input}
              value={server}
              onChangeText={setServer}
              autoCapitalize="none"
              autoCorrect={false}
              inputMode="url"
              placeholder={`http://host:${PORT}`}
              placeholderTextColor={theme.faint}
              returnKeyType="go"
              onSubmitEditing={join}
            />
            <Text style={s.hint}>
              `nix run .#serve` listens on {PORT}. An Android emulator reaches the host as
              10.0.2.2; a real device needs the machine&apos;s address on your network.
            </Text>

            {login ? (
              <Pressable
                style={({ pressed }) => [s.button, pressed && s.muted]}
                onPress={() => go(server.trim(), true)}
              >
                <Text style={s.buttonText}>continue as {login.user.name || login.user.id}</Text>
              </Pressable>
            ) : null}

            <Pressable
              style={({ pressed }) => [
                login ? s.ghost : s.button,
                (urlProblem !== null || busy || pressed) && s.muted,
              ]}
              disabled={urlProblem !== null || busy}
              onPress={join}
            >
              <Text style={login ? s.ghostText : s.buttonText}>
                {urlProblem ?? (busy ? 'signing in…' : login ? 'sign in as someone else' : 'sign in')}
              </Text>
            </Pressable>

            {/* Not a fallback for a server that would not answer: no socket is
                opened at all, and nothing retries in the background. Edits are
                kept and offered whenever you link up, which is the pill in the
                library. */}
            {login ? (
              <Pressable
                style={({ pressed }) => [s.ghost, pressed && s.muted]}
                onPress={() => go(server.trim(), false)}
              >
                <Text style={s.ghostText}>use offline</Text>
              </Pressable>
            ) : null}

            {problem ? <Text style={s.problem}>{problem}</Text> : null}
          </Animated.View>
        </ScrollView>
      </KeyboardAvoidingView>
    </View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    // Not `absoluteFill` with a height on top of it: that is two answers to
    // where the bottom edge is, and which one wins is a platform detail.
    wash: { position: 'absolute', top: 0, left: 0, right: 0, height: '62%' },
    body: { paddingHorizontal: space.xl, gap: space.xl, flexGrow: 1, justifyContent: 'center' },
    brand: { gap: space.sm },
    mark: {
      width: 52,
      height: 52,
      borderRadius: radius.lg,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.accent,
      marginBottom: space.sm,
    },
    title: { fontSize: 42, fontWeight: '800', color: t.text, letterSpacing: -1 },
    blurb: { fontSize: 15, lineHeight: 21, color: t.dim },
    card: {
      backgroundColor: t.raised,
      borderRadius: radius.lg,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
      padding: space.lg,
      gap: space.sm,
    },
    label: { fontSize: 12, fontWeight: '700', color: t.dim },
    input: {
      backgroundColor: t.card,
      borderColor: t.border,
      borderWidth: StyleSheet.hairlineWidth,
      borderRadius: radius.md,
      paddingHorizontal: space.md,
      paddingVertical: 12,
      fontSize: 16,
      color: t.text,
    },
    hint: { fontSize: 11.5, lineHeight: 16, color: t.faint },
    button: {
      marginTop: space.md,
      backgroundColor: t.accent,
      borderRadius: radius.pill,
      paddingVertical: 15,
      alignItems: 'center',
    },
    muted: { opacity: 0.55 },
    buttonText: { color: t.onAccent, fontSize: 15.5, fontWeight: '700' },
    ghost: {
      marginTop: space.sm,
      borderColor: t.border,
      borderWidth: StyleSheet.hairlineWidth,
      borderRadius: radius.pill,
      paddingVertical: 15,
      alignItems: 'center',
    },
    ghostText: { color: t.text, fontSize: 15.5, fontWeight: '600' },
    problem: { marginTop: space.md, fontSize: 13, lineHeight: 18, color: t.danger },
  });
