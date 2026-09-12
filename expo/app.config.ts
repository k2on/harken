// The app, and which of the two it is.
//
// A development build and a release build are different apps to Android —
// `dev.harken.koon.us` beside `harken.koon.us` — so both can be installed
// at once, and the development one says so in its name and wears a banner
// on its icon.
//
// `APP_VARIANT` is `development` or `production`. The nix build exports it
// from the APK derivation's own `variant` before `expo prebuild`; EAS sets it
// per profile in `eas.json`; a bare `expo start` gets `development`, which is
// the only thing it can be pointed at anyway.
import type { ExpoConfig } from 'expo/config';
import type { Badge } from 'app-icon-badge/types';
import withDevBadge from './plugins/with-dev-badge';

type Variant = 'development' | 'production';

const variant: Variant = process.env.APP_VARIANT === 'production' ? 'production' : 'development';
const dev = variant === 'development';

const identity = {
  development: { name: 'Harken Dev', package: 'dev.harken.koon.us' },
  production: { name: 'Harken', package: 'harken.koon.us' },
} as const satisfies Record<Variant, { name: string; package: string }>;

const badges: Badge[] = [
  { text: 'DEV', type: 'banner', position: 'bottom', color: 'white', background: '#E8590C' },
];

const config: ExpoConfig = {
  name: identity[variant].name,
  slug: 'exo-expo',
  version: '1.0.0',
  orientation: 'portrait',
  icon: './assets/images/icon.png',
  scheme: 'harken',
  userInterfaceStyle: 'automatic',
  // `assets/expo.icon` is a directory in Apple's layered icon format, which
  // no badge can be drawn on. The development build takes the badged flat
  // icon instead, which is what it should show anyway.
  ios: dev ? {} : { icon: './assets/expo.icon' },
  android: {
    package: identity[variant].package,
    adaptiveIcon: {
      backgroundColor: '#E6F4FE',
      // 1024px for the development build, because the banner is drawn at
      // that size; see the plugin.
      foregroundImage: dev
        ? './assets/images/android-icon-foreground-1024.png'
        : './assets/images/android-icon-foreground.png',
      backgroundImage: './assets/images/android-icon-background.png',
      monochromeImage: './assets/images/android-icon-monochrome.png',
    },
    predictiveBackGestureEnabled: false,
  },
  web: {
    output: 'static',
    favicon: './assets/images/favicon.png',
  },
  plugins: [
    'expo-router',
    [
      'expo-splash-screen',
      { backgroundColor: '#208AEF', image: './assets/images/splash-icon.png', imageWidth: 76 },
    ],
    ['expo-build-properties', { android: { usePrecompiledHeaders: true } }],
  ],
  experiments: {
    typedRoutes: true,
    reactCompiler: true,
  },
  extra: {
    eas: { projectId: '8ddfc581-ae95-4973-8702-7556ff41b220' },
    router: {},
  },
  owner: 'koon-industries',
};

// Applied as a function rather than listed in `plugins`, which `ExpoConfig`
// types as names and name-with-props pairs only. A function keeps the props
// checked against the plugin's own type.
export default dev ? withDevBadge(config, { badges }) : config;
