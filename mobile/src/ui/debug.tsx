/**
 * What this peer actually thinks, on the phone that is being wrong.
 *
 * "The library is empty" and "it says offline" are the same sentence from two
 * ends, and neither says which of five things broke: no socket, a refused
 * token, a module that would not install, a server with nothing in it, or a
 * list that is fine and not being drawn. Every one of those is a number the
 * peer already holds — they were simply nowhere anybody could read them.
 *
 * So this is a reader, not a feature. The one thing it *does* is point the
 * peer somewhere else, and that is here because it was nowhere: `setServer`
 * has always been on the peer and no screen ever called it, so a phone that
 * remembered an address it can no longer reach had no way back. The connect
 * screen is not that way out — it is skipped once a login is remembered,
 * which is the whole point of remembering one.
 *
 * Nothing is copied to a clipboard: that is a dependency, and a dependency
 * here moves two hashes and a recorded Maven graph. The text is `selectable`
 * instead, which is what a clipboard was going to be for.
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
import Constants from 'expo-constants';
import { socketUrl, type Login } from '@petros/client';

import { mediaUrl } from '@/media';
import { databasePath, type Peer } from '@/peer';
import { radius, space, type Theme } from '@/theme';
import { Icon } from '@/ui/icon';

/** A time the token stops working, as something a person can act on. */
function expiry(login: Login): { text: string; bad: boolean } {
  // Not every server states one. Zero is "unsaid", not "expired in 1970" —
  // reading it the other way would condemn a perfectly good login.
  if (!login.expiresMs) return { text: 'not stated', bad: false };
  const left = login.expiresMs - Date.now();
  const when = new Date(login.expiresMs).toISOString().replace('T', ' ').slice(0, 19);
  if (left <= 0) return { text: `${when} — EXPIRED`, bad: true };
  const hours = left / 3_600_000;
  const rest = hours < 1 ? `${Math.round(left / 60_000)}m` : `${hours.toFixed(1)}h`;
  return { text: `${when} (${rest} left)`, bad: false };
}

