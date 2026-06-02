{
  description = "GatheRs development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.cargo-watch
            pkgs.cargo-edit
            pkgs.openssl
            pkgs.pkg-config
            pkgs.pnpm
            pkgs.tilt
            pkgs.sqlite
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/src";
        };

        packages = rec {
          default = gathers-api;
          gathers-api = pkgs.callPackage (
            {
              sqlite,
              rustPlatform,
              cacert,
            }:
              rustPlatform.buildRustPackage {
                name = "gathers-api";
                src = ./.;
                cargoLock = {lockFile = ./Cargo.lock;};

                buildInputs = [
                  cacert
                  sqlite
                ];

                checkFlags = [
                  # Scryfall API not reachable in nix sandbox
                  "--skip=systems::scryfall::tests"
                ];
              }
          ) {};

          gathers-webui = pkgs.callPackage (
            {
              importNpmLock,
              buildNpmPackage,
            }:
              buildNpmPackage {
                name = "gathers-webui";
                src = ./webui;
                npmDeps = importNpmLock {npmRoot = ./webui;};
                inherit (importNpmLock) npmConfigHook;
              }
          ) {};
        };

        apps = rec {
          default = gathers-cli;
          gathers-api = flake-utils.lib.mkApp {
            drv = self.packages.${system}.gathers-api;
            exePath = "/bin/server";
          };

          gathers-cli = flake-utils.lib.mkApp {
            drv = self.packages.${system}.gathers-api;
            exePath = "/bin/gathers";
          };
        };
      });
}
