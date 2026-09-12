// A banner on the development build's icon, drawn by `app-icon-badge`.
//
// Not that package's own config plugin, and the difference is when the
// drawing happens. Its plugin starts `addBadge` and returns without waiting
// — the promise is dropped with `.catch(() => {})` and `addBadge` itself
// does not await its write — and rewrites the icon paths in the config at
// once. Expo then reads files that may be half-written, and on the iOS side
// two badges are written to the *same* path at the same time. Seen here as
// a Jimp `parseBitmap` error and, once, a prebuild that lost
// `android.package` and produced `com.harkendev`. Inside a sandboxed build
// there is no second try, so the drawing is done in a dangerous mod, which
// is allowed to be async, and each file is waited for before the config
// points at it.
//
// The adaptive foreground must be 1024px. The package's adaptive overlay
// is drawn at that size and composited at the origin, so on a 512px image
// the banner lands entirely outside the canvas and nothing appears; on
// device, Android's launcher mask then shows the bottom banner with its
// text intact, which was checked by rendering the circular mask.
const fs = require('fs');
const path = require('path');
const { withDangerousMod } = require('expo/config-plugins');
const { addBadge } = require('app-icon-badge');
const Jimp = require('jimp');

const OUT = '.expo/app-icon-badge';

async function badge(projectRoot, icon, out, badges, isAdaptiveIcon) {
  const dst = path.join(projectRoot, OUT, out);
  fs.mkdirSync(path.dirname(dst), { recursive: true });
  fs.rmSync(dst, { force: true });
  await addBadge({ icon: path.join(projectRoot, icon), dstPath: dst, badges, isAdaptiveIcon });
  // `addBadge` returns before its `writeAsync` finishes. A PNG Jimp can
  // read back is a PNG that has been fully written.
  for (let i = 0; i < 100; i++) {
    try {
      await Jimp.read(dst);
      return path.join(OUT, out);
    } catch {
      await new Promise((r) => setTimeout(r, 100));
    }
  }
  throw new Error(`with-dev-badge: ${dst} was never written`);
}

module.exports = (config, { badges }) =>
  withDangerousMod(config, [
    'android',
    async (config) => {
      const root = config.modRequest.projectRoot;
      if (config.icon) {
        config.icon = await badge(root, config.icon, 'icon.png', badges, false);
      }
      const fg = config.android?.adaptiveIcon?.foregroundImage;
      if (fg) {
        config.android.adaptiveIcon.foregroundImage = await badge(root, fg, 'foreground.png', badges, true);
      }
      return config;
    },
  ]);