export function Debug({
  peer,
  login,
  server,
  theme,
  onClose,
  top,
  bottom,
}: {
  peer: Peer;
  login: Login;
  server: string;
  theme: Theme;
  onClose: () => void;
  top: number;
  bottom: number;
}) {
  const [target, setTarget] = useState(server);
  const s = styles(theme);

  const first = peer.items[0];
  const token = expiry(login);
  // The two worth reading against each other. `dialling` is what the live
  // session is actually using; `this screen` is what the screen was opened
  // with. A session is a module-level singleton keyed by the user, so it
  // outlives every screen — and one opened against an address you have since
  // stopped using goes on dialling it. When these two differ that is the bug,
  // and nothing else on this sheet says so.
  const dialling = peer.server ?? '(none — working alone)';
  const wanted = socketUrl(server);
  const link = peer.denied !== null ? `DENIED: ${peer.denied}` : peer.online ? 'online' : 'offline';

  return (
    <View style={[s.sheet, { paddingTop: top + space.md }]}>
      <View style={s.bar}>
        <Text style={s.title}>debug</Text>
        <Pressable onPress={onClose} style={s.round} accessibilityLabel="close">
          <Icon name="close" size={18} tint={theme.dim} />
        </Pressable>
      </View>

      <KeyboardAvoidingView
        style={{ flex: 1 }}
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      >
        <ScrollView
          contentContainerStyle={[s.body, { paddingBottom: bottom + space.xl }]}
          keyboardShouldPersistTaps="handled"
        >
          {/* The socket first, because every number below it is downstream: a
              peer that never connected has a cursor of 0 and an empty library,
              and both of those are correct. */}
          <Section title="the socket" theme={theme}>
            <Row k="link" v={link} bad={!peer.online} theme={theme} />
            <Row k="server" v={server} theme={theme} />
            <Row k="dialling" v={dialling} bad={dialling !== wanted} theme={theme} />
            <Row k="this screen" v={wanted} bad={dialling !== wanted} theme={theme} />
            <Row k="note" v={peer.note || '(none)'} theme={theme} />
          </Section>

          {/* `cursor` is the one that separates "nothing arrived" from
              "something arrived and is not on screen". */}
          <Section title="the engine" theme={theme}>
            <Row k="cursor" v={String(peer.cursor)} bad={peer.cursor === 0} theme={theme} />
            <Row k="pending" v={String(peer.pending)} theme={theme} />
            <Row
              k="mutators"
              v={peer.mutators === 0 ? '0 — apply did NOT install' : `v${peer.mutators}`}
              bad={peer.mutators === 0}
              theme={theme}
            />
            <Row
              k="last mutation"
              v={peer.lastMutationMs === null ? '(none yet)' : `${peer.lastMutationMs.toFixed(1)}ms`}
              theme={theme}
            />
          </Section>

          <Section title="who" theme={theme}>
            <Row k="id" v={login.user.id} theme={theme} />
            <Row k="name" v={login.user.name || '(none)'} theme={theme} />
            <Row k="session" v={login.session} theme={theme} />
            <Row k="expires" v={token.text} bad={token.bad} theme={theme} />
            <Row k="database" v={databasePath(login.user.id)} theme={theme} />
          </Section>

          {/* `items` against `shown` is the whole difference between no data
              and a selection that is filtering all of it away. */}
          <Section title="the lists" theme={theme}>
            <Row
              k="items"
              v={String(peer.items.length)}
              bad={peer.items.length === 0}
              theme={theme}
            />
            <Row k="shown" v={String(peer.shown.length)} theme={theme} />
            <Row k="source" v={peer.source.kind} theme={theme} />
            <Row k="hearted" v={String(peer.onPlaylist)} theme={theme} />
            <Row
              k="hearts playlist"
              v={peer.playlist ?? '(none — createPlaylist failed)'}
              bad={peer.playlist === null}
              theme={theme}
            />
            <Row k="playlists" v={String(peer.playlists.length)} theme={theme} />
            <Row k="albums" v={String(peer.albums.length)} theme={theme} />
            <Row k="artists" v={String(peer.artists.length)} theme={theme} />
          </Section>

          {/* One row, resolved the way the player resolves it. A `file` that is
              a path and a server that is null is a track that cannot play, and
              it looks exactly like a track that can. */}
          {first ? (
            <Section title="the first track" theme={theme}>
              <Row k="title" v={first.title} theme={theme} />
              <Row k="file" v={first.file || '(empty)'} theme={theme} />
              <Row k="url" v={mediaUrl(first.file, server) ?? '(cannot resolve)'} theme={theme} />
            </Section>
          ) : null}

          <Section title="the build" theme={theme}>
            <Row k="app" v={Constants.expoConfig?.name ?? '(unknown)'} theme={theme} />
            <Row k="version" v={Constants.expoConfig?.version ?? '(unknown)'} theme={theme} />
            <Row
              k="metro"
              v={Constants.expoConfig?.hostUri ?? '(none — a release bundle)'}
              theme={theme}
            />
            <Row k="platform" v={`${Platform.OS} ${String(Platform.Version)}`} theme={theme} />
          </Section>

          {/* The one control. Everything above it is a reading. */}
          <Section title="point it somewhere else" theme={theme}>
            <TextInput
              style={s.input}
              value={target}
              onChangeText={setTarget}
              autoCapitalize="none"
              autoCorrect={false}
              inputMode="url"
              placeholder="http://host:8787"
              placeholderTextColor={theme.faint}
            />
            <View style={s.buttons}>
              <Button label="use this" theme={theme} onPress={() => peer.setServer(target.trim())} />
              <Button label="reconnect" theme={theme} onPress={peer.reconnect} />
              <Button
                label={peer.online ? 'go offline' : 'link up'}
                theme={theme}
                onPress={peer.toggleLink}
              />
              <Button label="work alone" theme={theme} onPress={() => peer.setServer(null)} />
            </View>
            <Text style={s.hint}>
              An emulator reaches the machine running `nix run .#serve` as 10.0.2.2; a real device
              needs that machine&apos;s address on your network, and the server has to have been
              started on 0.0.0.0 to answer it.
            </Text>
          </Section>
        </ScrollView>
      </KeyboardAvoidingView>
    </View>
  );
}

