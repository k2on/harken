/**
 * Where the sign-in comes back to, `harken://auth?code=…`.
 *
 * It exists because a URL with this app's scheme is two things at once: the
 * answer `expo-web-browser`'s sheet is watching for, and a deep link the OS
 * hands to the app. `expo-router` navigates on the second whatever the first
 * does — so without a route of this name the sign-in lands on the unmatched
 * screen, which says the page does not exist while the exchange is quietly
 * succeeding behind it.
 *
 * So this screen is mostly a place to stand for the moment the code is in
 * flight. Usually a `signIn` is already waiting for it and takes it, and this
 * pops itself. When nothing is waiting — the app was killed while the browser
 * was open, and the link launched it — there is no sign-in to hand the code
 * to, so it is traded here, against the server this phone was last pointed at.
 */

import { useEffect, useRef, useState } from 'react';
import { router, useLocalSearchParams } from 'expo-router';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';

import { finish, offer, recallServer } from '@/auth';
import { FONT, radius, space, useTheme, type Theme } from '@/theme';

export default function Auth() {
  const theme = useTheme();
  const { code, error } = useLocalSearchParams<{ code?: string; error?: string }>();
  const [failed, setFailed] = useState<string | null>(null);
  // Which server the code is for, when nothing is waiting to say: `signIn`
  // writes it before it opens the sheet, for exactly this.
  const server = recallServer();
  // Once: React mounts an effect twice in development, and a code is
  // single-use — a second exchange is a refusal, not a second login.
  const done = useRef(false);

  useEffect(() => {
    if (done.current || !code) return;
    done.current = true;
    // A waiting sign-in takes the code and knows where to go next, so all
    // that is left here is to get out of its way.
    if (offer(code)) {
      if (router.canGoBack()) router.back();
      return;
    }
    if (!server) return;
    finish(server, code)
      .then(() => router.replace('/home'))
      .catch((e) => setFailed(e instanceof Error ? e.message : String(e)));
  }, [code, server]);

  const problem =
    failed ??
    error ??
    (!code
      ? 'the server sent no code back'
      : !server
        ? 'this phone does not remember which server that was'
        : null);

  const s = styles(theme);
  return (
    <View style={s.page}>
      {problem ? (
        <>
          <Text style={s.title}>that sign-in did not finish</Text>
          <Text style={s.detail}>{problem}</Text>
          <Pressable
            style={({ pressed }) => [s.button, pressed && s.muted]}
            onPress={() => router.replace('/')}
          >
            <Text style={s.buttonText}>try again</Text>
          </Pressable>
        </>
      ) : (
        <>
          <ActivityIndicator color={theme.accent} />
          <Text style={s.detail}>signing in…</Text>
        </>
      )}
    </View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    page: {
      flex: 1,
      backgroundColor: t.bg,
      alignItems: 'center',
      justifyContent: 'center',
      gap: space.md,
      paddingHorizontal: space.xl,
    },
    title: { fontFamily: FONT, fontSize: 19, fontWeight: '700', color: t.text, textAlign: 'center' },
    detail: { fontFamily: FONT, fontSize: 14, lineHeight: 20, color: t.dim, textAlign: 'center' },
    button: {
      marginTop: space.sm,
      backgroundColor: t.accent,
      borderRadius: radius.pill,
      paddingVertical: 14,
      paddingHorizontal: space.xl,
    },
    muted: { opacity: 0.55 },
    buttonText: { color: t.onAccent, fontFamily: FONT, fontSize: 15.5, fontWeight: '700' },
  });
