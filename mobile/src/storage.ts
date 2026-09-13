/**
 * Somewhere for this app to keep a handful of strings.
 *
 * `@petros/client` says what is worth remembering and how to read it back; it
 * deliberately does not say where the bytes go, because there is no answer that
 * is right on a browser, a phone and a test at once. This is the phone's
 * answer.
 *
 * One JSON file in the documents directory, read once and written through.
 * `expo-file-system`'s newer API is synchronous — `textSync` and `write` — and
 * that is what makes it usable here: the connect screen needs its initial value
 * while it renders, and an async read means a frame of the wrong answer and a
 * visible correction.
 *
 * Nothing here is precious. A settings file that will not parse is a settings
 * file that does not exist, and the screen falls back to asking — which is
 * strictly better than a peer that will not start because it cannot remember
 * what it did last time.
 */

import { Directory, File, Paths } from 'expo-file-system';
import type { Storage } from '@petros/client';

const FILE = 'settings.json';

let cache: Record<string, string> | null = null;

function file(): File {
  return new File(Paths.document, FILE);
}

function all(): Record<string, string> {
  if (cache) return cache;
  try {
    const handle = file();
    if (handle.exists) {
      const parsed: unknown = JSON.parse(handle.textSync());
      // An array, a number and a null are all valid JSON and none of them is a
      // settings file.
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        cache = parsed as Record<string, string>;
        return cache;
      }
    }
  } catch {
    // Unreadable, unparseable, or a directory where a file should be. Start over.
  }
  cache = {};
  return cache;
}

function flush(): void {
  try {
    const dir = new Directory(Paths.document);
    if (!dir.exists) dir.create({ intermediates: true, idempotent: true });
    file().write(JSON.stringify(cache ?? {}));
  } catch {
    // A write that fails costs the next launch its memory and nothing else. It
    // is not worth taking a screen down for.
  }
}

export const storage: Storage = {
  get: (key) => all()[key] ?? null,
  set: (key, value) => {
    all()[key] = value;
    flush();
  },
  remove: (key) => {
    delete all()[key];
    flush();
  },
};

/** Which peer this device was last used as. The server is remembered per peer,
 *  so this is what says whose to look up. */
export const LAST_ACTOR = 'harken.actor';