function Section({
  title,
  theme,
  children,
}: {
  title: string;
  theme: Theme;
  children: React.ReactNode;
}) {
  const s = styles(theme);
  return (
    <View style={s.section}>
      <Text style={s.sectionTitle}>{title}</Text>
      {children}
    </View>
  );
}

/** One reading. `bad` is the value worth looking at first, not an error. */
function Row({ k, v, bad, theme }: { k: string; v: string; bad?: boolean; theme: Theme }) {
  const s = styles(theme);
  return (
    <View style={s.row}>
      <Text style={s.key}>{k}</Text>
      <Text style={[s.value, bad && s.valueBad]} selectable>
        {v}
      </Text>
    </View>
  );
}

function Button({ label, theme, onPress }: { label: string; theme: Theme; onPress: () => void }) {
  const s = styles(theme);
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [s.button, pressed && s.muted]}
      accessibilityRole="button"
    >
      <Text style={s.buttonText}>{label}</Text>
    </Pressable>
  );
}

const mono = Platform.select({ ios: 'Menlo', android: 'monospace', default: 'monospace' });

const styles = (t: Theme) =>
  StyleSheet.create({
    // Written out rather than spread from `absoluteFill`: the edges are the
    // whole behaviour here, and one place that says where they are beats two
    // that could disagree.
    sheet: {
      position: 'absolute',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      backgroundColor: t.bg,
      zIndex: 20,
    },
    bar: {
      flexDirection: 'row',
      alignItems: 'center',
      justifyContent: 'space-between',
      paddingHorizontal: space.lg,
      paddingBottom: space.sm,
    },
    title: { fontSize: 22, fontWeight: '800', color: t.text },
    round: {
      width: 34,
      height: 34,
      borderRadius: radius.pill,
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: t.card,
    },
    body: { paddingHorizontal: space.lg, gap: space.lg },
    section: {
      backgroundColor: t.raised,
      borderRadius: radius.lg,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
      padding: space.md,
      gap: 6,
    },
    sectionTitle: { fontSize: 12, fontWeight: '800', color: t.accent, marginBottom: 2 },
    row: { flexDirection: 'row', alignItems: 'flex-start', gap: space.sm },
    key: { width: 108, fontSize: 11.5, color: t.dim, fontFamily: mono },
    // Wrapping rather than truncating: the value that matters most here is a
    // URL, and the wrong half of a truncated URL is the half you need.
    value: { flex: 1, fontSize: 11.5, color: t.text, fontFamily: mono },
    valueBad: { color: t.danger, fontWeight: '700' },
    input: {
      backgroundColor: t.card,
      borderColor: t.border,
      borderWidth: StyleSheet.hairlineWidth,
      borderRadius: radius.md,
      paddingHorizontal: space.md,
      paddingVertical: 10,
      fontSize: 14,
      color: t.text,
      marginTop: 4,
    },
    buttons: { flexDirection: 'row', flexWrap: 'wrap', gap: space.sm, marginTop: space.sm },
    button: {
      borderRadius: radius.pill,
      borderWidth: StyleSheet.hairlineWidth,
      borderColor: t.border,
      backgroundColor: t.card,
      paddingVertical: 9,
      paddingHorizontal: space.md,
    },
    muted: { opacity: 0.55 },
    buttonText: { fontSize: 13, fontWeight: '700', color: t.text },
    hint: { fontSize: 11, lineHeight: 15, color: t.faint, marginTop: space.sm },
  });
