/**
 * Home, Search, Your Library.
 *
 * Drawn rather than taken from `expo-router`'s `Tabs`, for one reason that
 * decides the whole layout: the player bar has to sit *above* this and *below*
 * everything else, on every screen including the ones pushed over the tabs. A
 * router-owned tab bar is a sibling of its screens and there is nowhere in
 * that arrangement to put a third thing between them — so the shell is one
 * `Stack` with these two drawn on top of it, and a tab is a `replace` rather
 * than a push. Which is also what makes the back gesture mean "out of the
 * album" rather than "to the tab I was on before".
 */

import { Pressable, StyleSheet, Text, View } from 'react-native';
import { router, usePathname } from 'expo-router';

import { space, type Theme } from '@/theme';
import { Icon, type IconName } from './icon';

const TABS: { href: string; label: string; icon: IconName }[] = [
  { href: '/home', label: 'Home', icon: 'home' },
  { href: '/search', label: 'Search', icon: 'search' },
  { href: '/library', label: 'Your Library', icon: 'library' },
];

/** How tall it is, so the player can sit on top of it and a list can end
 *  above it. A constant rather than a measurement because everything that has
 *  to agree about it is laid out before it is drawn. */
export const TAB_BAR = 52;

export function TabBar({ theme, bottom }: { theme: Theme; bottom: number }) {
  const path = usePathname();
  const s = styles(theme);
  return (
    <View style={[s.bar, { paddingBottom: bottom }]}>
      {TABS.map((tab) => {
        // `startsWith`, so a screen pushed from a tab keeps that tab lit —
        // which is the honest answer to "where am I", and what every phone
        // app does.
        const here = path === tab.href;
        return (
          <Pressable
            key={tab.href}
            style={s.tab}
            onPress={() => router.replace(tab.href)}
            accessibilityRole="tab"
            accessibilityState={{ selected: here }}
            accessibilityLabel={tab.label}
          >
            <Icon name={tab.icon} size={23} tint={here ? theme.text : theme.faint} />
            <Text style={[s.label, here && s.labelOn]} numberOfLines={1}>
              {tab.label}
            </Text>
          </Pressable>
        );
      })}
    </View>
  );
}

const styles = (t: Theme) =>
  StyleSheet.create({
    bar: {
      flexDirection: 'row',
      alignItems: 'flex-start',
      backgroundColor: t.bg,
      paddingTop: space.sm,
      height: TAB_BAR,
      // Nothing above it: the player bar has its own hairline, and two rules a
      // few pixels apart is a seam.
    },
    tab: { flex: 1, alignItems: 'center', gap: 3 },
    label: { fontSize: 10.5, color: t.faint },
    labelOn: { color: t.text, fontWeight: '600' },
  });
