/**
 * Home — deliberately empty.
 *
 * What goes here is recommendations, and this app has nothing to recommend
 * from: there is no play history in the log and no intention of putting one
 * there. So rather than a screen of placeholder shelves that would have to be
 * unpicked later, it says what it is and points at the two screens that do
 * work. When there is something honest to put here, this file is where it
 * goes.
 */

import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { router } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { FONT, radius, space, useTheme, type Theme, sheet } from '@/theme';
import { Icon } from '@/ui/icon';
import { useShell } from './_layout';

export default function Home() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { peer, login, inset } = useShell();
  const s = styles(theme);

  const hour = new Date().getHours();
  const greeting = hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening';

  return (
    <ScrollView
      style={s.page}
      contentContainerStyle={[s.inner, { paddingTop: insets.top + space.lg, paddingBottom: inset }]}
    >
      <Text style={s.title}>{greeting}</Text>
      <Text style={s.who}>{login.user.name || login.user.id}</Text>

      <View style={s.empty}>
        <Icon name="note" size={26} tint={theme.faint} />
        <Text style={s.emptyTitle}>Nothing to put here yet</Text>
        <Text style={s.emptyWhy}>
          Home is for what you might want next, and nothing in the log says what that is. Your
          library has {peer.items.length} {peer.items.length === 1 ? 'track' : 'tracks'} in it.
        </Text>
      </View>

      <View style={s.links}>
        <Link
          theme={theme}
          icon="search"
          label="Search"
          why="everything, by title or by who made it"
          onPress={() => router.replace('/search')}
        />
        <Link
          theme={theme}
          icon="library"
          label="Your Library"
          why="playlists, albums and artists"
          onPress={() => router.replace('/library')}
        />
      </View>
    </ScrollView>
  );
}

/** A row rather than a styled `Text`: a glyph nested inside `Text` is an
 *  `<Svg>` inside a text run, which iOS lays out and Android quietly drops.
 *  (It was an `expo-symbols` view before, with the same problem — the
 *  mechanism changed and the layout rule did not.) */
function Link({
  theme,
  icon,
  label,
  why,
  onPress,
}: {
  theme: Theme;
  icon: 'search' | 'library';
  label: string;
  why: string;
  onPress: () => void;
}) {
  const s = styles(theme);
  return (
    <Pressable
      onPress={onPress}
      accessibilityRole="button"
      style={({ pressed }) => [s.link, pressed && s.linkPressed]}
    >
      <Icon name={icon} size={17} tint={theme.accent} />
      <View style={s.linkText}>
        <Text style={s.linkLabel}>{label}</Text>
        <Text style={s.linkWhy}>{why}</Text>
      </View>
      <Icon name="chevron" size={15} tint={theme.faint} />
    </Pressable>
  );
}

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    page: { flex: 1, backgroundColor: t.bg },
    inner: { paddingHorizontal: space.lg, gap: space.sm },
    title: { fontFamily: FONT, fontSize: 27, fontWeight: '800', color: t.text },
    who: { fontFamily: FONT, fontSize: 13, color: t.dim, marginBottom: space.xl },
    empty: {
      alignItems: 'center',
      gap: space.sm,
      paddingVertical: space.xl,
      paddingHorizontal: space.lg,
    },
    emptyTitle: { fontFamily: FONT, fontSize: 16, fontWeight: '700', color: t.text },
    emptyWhy: { fontFamily: FONT, fontSize: 13, lineHeight: 19, color: t.dim, textAlign: 'center' },
    links: { gap: space.xs, paddingTop: space.lg },
    link: {
      flexDirection: 'row',
      alignItems: 'center',
      gap: space.md,
      paddingVertical: space.md,
      paddingHorizontal: space.sm,
      borderRadius: radius.md,
    },
    linkPressed: { backgroundColor: t.cardHigh },
    linkText: { flex: 1, gap: 1 },
    linkLabel: { fontFamily: FONT, fontSize: 15, color: t.text, fontWeight: '700' },
    linkWhy: { fontFamily: FONT, fontSize: 12.5, color: t.dim },
  }),
);
