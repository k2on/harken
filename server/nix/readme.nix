# The server's part of README.md: installing it, and signing people in.
{
  perSystem.readme.sections.server = {
    order = 20;
    text = ''
      ## The server

      One axum program: the sync socket at `/sync`, the sign-in routes under
      `/auth/`, a health check at `/healthz`, and — when given one — the
      browser client at `/`. One port for all of it, so a phone or another
      laptop needs one address and no CORS.

      ```
      nix run .#serve                 # a dev server on 127.0.0.1:8787, no provider
      nix run .#serve 0.0.0.0:8787    # …reachable from a phone on the same network
      nix build .#harken-server       # the binary, at result/bin/harken-server
      ```

      ### The music

      Point it at a directory and it fills the library from it:

      ```
      HARKEN_MUSIC=/srv/music nix run .#serve
      ```

      It walks the directory at startup, watches it after — a file copied in
      appears without a rescan — and reads each track's tags rather than
      guessing them from the folder names. Every track is added as an
      ordinary mutation, through the same `apply` each peer runs, so the
      server is a peer here and not a special case.

      Rescanning is free. Adding a file the library already has is refused
      inside `apply`, so restarting the service costs nothing however large
      the directory, and two servers scanning the same share agree.

      The bytes are served from `/media/`, with range requests, so seeking
      works. What the log carries is each path *relative to the directory* —
      which is the same path `/media/` serves, so moving the directory moves
      the music and nothing in the log has to change.

      On NixOS that is {option}`services.harken.music`.

      A dev server takes your word for who you are — `nix run .#iced alice`
      is a login for a name — and says so every time it starts. It refuses to
      run that way unless told to (`HARKEN_DEV_AUTH=1`), because on a machine
      anyone else can reach it means anyone can write as anyone.

      ### On NixOS, with OpenID Connect

      The flake exports a NixOS module. Point it at a provider and it signs
      people in for real: the server is the only OpenID Connect client, holds
      the secret, and hands each signed-in peer a session of its own, so the
      desktop, the browser and the phone need nothing but this address.

      ```nix
      {
        inputs.harken.url = "github:k2on/harken";

        # in a NixOS configuration:
        imports = [ inputs.harken.nixosModules.default ];

        services.harken = {
          enable = true;
          publicUrl = "https://harken.example.com";
          oidc = {
            issuer = "https://auth.example.com/application/o/harken/";
            clientId = "harken";
            clientSecretFile = config.age.secrets.harken-oidc.path;
          };
        };

        services.nginx.virtualHosts."harken.example.com" = {
          enableACME = true;
          forceSSL = true;
          locations."/" = {
            proxyPass = "http://127.0.0.1:8787";
            proxyWebsockets = true;
          };
        };
      }
      ```

      Register `https://harken.example.com/auth/callback` as the redirect URI
      at the provider; the server derives it from `publicUrl`. The secret is a
      file and never a store path: systemd hands it to the service as a
      credential, and nothing else on the machine can read it.

      `services.harken` also takes:

      - `web` — the browser client to serve at `/`. Defaults to this flake's
        `harken-web`; `null` serves the socket alone.
      - `redirects` — extra places a login may send its code back to. Every
        server allows a loopback port (the desktop), `harken://` (the phone)
        and its own `publicUrl` (the browser client it serves).
      - `address`, `port`, `openFirewall` — the default binds loopback, for
        the reverse proxy above to terminate TLS in front of.
      - `devAuth` — the dev server's behaviour, for a machine nobody else can
        reach.

      The log and the sessions live under `/var/lib/harken`.
    '';
  };
}
