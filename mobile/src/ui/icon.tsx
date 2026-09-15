/**
 * Every glyph this app draws, in one table.
 *
 * `iced/src/icon.rs` exists because the font iced embeds has no heart and no
 * transport, and a missing glyph lays out fine and draws nothing — so the
 * button looks *broken* rather than unfontable, which is a hard thing to
 * recognise as a font problem. React Native is not immune to that; it is only
 * differently exposed. `expo-symbols` draws SF Symbols on iOS and Google's
 * Material Symbols on Android, from two different names for the same idea, and
 * a name either platform does not have draws nothing at all.
 *
 * So every glyph here states both names *and* a character to fall back to.
 * `SymbolView`'s `fallback` is what renders when a platform has no symbol for
 * the name, and the characters below are ones the system font does have — the
 * point being that the worst case is a plain arrow rather than a blank square
 * where the play button should be.
 *
 * Nothing here chooses a colour. `tint` is passed in from the theme, for the
 * same reason the desktop's heart is drawn with placeholder `fill` and
 * `stroke` that the style replaces: a gold baked into the glyph is the same
 * gold on a dark row, a light one and the gold-filled one under the cursor.
 */

import { Text } from 'react-native';
import { SymbolView } from 'expo-symbols';
import type { AndroidSymbol } from 'expo-symbols';
import type { SFSymbol } from 'sf-symbols-typescript';

type Glyph = { ios: SFSymbol; android: AndroidSymbol; text: string };

const GLYPHS = {
  play: { ios: 'play.fill', android: 'play_arrow', text: '▶' },
  pause: { ios: 'pause.fill', android: 'pause', text: '❚❚' },
  next: { ios: 'forward.fill', android: 'skip_next', text: '⏭' },
  previous: { ios: 'backward.fill', android: 'skip_previous', text: '⏮' },
  heart: { ios: 'heart', android: 'favorite_border', text: '♡' },
  heartFilled: { ios: 'heart.fill', android: 'favorite', text: '♥' },
  note: { ios: 'music.note', android: 'music_note', text: '♪' },
  library: { ios: 'music.note.list', android: 'library_music', text: '≡' },
  playlist: { ios: 'list.bullet', android: 'queue_music', text: '≡' },
  album: { ios: 'square.stack', android: 'album', text: '◎' },
  artist: { ios: 'person', android: 'person', text: '☺' },
  close: { ios: 'xmark', android: 'close', text: '✕' },
  down: { ios: 'chevron.down', android: 'keyboard_arrow_down', text: '⌄' },
  online: { ios: 'wifi', android: 'cloud_done', text: '•' },
  offline: { ios: 'wifi.slash', android: 'cloud_off', text: '•' },
  signOut: { ios: 'rectangle.portrait.and.arrow.right', android: 'logout', text: '⇥' },
  playing: { ios: 'waveform', android: 'graphic_eq', text: '♪' },
  debug: { ios: 'ladybug', android: 'bug_report', text: '?' },
} as const satisfies Record<string, Glyph>;

export type IconName = keyof typeof GLYPHS;

export function Icon({
  name,
  size = 20,
  tint,
}: {
  name: IconName;
  size?: number;
  tint: string;
}) {
  const glyph: Glyph = GLYPHS[name];
  return (
    <SymbolView
      name={{ ios: glyph.ios, android: glyph.android, web: glyph.android }}
      size={size}
      tintColor={tint}
      fallback={
        <Text style={{ color: tint, fontSize: size * 0.8, lineHeight: size * 1.1 }}>
          {glyph.text}
        </Text>
      }
    />
  );
}
