/**
 * Signing in, on a phone.
 *
 * The flow is `@petros/client`'s: open the server's login page, get a code
 * back, trade it for a `Login`. The one thing only this app can say is what
 * a URL scheme to come back on looks like — `harken://auth`, from
 * `app.config.ts` — and `expo-web-browser` is what opens the page in a sheet
 * that knows to close itself when that scheme is hit.
 *
 * It is not the only thing that knows: a URL with this app's scheme is a deep
 * link, so the OS hands it to the app as well, and `expo-router` navigates to
 * whatever route it names. `harken://auth` therefore has to *be* a route —
 * `src/app/auth.tsx` — or the sign-in lands on the unmatched-route screen,
 * which reads exactly like "the auth page does not exist". That route calls
 * `offer` below, so whichever of the two sees the redirect first, the sign-in
 * waiting here is the one that finishes it.
 *
 * Against `nix run .#serve` the sheet shows a text box asking for a name,
 * because that server has no provider and says so. Against a real one it
 * shows the provider.
 */

import * as Linking from 'expo-linking';
import * as WebBrowser from 'expo-web-browser';
import {
  codeOf,
  exchange,
  forgetLogin,
  loginUrl,
  logout,
  recallLogin,
  rememberLogin,
  type Login,
} from '@petros/client';

import { storage } from './storage';

/** Where the server was last, before anyone is known. */
const LAST_SERVER = 'harken.server';

export function recallServer(): string | null {
  return storage.get(LAST_SERVER);
}

export function rememberServer(server: string): void {
  storage.set(LAST_SERVER, server);
}

/** Whether this phone was told to work alone, from the connect screen.
 *
 *  Remembered rather than carried as a route parameter, because it is an
 *  answer about this install and not about a navigation: it used to ride on
 *  `/library?online=0`, which meant every screen that wanted to know had to be
 *  reached through that link, and a relaunch silently went back online. */
const OFFLINE = 'harken.offline';

export function offline(): boolean {
  return storage.get(OFFLINE) === '1';
}

export function setOffline(alone: boolean): void {
  storage.set(OFFLINE, alone ? '1' : '0');
}

/** The login this phone has for `server`, if it signed in before. */
export function remembered(server: string): Login | null {
  return recallLogin(storage, server);
}

/** A sign-in that has opened the sheet and is waiting for the code. */
let waiting: ((code: string) => void) | null = null;

/**
 * The code, as the `auth` route received it from the OS.
 *
 * Returns whether a sign-in was waiting for it. If none was, this phone was
 * launched by the link rather than sent out by the sheet — the app was killed
 * while the browser was open — and the route finishes the exchange itself.
 */
export function offer(code: string): boolean {
  if (!waiting) return false;
  const resume = waiting;
  waiting = null;
  resume(code);
  return true;
}

/**
 * Sign in to `server`. Resolves to the login, or to null if the person
 * closed the sheet; throws if the server refused or could not be reached.
 */
export async function signIn(server: string): Promise<Login | null> {
  // Before the sheet rather than after it, so the route can recall where a
  // code came from even if this sign-in never gets to resolve.
  rememberServer(server);
  const redirect = Linking.createURL('auth');
  const routed = new Promise<string>((resolve) => {
    waiting = resolve;
  });
  try {
    const sheet = WebBrowser.openAuthSessionAsync(loginUrl(server, redirect), redirect).then((r) =>
      r.type === 'success' ? codeOf(r.url) : null,
    );
    // Whichever arrives first. The sheet closing with no answer is not yet a
    // refusal — on Android it can close while the OS is still delivering the
    // link to the app — so a null from it waits a moment for the route.
    const code = (await Promise.race([sheet, routed])) ?? (await Promise.race([routed, grace()]));
    if (!code) return null;
    return await finish(server, code);
  } finally {
    waiting = null;
  }
}

/** The code, for the login it stands for, remembered here. Once. */
export async function finish(server: string, code: string): Promise<Login> {
  const login = await exchange(server, code);
  rememberLogin(storage, server, login);
  return login;
}

/** Long enough for a delivered deep link, short enough to feel like a close. */
function grace(): Promise<null> {
  return new Promise((resolve) => setTimeout(() => resolve(null), 500));
}

/** Sign out of `server`: tell it, and forget the login here either way. */
export async function signOut(server: string): Promise<void> {
  const login = recallLogin(storage, server);
  forgetLogin(storage, server);
  if (login) {
    try {
      await logout(server, login.token);
    } catch {
      // The server will expire it; what mattered was forgetting it here.
    }
  }
}
