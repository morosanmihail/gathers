{ pkgs }:
let
  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    extensions = [
      "rust-src"
      "rust-analyzer"
      "clippy"
      "rustfmt"
    ];
  };
in
pkgs.mkShell {
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
}
