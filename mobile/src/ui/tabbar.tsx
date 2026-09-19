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

import { useRef } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { router, usePathname } from 'expo-router';

import { FONT, space, type Theme, sheet } from '@/theme';
import { Icon, type IconName } from './icon';

const TABS: { href: string; label: string; icon: IconName }[] = [
  { href: '/home', label: 'Home', icon: 'home' },
  { href: '/search', label: 'Search', icon: 'search' },
  { href: '/library', label: 'Your Library', icon: 'library' },
];

/** How tall the bar's *content* is. The safe area is added to it rather than
 *  taken out of it — see the note on `bar` below — so a list that wants to end
 *  above the whole thing asks for `TAB_BAR + insets.bottom`. */
export const TAB_BAR = 52;

export function TabBar({ theme, bottom }: { theme: Theme; bottom: number }) {
  const path = usePathname();
  const s = styles(theme);
  // Which tab is lit when the screen is a *push* — an album reached from Your
  // Library is not `/library`, and a bar with nothing lit on it reads as a bar
  // that has lost track of where you are. So the last tab actually visited is
  // remembered, which is the honest answer to "where am I" and what every
  // phone app does. A ref rather than state: it is read during the same render
  // that writes it and nothing should re-render because of it.
  const lit = useRef(TABS[0].href);
  if (TABS.some((tab) => tab.href === path)) lit.current = path;

  return (
    // The inset is *added* to the height rather than padded out of it. Written
    // as `height: TAB_BAR` with `paddingBottom: bottom`, a phone with a 34px
    // home indicator leaves the icons and their labels 10px to live in — which
    // is the bar arriving cut off along the bottom, on exactly the devices
    // nobody tests on first.
    <View style={[s.bar, { height: TAB_BAR + bottom, paddingBottom: bottom }]}>
      {TABS.map((tab) => {
        const here = lit.current === tab.href;
        return (
          <Pressable
            key={tab.href}
            style={s.tab}
            // No animation: a tab is a sideways move between three peers, and
            // the `slide_from_right` a push earns would say one had come out
            // of the other. `_layout.tsx` gives these three screens
            // `animation: 'none'`, because the *incoming* screen's options are
            // what a replace is drawn with.
            onPress={() => {
              if (path === tab.href) return;
              router.replace(tab.href);
            }}
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

const styles = sheet((t: Theme) =>
  StyleSheet.create({
    bar: {
      flexDirection: 'row',
      alignItems: 'flex-start',
      backgroundColor: t.bg,
      paddingTop: space.sm,
      // Nothing above it: the player bar has its own ground, and two rules a
      // few pixels apart is a seam.
    },
    tab: { flex: 1, alignItems: 'center', gap: 3 },
    label: { fontFamily: FONT, fontSize: 10.5, color: t.faint },
    labelOn: { color: t.text, fontWeight: '600' },
  }),
);
