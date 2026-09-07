/** One small palette, so the two screens agree without a theming library. */
import { useColorScheme } from 'react-native';

export type Theme = ReturnType<typeof useTheme>;

export function useTheme() {
  const dark = useColorScheme() === 'dark';
  return {
    dark,
    bg: dark ? '#16181d' : '#f6f7f9',
    card: dark ? '#1f232b' : '#ffffff',
    border: dark ? '#2e3440' : '#e2e5ea',
    text: dark ? '#e8eaee' : '#1a1c20',
    dim: dark ? '#8b93a1' : '#6b7280',
    accent: '#5b6ee1',
    danger: '#d05353',
    good: '#3f9d5a',
  };
}
