/**
 * The palette, and the only place a colour is written down.
 *
 * The desktop client asks iced for `theme.extended_palette()` and never names a
 * colour, which is the whole reason dark mode works there. React Native has no
 * such palette, so this file *is* it: every screen and every component below
 * takes its colours from here and from nowhere else, and the same rule holds —
 * a literal `'#fff'` in a component is a colour chosen against one background
 * and wrong on the other two.
 *
 * Two themes, picked from the system by `useColorScheme`, because
 * `app.config.ts` asks for `userInterfaceStyle: 'automatic'`.
 *
 * The accent is gold rather than the green everyone expects of a music app.
 * Gold is one hue that has to work on a near-black sheet, on a warm white one
 * and as the fill behind text, so the two themes do not use the *same* gold:
 * the dark one can afford a bright leaf, and the light one needs a darker,
 * browner gold or text on it is unreadable. That is the only asymmetry here,
 * and it is deliberate.
 */
import { useMemo } from 'react';
import { useColorScheme } from 'react-native';

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
  /** The one legible colour for text *on* `accent`. */
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
   * name — the same album gets the same two colours on every device, because
   * the hash is of the name and nothing else — and the set is small and all of
   * one family so that a list of twenty reads as one library rather than as a
   * paint chart.
   */
  art: readonly (readonly [string, string])[];
};

const DARK: Theme = {
  dark: true,
  bg: '#0C0B09',
  raised: '#17150F',
  card: '#1D1A14',
  cardHigh: '#2A251A',
  border: '#2C2820',
  text: '#F6F2E7',
  dim: '#A69E8C',
  faint: '#6F6857',
  accent: '#E9BB45',
  accentSoft: '#33290F',
  onAccent: '#1A1400',
  danger: '#E4685C',
  good: '#5FBE88',
  scrim: 'rgba(0,0,0,0.66)',
  glow: ['#4A3A12', '#1D1710', '#0C0B09'],
  art: [
    ['#7A5C15', '#2A2010'],
    ['#8A6B22', '#241C12'],
    ['#6B5A2A', '#1F1B12'],
    ['#8F7327', '#2C2312'],
    ['#5E4A18', '#1B160E'],
    ['#A08137', '#30260F'],
  ],
};

const LIGHT: Theme = {
  dark: false,
  bg: '#FBF8F1',
  raised: '#FFFFFF',
  card: '#FFFFFF',
  cardHigh: '#F2EADA',
  border: '#E7E0CF',
  text: '#1A1712',
  dim: '#6C6558',
  faint: '#9A9284',
  // Darker and browner than the dark theme's: `onAccent` has to be readable
  // on it, and a bright leaf gold takes no text at all.
  accent: '#9A741A',
  accentSoft: '#F5EACA',
  onAccent: '#FFFDF5',
  danger: '#B2352A',
  good: '#2E7C51',
  scrim: 'rgba(26,23,18,0.35)',
  glow: ['#F1E1B6', '#FAF3E4', '#FBF8F1'],
  art: [
    ['#E8D19A', '#C9A85F'],
    ['#EFDCAE', '#D2B370'],
    ['#E2D2AC', '#BFA671'],
    ['#F0DFA8', '#CBAA5C'],
    ['#E6D0A0', '#C3A05A'],
    ['#F3E6BE', '#D6B877'],
  ],
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
