# The phone's part of README.md.
{
  perSystem.readme.sections.mobile = {
    order = 40;
    text = ''
      ## The phone

      An Expo app that calls into the same Rust, so it needs a development
      build rather than Expo Go. It signs in through the server too: enter the
      server's address, tap sign in, and the provider opens in a sheet that
      closes itself when the login comes back on `harken://`. Once per server
      per phone: after that it opens straight into the library, because both
      answers that screen wants have already been given.

      It plays, too. The library is the same one the desktop shows, browsed
      by playlist, album or artist; tapping a row starts it and the list
      becomes the queue, and every row opens a sheet of playlists to put it on
      or make another. Playback is `expo-audio` — ExoPlayer on Android,
      AVPlayer on iOS — with the transport on the lock screen, so it keeps
      going when the screen does not.

      ```
      nix build .#apk             # the development APK, toolchain and all
      nix build .#apk-release     # the release build (debug-signed; see CLAUDE.md)
      nix run .#expo-android      # the edit loop, with Metro; needs `nix develop .#android`
      ```

      Two apps can be installed side by side: `dev.harken.koon.us` from a
      development build and `harken.koon.us` from a release one.
    '';
  };
}
