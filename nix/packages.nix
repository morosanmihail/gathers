{ pkgs }:
let
  gathers-api = pkgs.callPackage (
    {
      sqlite,
      rustPlatform,
      cacert,
    }:
    rustPlatform.buildRustPackage {
      name = "gathers-api";
      src = ../.;
      cargoLock = {
        lockFile = ../Cargo.lock;
      };

      buildInputs = [
        cacert
        sqlite
      ];

      checkFlags = [
        # Scryfall API not reachable in nix sandbox
        "--skip=systems::scryfall::tests"
      ];
    }
  ) { };

  gathers-webui2 = pkgs.callPackage (
    {
      importNpmLock,
      buildNpmPackage,
    }:
    buildNpmPackage {
      name = "gathers-webui2";
      src = ../webui2;
      npmDeps = importNpmLock { npmRoot = ../webui2; };
      inherit (importNpmLock) npmConfigHook;

      # `build/` is gitignored, so npmInstallHook's default `npm pack`-based
      # install would skip the very output `npm run build` just produced.
      installPhase = ''
        runHook preInstall
        cp -r build $out
        runHook postInstall
      '';
    }
  ) { };
in
{
  inherit gathers-api gathers-webui2;
  default = gathers-api;
}
