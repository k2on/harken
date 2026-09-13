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
        assertions = [{
          assertion = cfg.oidc != null || cfg.devAuth;
          message = ''
            services.harken: nobody could sign in. Set services.harken.oidc
            to an OpenID Connect provider, or services.harken.devAuth = true
            on a machine nobody else can reach.
          '';
        }];

        systemd.services.harken = {
          description = "Harken sync server";
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];

          environment = {
            HARKEN_PUBLIC_URL = cfg.publicUrl;
            HARKEN_REDIRECTS = lib.concatStringsSep "," cfg.redirects;
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
          };

          serviceConfig = {
            ExecStart = "${lib.getExe cfg.package} ${cfg.address}:${toString cfg.port}";
            Restart = "on-failure";
            LoadCredential = lib.optional (cfg.oidc != null)
              "oidc-secret:${cfg.oidc.clientSecretFile}";

            # The log is the whole of the state, so it wants a real place
            # rather than the temp dir the demo uses. `TMPDIR` is what the
            # server reads for it, which is why this is set rather than a
            # flag: the same binary serves `nix run .#serve` and this.
            DynamicUser = true;
            StateDirectory = "harken";
            Environment = [ "TMPDIR=%S/harken" ];

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
