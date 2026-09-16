/**
 * The palette, generated from `branding/nix/palette.nix`.
 *
 * Do not edit: `nix run .#write-files` writes this and `nix flake
 * check` fails while it differs. `theme.ts` beside it is the hook
 * and the type; this is only the colors, and the desktop client
 * gets the same ones as `iced/src/palette.rs`.
 */
export const DARK = {
  dark: true,
  bg: '#1E1E1E',
  raised: '#282828',
  card: '#323232',
  cardHigh: '#464646',
  border: '#3A3A3A',
  text: '#DDDDDD',
  dim: '#9A9A9A',
  faint: '#565656',
  accent: '#E9BB45',
  accentSoft: '#433A25',
  onAccent: '#16120A',
  danger: '#FF453A',
  good: '#32D74B',
  scrim: 'rgba(0,0,0,0.55)',
  glow: ['#514528', '#2E2B21', '#1E1E1E'],
  art: [
    ['#7A5C15', '#201907'],
    ['#8A6B22', '#1B1509'],
    ['#6B5A2A', '#17140B'],
    ['#8F7327', '#221B09'],
    ['#5E4A18', '#141007'],
    ['#A08137', '#261E08'],
  ],
} as const;

export const LIGHT = {
  dark: false,
  bg: '#FFFFFF',
  raised: '#F4F5F5',
  card: '#ECECEC',
  cardHigh: '#DCDCDC',
  border: '#E5E5E5',
  text: '#262626',
  dim: '#808080',
  faint: '#BDBDBD',
  accent: '#9A741A',
  accentSoft: '#F0EADD',
  onAccent: '#FFFDF6',
  danger: '#FF3B30',
  good: '#28CD41',
  scrim: 'rgba(0,0,0,0.35)',
  glow: ['#E6DCC6', '#F7F4ED', '#FFFFFF'],
  art: [
    ['#E8D19A', '#C9A85F'],
    ['#EFDCAE', '#D2B370'],
    ['#E2D2AC', '#BFA671'],
    ['#F0DFA8', '#CBAA5C'],
    ['#E6D0A0', '#C3A05A'],
    ['#F3E6BE', '#D6B877'],
  ],
} as const;
