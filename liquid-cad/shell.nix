
let
  here = toString ./.;
  moz_overlay = import (builtins.fetchTarball
    "https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz");
  pkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  rust = (pkgs.rustChannelOf {
    channel = "stable";
  }).rust.override {
    extensions = [ "rust-src" "rust-analysis" ];
    targets = [ "wasm32-unknown-unknown" ];
  };
  rustPlatform = pkgs.makeRustPlatform {
    rustc = rust;
    cargo = rust;
  };
in pkgs.mkShell {
  buildInputs = [
    pkgs.pkg-config
    pkgs.openssl
    rust

    pkgs.wasm-pack
    pkgs.wasm-bindgen-cli
    pkgs.trunk
  ];
  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang}/lib";
  LD_LIBRARY_PATH = "${pkgs.stdenv.cc.cc.lib}/lib64:$LD_LIBRARY_PATH";
  CARGO_INCREMENTAL = 1;
}
