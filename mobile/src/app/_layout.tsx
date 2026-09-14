/**
 * The shell: the three things every screen under it needs.
 *
 * `GestureHandlerRootView` because the seek bar and the player sheet are
 * gestures, and a `GestureDetector` with no root above it does nothing at all
 * — silently, which is the trap worth knowing here.
 *
 * `PlayerProvider` is at the root rather than on the library screen on
 * purpose: `useAudioPlayer` releases its player when the component holding it
 * unmounts, so an audio session that lives on a screen stops when you leave
 * it. Music does not work that way.
 */

import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';

import { PlayerProvider } from '@/player';
import { useTheme } from '@/theme';

export default function RootLayout() {
  const theme = useTheme();
  return (
    <GestureHandlerRootView style={{ flex: 1, backgroundColor: theme.bg }}>
      <SafeAreaProvider>
        <PlayerProvider>
          <StatusBar style={theme.dark ? 'light' : 'dark'} />
          {/* No headers: both screens draw their own, because a Spotify-shaped
              library wants its title to scroll with the content and a
              navigation bar cannot. */}
          <Stack
            screenOptions={{
              headerShown: false,
              contentStyle: { backgroundColor: theme.bg },
              animation: 'fade',
            }}
          />
        </PlayerProvider>
      </SafeAreaProvider>
    </GestureHandlerRootView>
  );
}
