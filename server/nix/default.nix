# The sync server: the package, `nix run .#serve`, and the NixOS service.
{ inputs, ... }: {
  perSystem = { pkgs, crate, script, ... }: {
    packages = {
      # `HARKEN_WEB` points it at a browser client; the NixOS module sets it
      # to `harken-web` so both are on one port.
      harken-server = crate "harken-server" { };
      default = crate "harken-server" { };
    };

    # Pass 0.0.0.0:8787 to reach it from a phone on the same network. A dev
    # server: no provider, anyone is whoever they say — `nix run .#iced alice`
    # is a login for a name. The NixOS module below is the one that signs
    # people in for real.
    apps.serve.program = script "serve" {
      text = ''HARKEN_DEV_AUTH=1 cargo run -p harken-server -- "''${1:-127.0.0.1:8787}"'';
    };

    # sqlite for looking at a log; ffmpeg for the audio server's transcoding.
    petros.shell.packages = [ pkgs.sqlite pkgs.ffmpeg ];
  };

  # `services.harken.enable = true` and there is a music system on a port:
  # the sync socket and the browser client that talks to it, together,
  # because they are one deployment and splitting them across two ports
  # buys an origin to configure and nothing else.
  flake.nixosModules.default = { config, lib, pkgs, ... }:
    let
      cfg = config.services.harken;
      harken = inputs.self.packages.${pkgs.stdenv.hostPlatform.system};

      # A URL nothing off this machine can fetch: this machine talking to
      # itself, or a wildcard that is not an address at all.
      selfAddressed = url:
        lib.any (needle: lib.hasInfix needle url) [
          "127.0.0.1"
          "localhost"
          "::1"
          "0.0.0.0"
        ];

      # Whether what is bound answers on this machine and nowhere else.
      # `0.0.0.0` is deliberately not in here: as a *bind* it means every
      # interface, which is the opposite of what it means in a URL — which
      # is why this is its own list rather than `selfAddressed cfg.address`.
      loopbackOnly = lib.elem cfg.address [
        "127.0.0.1"
        "localhost"
        "::1"
        "[::1]"
      ];

      # What a speaker resolves a `file` against. Total rather than guarded
      # at each use, so neither the assertion's message nor the warning's
      # has to care whether `homeAssistant` is there.
      mediaBase =
        if cfg.homeAssistant == null then cfg.publicUrl
        else if cfg.homeAssistant.mediaUrl != null then cfg.homeAssistant.mediaUrl
        else cfg.publicUrl;
    in
    {
      options.services.harken = {
        enable = lib.mkEnableOption "the Harken sync server and its browser client";

        port = lib.mkOption {
          type = lib.types.port;
          default = 8787;
          description = "Port for both the sync socket and the browser client.";
        };

        address = lib.mkOption {
          type = lib.types.str;
          default = "127.0.0.1";
          example = "0.0.0.0";
          description = ''
            Address to bind. The default is loopback, for a reverse proxy in
            front that terminates TLS — the login sends a browser here and
            back, and a session token crosses on every connect, neither of
            which belongs on plain HTTP off the machine.

            {option}`services.harken.homeAssistant` is the one thing that
            pulls the other way: a speaker fetches its own bytes, so it needs
            an address of its own to reach — and naming one in `mediaUrl`
            does not bind it. Either widen this to `0.0.0.0`, or put
            something in front that forwards `mediaUrl` here. Leaving both
            undone is a warning rather than an error, because only the first
            is this option's to know about.
          '';
        };

        publicUrl = lib.mkOption {
          type = lib.types.str;
          default = "http://${cfg.address}:${toString cfg.port}";
          defaultText = lib.literalExpression ''"http://''${address}:''${toString port}"'';
          example = "https://harken.example.com";
          description = ''
            Where a browser reaches this server: the identity provider sends
            people back to `''${publicUrl}/auth/callback`, and the browser
            client served at `/` signs in against it. Behind a reverse proxy
            it is the proxy's address, and the provider must be told the
            callback under it.
          '';
        };

        oidc = lib.mkOption {
          default = null;
          description = ''
            The OpenID Connect provider people sign in through. The server
            is the only OpenID Connect client — it holds the secret and
            hands each signed-in peer a session of its own — so the
            desktop, the browser and the phone need nothing but this
            server's address. Null runs no login at all, which the server
            refuses unless {option}`services.harken.devAuth` says a laptop.
          '';
          type = lib.types.nullOr (lib.types.submodule {
            options = {
              issuer = lib.mkOption {
                type = lib.types.str;
                example = "https://auth.example.com/application/o/harken/";
                description = ''
                  The issuer URL, exactly as the provider states it: the
                  server reads `/.well-known/openid-configuration` under
                  it and checks every ID token names it.
                '';
              };
              clientId = lib.mkOption {
                type = lib.types.str;
                example = "harken";
                description = "The client id the provider knows this server as.";
              };
              clientSecretFile = lib.mkOption {
                type = lib.types.path;
                example = "/run/secrets/harken-oidc";
                description = ''
                  A file holding the client secret and nothing else. Read
                  by the service through systemd's credentials, so it can
                  live wherever the machine keeps secrets — never in the
                  store, which is world-readable.

                  It has to *exist by the time the unit starts*, and the
                  failure when it does not names nothing (see `tokenFile`
                  below). A secret under `/run` is put there by something
                  else — sops-nix, agenix — so order this unit after
                  whatever that is.
                '';
              };
              scopes = lib.mkOption {
                type = lib.types.listOf lib.types.str;
                default = [ "openid" "profile" "email" ];
                description = "What to ask the provider for. `openid` is required.";
              };
            };
          });
        };

        devAuth = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = ''
            Sign anyone in as whatever name they give, with no provider.
            What `nix run .#serve` does on a laptop; on a machine anyone
            else can reach it means anyone can write as anyone.
          '';
        };

        redirects = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
          example = [ "https://harken-web.example.com/" ];
          description = ''
            Where a login may send its code back to, besides the places
            every server allows: a loopback port (the desktop client),
            `harken://` (the phone), and {option}`services.harken.publicUrl`
            (the browser client it serves). A browser client served from
            somewhere else goes here.
          '';
        };

        web = lib.mkOption {
          type = lib.types.nullOr lib.types.package;
          default = harken.harken-web;
          defaultText = lib.literalExpression "harken.packages.\${system}.harken-web";
          description = ''
            The browser client to serve at `/`. Null serves the socket
            alone, for a deployment that only wants the phone.
          '';
        };

        homeAssistant = lib.mkOption {
          default = null;
          description = ''
            The house's media players, as devices in every listening session.

            A speaker is a *device*: it joins a session, becomes the output,
            is told things and reports what it is doing, exactly as a phone
            does. Which is why this is Home Assistant rather than Sonos — you
            do not get Sonos, you get every `media_player` entity it knows
            about, for the same six service calls.

            The whole queue is pushed to the player, so its own buttons and
            its own app keep working; where it is in that queue is then read
            back from what it says it is playing. Somebody skipping on the
            speaker moves every phone.
          '';
          example = lib.literalExpression ''
            {
              url = "http://homeassistant.local:8123";
              tokenFile = "/run/secrets/harken-ha";
              players = [ "media_player.kitchen" "media_player.study=Study" ];
              mediaUrl = "http://10.0.0.2:8787";
            }
          '';
          type = lib.types.nullOr (lib.types.submodule {
            options = {
              url = lib.mkOption {
                type = lib.types.str;
                example = "http://homeassistant.local:8123";
                description = "Where Home Assistant is.";
              };

              tokenFile = lib.mkOption {
                type = lib.types.path;
                example = "/run/secrets/harken-ha";
                description = ''
                  A file holding a long-lived access token. A file rather
                  than a string for the reason the OpenID Connect secret is
                  one: systemd hands it over as a credential, so it is never
                  in a process listing, a unit file or the store.

                  Missing when the unit starts, it takes the whole server
                  down before the binary runs, and says only

                  ```
                  harken.service: Failed to set up credentials: No such file or directory
                  harken.service: Failed at step CREDENTIALS spawning …/harken-server
                  ```

                  which names neither the credential nor the path. Both this
                  and `oidc.clientSecretFile` load the same way, so
                  `systemctl show harken -p LoadCredential` and then `ls`
                  each source is what tells them apart. Setting this is what
                  makes a speaker's token able to stop the music, so it is
                  worth knowing that is the trade.
                '';
              };

              players = lib.mkOption {
                type = lib.types.listOf lib.types.str;
                example = [ "media_player.kitchen" "media_player.study=The study" ];
                description = ''
                  Which entities to offer, and what to call them. `id=Name`
                  when the entity id is not what you would say out loud, and
                  the id alone when it is — `media_player.the_kitchen`
                  becomes "The kitchen".

                  Named rather than discovered, deliberately: a picker with
                  every `media_player` in the house in it, including the
                  television and the doorbell, is a picker nobody reads.
                '';
              };

              mediaUrl = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                example = "http://10.0.0.2:8787";
                description = ''
                  Where a *speaker* fetches bytes from, which is not
                  necessarily where a phone does: the phone may be on
                  `publicUrl` while the speaker only knows an address on the
                  LAN. Falls back to `publicUrl`.

                  Note that `/media` is served with no authentication at all,
                  which is what lets a speaker fetch at all — so this wants to
                  be an address only the house can reach.
                '';
              };
            };
          });
        };

        mediaPath = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          default = "/srv/media";
          example = "/mnt/library";
          description = ''
            The directory the library is made of, and the one `/media/`
            serves. Created on activation if it is not there, with a
            `music/` inside it.

            **Music goes in `music/` under this**, not directly in it. The
            root is kind-neutral because the `file` column is: an episode or
            a sermon becomes a sibling directory rather than a second option
            and a second URL prefix to configure. So a track at
            `''${mediaPath}/music/Bach/air.flac` is served as
            `/media/music/Bach/air.flac`, and that same relative path is
            what the log carries.

            The server walks `music/` at startup and watches the root after,
            so a file copied in appears without a rescan, and authors each
            track as an ordinary mutation — through the same `apply` every
            peer runs, so there is no second definition of what adding a
            song means. A rescan adds nothing: `apply` refuses a path the
            library already has, which makes restarting the service free
            however large the directory is.

            Moving this directory moves the media and nothing in the log has
            to change; renaming a file inside it makes a new song and leaves
            the old one pointing at nothing.

            The service reads it and never writes to it. It has to be
            readable by a dynamic user, which for most libraries means
            world-readable. Set it to `null` for a server that syncs and
            serves no files at all.
          '';
        };

        package = lib.mkOption {
          type = lib.types.package;
          default = harken.harken-server;
          defaultText = lib.literalExpression "harken.packages.\${system}.harken-server";
          description = "The server to run.";
        };

        openFirewall = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = "Open {option}`services.harken.port` in the firewall.";
        };
      };

      config = lib.mkIf cfg.enable {
        assertions = [
          {
            assertion = cfg.oidc != null || cfg.devAuth;
            message = ''
              services.harken: nobody could sign in. Set services.harken.oidc
              to an OpenID Connect provider, or services.harken.devAuth = true
              on a machine nobody else can reach.
            '';
          }
          {
            # The default `publicUrl` is loopback, because the default
            # `address` is — which is right for a reverse proxy and useless
            # to a speaker. A Sonos is a different computer, and being handed
            # `http://127.0.0.1:8787/media/…` is the failure that looks like
            # "the speaker plays nothing" and is really "the speaker fetched
            # from itself".
            #
            # Asked of `mediaBase` rather than of `publicUrl`, because the
            # question is what a speaker is *given*: this held only while
            # `mediaUrl` was unset, so setting it to a loopback address of
            # its own — the one thing no proxy in front can rescue — was the
            # exact failure the comment describes and went unchecked.
            assertion = cfg.homeAssistant == null || !(selfAddressed mediaBase);
            message = ''
              services.harken: the house's speakers would be told to fetch
              from ${mediaBase}, which is this machine talking to itself.
              Set services.harken.homeAssistant.mediaUrl to an address a
              speaker can reach.
            '';
          }
        ];

        # The assertion above checks the address a speaker is *told*; this is
        # the other half, and the half that was missing. Setting `mediaUrl`
        # satisfies it whatever the server is bound to — so a LAN address
        # beside the default loopback `address` passes evaluation, starts
        # cleanly, and hands the house a URL nothing is listening on.
        #
        # It presents as "the speaker plays nothing", and every part you
        # would check looks right: the queue lands, the speaker accepts it,
        # `media_content_id` is exactly the URL harken meant, `queue_size` is
        # the length of the hand-off — and the state is `paused`, because a
        # speaker that cannot fetch is not a speaker that refused. The tell
        # is `curl` from another machine, not from this one.
        #
        # A warning rather than an assertion, because it cannot know: a
        # reverse proxy in front of loopback is exactly how the browser half
        # of this is meant to be served, and a `mediaUrl` naming that proxy
        # is correct. What it can say is that one of the two has to be true.
        warnings =
          lib.optional
            (cfg.homeAssistant != null && loopbackOnly && !(selfAddressed mediaBase))
            ''
              services.harken: the house's speakers are told to fetch from
              ${mediaBase}, and this server is bound to ${cfg.address} —
              which answers on this machine and nowhere else. Unless
              something in front forwards ${mediaBase} to it, a speaker gets
              no bytes and sits paused holding the right queue.

              Either set services.harken.address = "0.0.0.0" (with
              openFirewall, or a firewall rule of your own) so the LAN can
              reach it, or point mediaUrl at a proxy that can.
            '';

        # Made rather than required, so the default works on a machine where
        # nobody has put anything in it yet: an empty library is a library
        # with no songs, not a service that refuses to start. Root-owned and
        # world-readable — the service reads it as a dynamic user, and
        # putting files in it is the administrator's job.
        systemd.tmpfiles.rules = lib.optionals (cfg.mediaPath != null) [
          "d ${cfg.mediaPath} 0755 root root -"
          "d ${cfg.mediaPath}/music 0755 root root -"
        ];

        systemd.services.harken = {
          description = "Harken sync server";
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];

          environment = {
            HARKEN_PUBLIC_URL = cfg.publicUrl;
            HARKEN_REDIRECTS = lib.concatStringsSep "," cfg.redirects;
          } // lib.optionalAttrs (cfg.mediaPath != null) {
            HARKEN_MEDIA = "${cfg.mediaPath}";
          } // lib.optionalAttrs (cfg.web != null) {
            HARKEN_WEB = "${cfg.web}";
          } // lib.optionalAttrs (cfg.oidc != null) {
            HARKEN_OIDC_ISSUER = cfg.oidc.issuer;
            HARKEN_OIDC_CLIENT_ID = cfg.oidc.clientId;
            HARKEN_OIDC_SCOPES = lib.concatStringsSep " " cfg.oidc.scopes;
            # systemd copies the file into a directory only this service
            # can read and says where in `CREDENTIALS_DIRECTORY`; `%d` is
            # that directory in a unit file.
            HARKEN_OIDC_CLIENT_SECRET_FILE = "%d/oidc-secret";
          } // lib.optionalAttrs cfg.devAuth {
            HARKEN_DEV_AUTH = "1";
          } // lib.optionalAttrs (cfg.homeAssistant != null) {
            HARKEN_HA_URL = cfg.homeAssistant.url;
            HARKEN_HA_TOKEN_FILE = "%d/ha-token";
            HARKEN_HA_PLAYERS = lib.concatStringsSep "," cfg.homeAssistant.players;
          } // lib.optionalAttrs
            (cfg.homeAssistant != null && cfg.homeAssistant.mediaUrl != null)
            {
              HARKEN_HA_MEDIA = cfg.homeAssistant.mediaUrl;
            };

          serviceConfig = {
            ExecStart = "${lib.getExe cfg.package} ${cfg.address}:${toString cfg.port}";
            Restart = "on-failure";
            LoadCredential =
              lib.optional (cfg.oidc != null) "oidc-secret:${cfg.oidc.clientSecretFile}"
              ++ lib.optional (cfg.homeAssistant != null)
                "ha-token:${cfg.homeAssistant.tokenFile}";

            # The log is the whole of the state, so it wants a real place
            # rather than the temp dir the demo uses. `TMPDIR` is what the
            # server reads for it, which is why this is set rather than a
            # flag: the same binary serves `nix run .#serve` and this.
            DynamicUser = true;
            StateDirectory = "harken";
            Environment = [ "TMPDIR=%S/harken" ];

            # The media, and nothing else of the filesystem. A bind mount
            # rather than `ReadOnlyPaths`, because `ProtectHome` below masks
            # `/home` outright and a library under there would simply not be
            # visible — a bind overrides that for the one directory, without
            # opening the rest. Read-only: the server indexes and serves the
            # files, and has no business changing them.
            #
            # This is also why the tmpfiles rule below is not optional. A
            # bind mount of a path that does not exist fails the unit at
            # startup, so a default nobody has created yet would mean a
            # server that will not boot until someone makes a directory.
            BindReadOnlyPaths = lib.optional (cfg.mediaPath != null) cfg.mediaPath;

            # Nothing here needs any of it.
            NoNewPrivileges = true;
            PrivateDevices = true;
            PrivateTmp = true;
            ProtectClock = true;
            ProtectControlGroups = true;
            ProtectHome = true;
            ProtectHostname = true;
            ProtectKernelLogs = true;
            ProtectKernelModules = true;
            ProtectKernelTunables = true;
            ProtectSystem = "strict";
            RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
            RestrictNamespaces = true;
            RestrictRealtime = true;
            SystemCallArchitectures = "native";
            SystemCallFilter = [ "@system-service" "~@privileged" ];
          };
        };

        networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
      };
    };
}
