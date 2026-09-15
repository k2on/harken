/**
 * The palette's shape, and the hook that picks one.
 *
 * The colors themselves are in `palette.ts` beside this, generated from
 * `branding/nix/palette.nix` — the desktop client gets the same six through
 * `iced/src/palette.rs`, from the same description. What is here is the type
 * every screen reads and the hook that answers with one of the two: a literal
 * '#fff' in a component is a color chosen against one background and wrong on
 * the other.
 *
 * Two themes, picked from the system by `useColorScheme`, because
 * `app.config.ts` asks for `userInterfaceStyle: 'automatic'`.
 *
 * The accent is gold rather than the green everyone expects of a music app,
 * and the two themes do not use the *same* gold: the dark one can afford a
 * bright leaf, the light one needs a darker, browner gold or `onAccent` is
 * unreadable on it. That asymmetry is in the branding, not here.
 */
import { useMemo } from 'react';
import { useColorScheme } from 'react-native';

import { DARK, LIGHT } from './palette';

export type Theme = {
  dark: boolean;
  /** The sheet everything sits on. */
  bg: string;
  /** One step up: the now-playing bar, a sheet over the list. */
  raised: string;
  /** A card, a chip, an input. */
  card: string;
  /** …the same, pressed or selected. */
  cardHigh: string;
  border: string;
  text: string;
  /** Secondary text: an artist, an album, a count. */
  dim: string;
  /** Tertiary: a hint, a status line, a placeholder. */
  faint: string;
  /** The gold. What is playing, what is hearted, what to press. */
  accent: string;
  /** The gold at a tenth of its confidence: a chip's fill, a heart's ghost. */
  accentSoft: string;
  /** The one legible color for text *on* `accent`. */
  onAccent: string;
  danger: string;
  good: string;
  /** Behind a sheet, over the list. */
  scrim: string;
  /**
   * The three stops of the now-playing gradient, top to bottom. The last is
   * `bg`, so the artwork appears to sit in the page rather than on it.
   */
  glow: readonly [string, string, string];
  /**
   * Stand-in artwork, as gradients rather than pictures.
   *
   * Nothing in the log carries a cover: `media` has a title, a creator, a
   * length and a file, and adding a picture to it would be adding a column to
   * every kind for the sake of one. So a record's art is *derived* from its
   * name — the same album gets the same two colors on every device, because
   * the hash is of the name and nothing else — and the set is small and all of
   * one family so that a list of twenty reads as one library rather than as a
   * paint chart.
   */
  art: readonly (readonly [string, string])[];
};

/** Spacing, in the four sizes anything here actually uses. */
export const space = { xs: 4, sm: 8, md: 12, lg: 16, xl: 24 } as const;

/** Corner radii. `pill` is anything taller than it is round. */
export const radius = { sm: 8, md: 12, lg: 18, xl: 26, pill: 999 } as const;

export function palette(dark: boolean): Theme {
  return dark ? DARK : LIGHT;
}

export function useTheme(): Theme {
  const dark = useColorScheme() === 'dark';
  return useMemo(() => palette(dark), [dark]);
}
