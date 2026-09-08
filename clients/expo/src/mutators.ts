/**
 * Carrying the domain to the device.
 *
 * `mutators.gen.ts` is a wasm module in a base64 string, rewritten by
 * `just mutators` every time you save a Rust file. Metro treats it as an
 * ordinary module, so its fast-refresh channel — the one already pushing your
 * component edits — is what delivers a new `apply`. There is no asset pipeline
 * here and no fetch, which is why the loop is fast: the only work on this side
 * is a base64 decode and a call into wasmi.
 *
 * In a release build nothing calls `watchMutators`; the module is installed once
 * at startup from the bundle it shipped with.
 */

import type { TodoClientLike } from 'exo-todo';

import { MUTATORS_BUILD, MUTATORS_WASM_B64 } from './mutators.gen';

const CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/** Hermes has `atob`, but not on every RN version worth supporting. */
function decode(b64: string): Uint8Array {
  const clean = b64.replace(/=+$/, '');
  const out = new Uint8Array((clean.length * 3) >> 2);
  let bits = 0;
  let acc = 0;
  let n = 0;
  for (let i = 0; i < clean.length; i++) {
    acc = (acc << 6) | CHARS.indexOf(clean[i]);
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[n++] = (acc >> bits) & 0xff;
    }
  }
  return out;
}

/** Install the module this bundle carries. Returns the running generation. */
export function installMutators(client: TodoClientLike): number {
  const bytes = decode(MUTATORS_WASM_B64);
  return Number(client.loadMutators(bytes.buffer as ArrayBuffer));
}

/**
 * Reinstall whenever Metro replaces `mutators.gen.ts`.
 *
 * `module.hot` is Metro's own hot-module channel. Accepting the dependency
 * stops a mutator edit from remounting the tree: the component keeps its state
 * and its database, and only `apply` changes underneath it — which is the
 * difference between a hot reload and a restart.
 */
export function watchMutators(
  client: TodoClientLike,
  onSwap: (generation: number, build: string) => void,
): () => void {
  const hot = (module as unknown as { hot?: { accept: (dep: string[], cb: () => void) => void } })
    .hot;
  if (!hot || !__DEV__) return () => {};

  let stopped = false;
  hot.accept(['./mutators.gen'], () => {
    if (stopped) return;
    try {
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      const next = require('./mutators.gen') as typeof import('./mutators.gen');
      const bytes = decode(next.MUTATORS_WASM_B64);
      const generation = Number(client.loadMutators(bytes.buffer as ArrayBuffer));
      onSwap(generation, next.MUTATORS_BUILD);
    } catch (e) {
      // A module that will not load leaves the previous one running, which is
      // the right failure: you keep working while you fix the Rust.
      onSwap(-1, e instanceof Error ? e.message : String(e));
    }
  });
  return () => {
    stopped = true;
  };
}

export { MUTATORS_BUILD };
