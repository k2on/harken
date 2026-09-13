/**
 * The module this bundle carries, and the channel that replaces it.
 *
 * Everything general lives in `@petros/client`. What stays here is the only
 * part that cannot: Metro needs a *static* path to the generated file, so only
 * the app can name it.
 */

import { installMutators } from '@petros/client';
import type { PetrosClient } from '@petros/client';

import { MUTATORS_BUILD, MUTATORS_WASM_B64 } from './mutators.gen';

/** Install what shipped with this bundle. Returns a line worth showing. */
export function install(client: PetrosClient): string {
  const { error } = installMutators(client, MUTATORS_WASM_B64);
  return error ? `could not install the mutators: ${error}` : `mutators ${MUTATORS_BUILD}`;
}

/**
 * Reinstall whenever Metro replaces `mutators.gen.ts`.
 *
 * `module.hot` is Metro's own hot-module channel. Accepting the dependency
 * stops a mutator edit from remounting the tree: the component keeps its state
 * and its database, and only `apply` changes underneath it — which is the
 * difference between a hot reload and a restart.
 *
 * In a release build nothing calls this; the module is installed once at
 * startup from the bundle it shipped with.
 */
export function watch(client: PetrosClient, onSwap: (note: string) => void): () => void {
  const hot = (module as unknown as { hot?: { accept: (dep: string[], cb: () => void) => void } })
    .hot;
  if (!hot || !__DEV__) return () => {};

  let stopped = false;
  hot.accept(['./mutators.gen'], () => {
    if (stopped) return;
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const next = require('./mutators.gen') as typeof import('./mutators.gen');
    const { generation, error } = installMutators(client, next.MUTATORS_WASM_B64);
    // A module that will not load leaves the previous one running, which is the
    // right failure: you keep working while you fix the Rust.
    onSwap(error ? `the new mutators would not load: ${error}` : `mutators ${next.MUTATORS_BUILD} · gen ${generation}`);
  });
  return () => {
    stopped = true;
  };
}

export { MUTATORS_BUILD };
