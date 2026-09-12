# The sync server: the package, and the NixOS service that runs it.
{ inputs, ... }: {
  perSystem = { rustPlatform, sources, ... }:
    let
      inherit (sources) workspace cargoDeps;

      # The sync server. `HARKEN_WEB` points it at a browser client; the
      # NixOS module sets it to `harken-web` so both are on one port.
      harken-server = rustPlatform.buildRustPackage {
        pname = "harken-server";
        version = "0.1.0";
        src = workspace;
        inherit cargoDeps;
        cargoBuildFlags = [ "-p" "harken-server" ];
        # The workspace's tests need the mutator module, which is a wasm
        # build with its own toolchain. `nix flake check` is not the place
        # for that; `just` is, and CI runs it.
        doCheck = false;
        meta.mainProgram = "harken-server";
      };
    in
    {
      packages = {
        inherit harken-server;
        default = harken-server;
      };
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
            Address to bind. The default is loopback, so reaching this from
            a phone means either setting this and opening the firewall, or
            — better — putting a reverse proxy in front, since the engine
            does no authentication of its own.
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
        systemd.services.harken = {
          description = "Harken sync server";
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];

          environment = lib.optionalAttrs (cfg.web != null) {
            HARKEN_WEB = "${cfg.web}";
          };

          serviceConfig = {
            ExecStart = "${lib.getExe cfg.package} ${cfg.address}:${toString cfg.port}";
            Restart = "on-failure";

            # The log is the whole of the state, so it wants a real place
            # rather than the temp dir the demo uses. `TMPDIR` is what the
            # server reads for it, which is why this is set rather than a
            # flag: the same binary serves `harken serve` and this.
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
