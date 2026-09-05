{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.gathers;

  inherit (pkgs.stdenv.hostPlatform) system;

  mkDbPathOption =
    filename:
    lib.mkOption {
      type = lib.types.path;
      default = "${cfg.dataDir}/db/${filename}";
    };
in
{
  options.services.gathers = {
    enable = lib.mkEnableOption "GatheRs, a card collection manager";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${system}.gathers-api;
      description = "The GatheRs package to use.";
    };

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/gathers";
      description = ''
        State directory for GatheRs (databases, generated config).
      '';
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "gathers-api";
      description = ''
        User account under which `gathers-api` runs. Created automatically if
        the default is used.
      '';
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "gathers-api";
      description = ''
        Group under which `gathers-api` runs. Created automatically if the
        default is used.
      '';
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 5234;
      description = "The TCP port the API server listens on.";
    };

    systems = lib.mkOption {
      type = lib.types.listOf (
        lib.types.enum [
          "scryfall"
          "sql"
          "riftbound-sql"
          "pokemon-sql"
        ]
      );
      default = [
        "riftbound-sql"
        "sql"
      ];
      description = ''
        Retrieval system(s) from which to fetch card
        data.
      '';
    };

    refreshInterval = lib.mkOption {
      type = lib.types.str;
      default = "weekly";
      description = ''
        How often to refresh the reference card/pricing databases, as a
        {manpage}`systemd.time(7)` expression.
      '';
    };

    mtgDbPath = mkDbPathOption "AllPrintings.db";
    mtgPricesPath = mkDbPathOption "AllPricesToday.sqlite";
    riftboundDbPath = mkDbPathOption "riftbound.db";
    pokemonDbPath = mkDbPathOption "pokemon.db";
    pokemonPricesPath = mkDbPathOption "pokemon_prices.sqlite";
    storageDbPath = mkDbPathOption "storage.db";

    frontend = {
      enable = lib.mkEnableOption "the GatheRs web UI" // {
        default = cfg.reverseProxy != null;
        defaultText = lib.literalExpression "config.services.gathers.reverseProxy != null";
      };

      package = lib.mkOption {
        type = lib.types.package;
        default = self.packages.${system}.gathers-webui2;
        description = "The built static GatheRs web UI to serve.";
      };
    };

    reverseProxy = lib.mkOption {
      type = lib.types.nullOr (
        lib.types.enum [
          "nginx"
          "caddy"
        ]
      );
      default = null;
      description = ''
        Reverse proxy to put in front of GatheRs. Proxies `/api/` to the
        backend and, when {option}`services.gathers.frontend.enable` is set,
        serves the built web UI at `/`. Set to `null` to disable.

        Further nginx configuration can be done by adapting
        `services.nginx.virtualHosts.<name>`; further Caddy configuration, by
        adapting `services.caddy.virtualHosts.<name>`.
      '';
    };

    virtualHost = lib.mkOption {
      type = lib.types.str;
      default = "gathers";
      description = ''
        Name of the nginx/Caddy virtual host to set
        up.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.frontend.enable -> cfg.reverseProxy != null;
        message = ''
          services.gathers.frontend.enable has no effect unless
          services.gathers.reverseProxy is set to "nginx" or "caddy" to actually
          serve the built web UI.
        '';
      }
    ];

    users.users = lib.optionalAttrs (cfg.user == "gathers-api") {
      gathers-api = {
        isSystemUser = true;
        group = cfg.group;
      };
    };
    users.groups = lib.optionalAttrs (cfg.group == "gathers-api") {
      gathers-api = { };
    };

    systemd.tmpfiles.rules = [
      "d ${cfg.dataDir} 0750 ${cfg.user} ${cfg.group} -"
      "d ${cfg.dataDir}/db 0750 ${cfg.user} ${cfg.group} -"
    ];

    systemd.services.gathers-api = {
      description = "GatheRs card collection manager API";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      environment = {
        HOME = cfg.dataDir;
        MTG_DB_PATH = cfg.mtgDbPath;
        MTG_PRICES_PATH = cfg.mtgPricesPath;
        RIFTBOUND_DB_PATH = cfg.riftboundDbPath;
        POKEMON_DB_PATH = cfg.pokemonDbPath;
        POKEMON_PRICES_PATH = cfg.pokemonPricesPath;
        STORAGE_DB_PATH = cfg.storageDbPath;
      };
      serviceConfig = {
        Type = "simple";
        User = cfg.user;
        Group = cfg.group;
        ExecStart = "${cfg.package}/bin/server --port ${toString cfg.port} ${
          lib.concatMapStringsSep " " (s: "--system ${s}") cfg.systems
        }";
        Restart = "on-failure";
        ReadWritePaths = [ cfg.dataDir ];

        CapabilityBoundingSet = "";
        DeviceAllow = "";
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateMounts = true;
        PrivateTmp = true;
        PrivateUsers = true;
        ProtectClock = true;
        ProtectHome = true;
        ProtectHostname = true;
        ProtectSystem = "strict";
        ProtectControlGroups = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [ "@system-service" ];
      };
    };

    systemd.services.gathers-refresh = {
      description = "Refresh GatheRs reference card/pricing databases";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "gathers-refresh" ''
          set -x
          for path in mtg/update mtg/prices/update riftbound/update pokemon/update; do
            ${lib.getExe pkgs.curl} -fsS "http://localhost:${toString cfg.port}/api/$path" || true
          done
        '';
      };
    };

    systemd.timers.gathers-refresh = {
      wantedBy = [ "timers.target" ];
      timerConfig.OnCalendar = cfg.refreshInterval;
    };

    environment.systemPackages = [ cfg.package ];

    services.nginx = lib.mkIf (cfg.reverseProxy == "nginx") {
      enable = true;
      virtualHosts.${cfg.virtualHost}.locations =
        if cfg.frontend.enable then
          {
            "/api/".proxyPass = "http://127.0.0.1:${toString cfg.port}";
            "/" = {
              root = cfg.frontend.package;
              tryFiles = "$uri $uri/ /index.html";
            };
          }
        else
          {
            "/".proxyPass = "http://127.0.0.1:${toString cfg.port}";
          };
    };

    services.caddy = lib.mkIf (cfg.reverseProxy == "caddy") {
      enable = true;
      virtualHosts.${cfg.virtualHost}.extraConfig =
        if cfg.frontend.enable then
          ''
            handle /api/* {
              reverse_proxy 127.0.0.1:${toString cfg.port}
            }
            handle {
              root * ${cfg.frontend.package}
              try_files {path} /index.html
              file_server
            }
          ''
        else
          ''
            reverse_proxy 127.0.0.1:${toString cfg.port}
          '';
    };
  };
}
