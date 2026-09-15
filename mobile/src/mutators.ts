/**
 * The module this bundle carries, and the channel that replaces it.
 *
 * Everything general lives in `@petros/client`. What stays here is the only
 * part that cannot: Metro needs a *static* path to the generated file, so only
 * the app can name it.
 *
 * **The module goes in before the database is opened, and that order is not a
 * preference.** Opening a peer replays: `Client::open` finishes whatever
 * confirmed entries the log is ahead on, and replaying a confirmed entry is
 * calling `apply` — which, here, is the module. Install afterwards and the
 * first sync stores entries nothing can apply; every launch after that meets
 * them at `open` and fails *there*, before the app has reached the line that
 * installs anything. The phone reports "could not open the database: no
 * mutator module is loaded", which sounds like a missing file and is an
 * ordering bug, and it never recovers on its own.
 *
 * So [`before`] installs with no peer in hand — the interpreter is the
 * process's, not the peer's — and `peer.ts` calls it inside the `open` it
 * hands the hook.
 */

import { asNumber, decodeBase64, installMutators, messageOf } from '@petros/client';
import type { PetrosClient } from '@petros/client';
// The engine's own free function: the same install, with nothing open yet.
import { installMutators as installWithoutAPeer } from 'harken-native';

import { MUTATORS_BUILD, MUTATORS_WASM_B64 } from './mutators.gen';

/** Why the bundled module would not load, if it would not. */
let failure: string | null = null;

/**
 * Install the bundled module, before anything opens a database.
 *
 * It throws nothing, because the caller is an `open` and there is nowhere to
 * put a message yet. The reason is kept instead and said by [`install`], which
 * runs against a peer and has a status line — and on an empty database that is
 * exactly the launch where a broken module has to name itself rather than
 * arriving one sync later as "could not open the database".
 */
export function before(): void {
  try {
    installWithoutAPeer(decodeBase64(MUTATORS_WASM_B64).buffer as ArrayBuffer);
    failure = null;
  } catch (e) {
    failure = messageOf(e);
  }
}

/** Install what shipped with this bundle. Returns a line worth showing. */
export function install(client: PetrosClient): string {
  if (failure !== null) return `could not install the mutators: ${failure}`;
  // Already in, from `before`. Loading the same bytes a second time is a
  // wasmi instantiation nobody needs.
  if (asNumber(client.mutatorsGeneration()) > 0) return `mutators ${MUTATORS_BUILD}`;
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
