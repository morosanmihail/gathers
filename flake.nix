{
  description = "GatheRs development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
      in
      {
        devShells.default = import ./nix/shell.nix { inherit pkgs; };
        packages = import ./nix/packages.nix { inherit pkgs; };
        apps = import ./nix/apps.nix { inherit self system flake-utils; };
      }
    )
    // {
      nixosModules = {
        gathers = import ./nix/gathers.nix { inherit self; };
        default = self.nixosModules.gathers;
      };
    };
}
