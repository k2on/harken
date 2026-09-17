/**
 * Every glyph this app draws — the same drawings the desktop draws.
 *
 * It used to be `expo-symbols`: SF Symbols on iOS and Google's Material
 * Symbols on Android, from two names per glyph, with a character to fall back
 * to when a platform had neither. That is three icon sets for one program —
 * four counting `iced/src/icon.rs`, which was drawing its own — and which one
 * you saw depended on what you were holding. A music app whose play button is
 * a different shape on each device is not one app with three skins; it is
 * three apps.
 *
 * So the geometry is Lucide's, vendored once in `branding/icons/` and
 * generated into `glyphs.ts` beside this and `iced/src/glyphs.rs`. This file
 * is the half that does not generalise: how big, and what colour.
 *
 * **What was given up, honestly.** SF Symbols is the native look on iOS and
 * this is not it — Apple's licence forbids using the font off Apple's
 * platforms anyway, but the *symbols* were legitimately available through
 * `expo-symbols` and are no longer used. What was bought is that the phone and
 * the desktop draw one set. The animations that made SF Symbols worth keeping
 * (`animationSpec`) were iOS-only, so Android was already getting nothing from
 * them.
 *
 * Nothing here chooses a colour. `tint` is passed in from the theme and
 * resolves the `currentColor` every drawing is written with — the same rule
 * the desktop's glyphs follow, and for the same reason: a gold baked into a
 * glyph is the same gold on a dark row, a light one and the gold-filled one
 * under the cursor.
 */

import { SvgXml } from 'react-native-svg';

import { GLYPHS, type IconName } from './glyphs';

export type { IconName };

export function Icon({ name, size = 20, tint }: { name: IconName; size?: number; tint: string }) {
  return <SvgXml xml={GLYPHS[name]} width={size} height={size} color={tint} />;
}
