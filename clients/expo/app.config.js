// The app's identity, decided by which build this is.
//
// `app.json` is the app; this file is what varies. A development build and
// a release build are two different apps to Android — `dev.harken.app.dev`
// beside `dev.harken.app` — so both can be installed at once, and the
// development one says so in its name and wears a banner on its icon.
//
// `APP_VARIANT` is `development` or `production`. The nix build exports it
// from the derivation's own `variant` before `expo prebuild`; EAS sets it per
// profile in `eas.json`; a bare `expo start` gets `development`, which is the
// only thing it can be pointed at anyway.
const variant = process.env.APP_VARIANT ?? 'development';
const dev = variant !== 'production';

module.exports = ({ config }) => ({
  ...config,
  name: dev ? 'Harken Dev' : 'Harken',
  android: {
    ...config.android,
    package: dev ? 'dev.harken.app.dev' : 'dev.harken.app',
    // 1024px, because the badge is drawn at that size; see the plugin.
    ...(dev
      ? { adaptiveIcon: { ...config.android.adaptiveIcon, foregroundImage: './assets/images/android-icon-foreground-1024.png' } }
      : {}),
  },
  // `ios.icon` is `assets/expo.icon`, a *directory* in Apple's layered icon
  // format, which no badge can be drawn on. A development build takes the
  // badged flat icon on iOS instead, which is what it should show anyway.
  ios: dev ? { ...config.ios, icon: undefined } : config.ios,
  plugins: [
    ...(config.plugins ?? []),
    ...(dev
      ? [
          [
            './plugins/with-dev-badge',
            { badges: [{ text: 'DEV', type: 'banner', position: 'bottom', color: 'white', background: '#E8590C' }] },
          ],
        ]
      : []),
  ],
});
