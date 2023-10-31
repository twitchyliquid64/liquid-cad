let
  here = toString ./.;
  moz_overlay = import (builtins.fetchTarball
    "https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz");
  pkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  rust = (pkgs.rustChannelOf {
    channel = "stable";
  }).rust.override {
    extensions = [ "rust-src" "rust-analysis" ];
  };
  rustPlatform = pkgs.makeRustPlatform {
    rustc = rust;
    cargo = rust;
  };

in pkgs.mkShell rec {

  buildInputs = with pkgs; [
    xorg.libX11
    wayland
    libxkbcommon
    # xorg.libxcb xorg.xcbutil libxkbcommon #xorg.libXrender

    libGL
    libGLU
  ];

  nativeBuildInputs = with pkgs; [
    # Toolchain
    rust pkg-config
    # IDE tooling
    rust-analyzer clippy rustfmt
  ];

  hardeningDisable = [ "fortify" ];

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
}
