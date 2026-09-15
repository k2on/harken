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
  bg: '#050505',
  raised: '#101010',
  card: '#161615',
  cardHigh: '#201F1E',
  border: '#262624',
  text: '#FFFFFF',
  dim: '#A1A09C',
  faint: '#6B6A66',
  accent: '#E9BB45',
  accentSoft: '#2A2210',
  onAccent: '#16120A',
  danger: '#E4685C',
  good: '#5FBE88',
  scrim: 'rgba(0,0,0,0.72)',
  glow: ['#3A2E0E', '#141309', '#050505'],
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
  raised: '#FFFFFF',
  card: '#F7F6F3',
  cardHigh: '#EDEBE5',
  border: '#E2E0D9',
  text: '#0A0A09',
  dim: '#5C5A55',
  faint: '#8E8B84',
  accent: '#9A741A',
  accentSoft: '#F5EDD8',
  onAccent: '#FFFDF6',
  danger: '#B2352A',
  good: '#2E7C51',
  scrim: 'rgba(10,10,9,0.38)',
  glow: ['#F0E3BC', '#FAF7EE', '#FFFFFF'],
  art: [
    ['#E8D19A', '#C9A85F'],
    ['#EFDCAE', '#D2B370'],
    ['#E2D2AC', '#BFA671'],
    ['#F0DFA8', '#CBAA5C'],
    ['#E6D0A0', '#C3A05A'],
    ['#F3E6BE', '#D6B877'],
  ],
} as const;
