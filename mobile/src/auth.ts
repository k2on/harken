/**
 * Signing in, on a phone.
 *
 * The flow is `@petros/client`'s: open the server's login page, get a code
 * back, trade it for a `Login`. The one thing only this app can say is what
 * a URL scheme to come back on looks like — `harken://auth`, from
 * `app.config.ts` — and `expo-web-browser` is what opens the page in a sheet
 * that knows to close itself when that scheme is hit.
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

/** The login this phone has for `server`, if it signed in before. */
export function remembered(server: string): Login | null {
  return recallLogin(storage, server);
}

/**
 * Sign in to `server`. Resolves to the login, or to null if the person
 * closed the sheet; throws if the server refused or could not be reached.
 */
export async function signIn(server: string): Promise<Login | null> {
  const redirect = Linking.createURL('auth');
  const result = await WebBrowser.openAuthSessionAsync(loginUrl(server, redirect), redirect);
  if (result.type !== 'success') return null;
  const code = codeOf(result.url);
  if (!code) throw new Error('the server sent no code back');
  const login = await exchange(server, code);
  rememberLogin(storage, server, login);
  return login;
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
