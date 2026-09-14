/**
 * The square beside a track, and the big one over the player.
 *
 * There is no artwork in the log and there should not be: `media` carries a
 * title, a creator, a length and the name of a file, and a cover would be a
 * column every kind pays for so that one kind can have a picture. So this
 * draws one instead, from the name — deterministically, so the same album is
 * the same square on every device and after every reinstall, which is most of
 * what a cover is actually doing in a list.
 */

import { View } from 'react-native';
import { LinearGradient } from 'expo-linear-gradient';

import { radius, type Theme } from '@/theme';
import { Icon } from './icon';

/** FNV-1a, enough to say "this name is not that name". */
function hash(text: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i += 1) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h;
}

export function Artwork({
  seed,
  size,
  theme,
  corner = radius.sm,
}: {
  /** What the square stands for: an album's name, or a track's title. */
  seed: string;
  size: number;
  theme: Theme;
  corner?: number;
}) {
  const [from, to] = theme.art[hash(seed) % theme.art.length];
  return (
    <LinearGradient
      colors={[from, to]}
      start={{ x: 0, y: 0 }}
      end={{ x: 1, y: 1 }}
      style={{
        width: size,
        height: size,
        borderRadius: corner,
        alignItems: 'center',
        justifyContent: 'center',
        overflow: 'hidden',
      }}
    >
      {/* Faint, because it is a placeholder and not a logo: it should read as
          "a record" at a glance and disappear on a second look. */}
      <View style={{ opacity: theme.dark ? 0.45 : 0.35 }}>
        <Icon name="note" size={size * 0.38} tint={theme.text} />
      </View>
    </LinearGradient>
  );
}
