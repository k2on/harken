// Metro, taught about the sibling repository.
//
// `@petros/client` is a `file:` dependency, which bun installs as a symlink
// pointing outside this project. Metro does not watch outside `projectRoot`,
// and the failure is nasty rather than loud: the build works, the app runs, and
// edits to the library are silently never picked up — which is the entire loop
// this architecture exists for.
const path = require('path');
const { getDefaultConfig } = require('expo/metro-config');

const projectRoot = __dirname;
const config = getDefaultConfig(projectRoot);

const petrosJs = path.resolve(projectRoot, '../../../petros-js');
config.watchFolders = [petrosJs];
// Resolve from here first, then from the workspace root, so the library shares
// this app's single copy of React rather than finding a second one.
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(petrosJs, 'node_modules'),
];
config.resolver.unstable_enableSymlinks = true;

module.exports = config;
