// Metro, taught about the sibling repository.
//
// `@petros/client` is a `file:` dependency, which bun installs as a symlink
// pointing outside this project. Metro does not watch outside `projectRoot`,
// and the failure is nasty rather than loud: the build works, the app runs, and
// edits to the library are silently never picked up — which is the entire loop
// this architecture exists for.
// eas build warns that this config "does not extend @expo/metro-config" and
// offers to abort. Answer no. Its check is whether the resolved config has an
// `expo-asset/tools/hashAssetFiles` entry in `transformer.assetPlugins`, and on
// this SDK that list is empty for the *untouched default* too — checked by
// resolving `getDefaultConfig` from both `expo/metro-config` and
// `@expo/metro-config` and printing it. So the warning is about eas-cli and this
// SDK version, not about anything below.
const { getDefaultConfig } = require('expo/metro-config');

const config = getDefaultConfig(__dirname);

// `@petros/client` is a git dependency now, so it is an ordinary directory in
// node_modules and Metro needs telling nothing. It used to be a `file:` path,
// which bun installs as a symlink out of the project root — and Metro does not
// watch outside it, so edits to the library were silently never picked up.
module.exports = config;
