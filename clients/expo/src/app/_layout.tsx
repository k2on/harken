import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';

import { useTheme } from '@/theme';

export default function RootLayout() {
  const theme = useTheme();
  return (
    <>
      <StatusBar style={theme.dark ? 'light' : 'dark'} />
      <Stack
        screenOptions={{
          headerStyle: { backgroundColor: theme.card },
          headerTintColor: theme.text,
          contentStyle: { backgroundColor: theme.bg },
        }}
      >
        <Stack.Screen name="index" options={{ title: 'petros · connect' }} />
        <Stack.Screen name="todo" options={{ title: 'petros · to-do' }} />
      </Stack>
    </>
  );
}
