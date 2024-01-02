# Liquid CAD

A 2D constraint-solving CAD program for rapid prototyping. Heavily inspired by SolveSpace.

### Backlog for Beta

 * Export to DXF
 * Export to STL
 * Arc dragging
 * Warn if solver tolerance if off in export UI
 * Broken constraint tools / solver stability (need help!):
  * Line angle
  * Lines parallel

### Building locally

#### Desktop

Make sure you are using the latest version of stable rust by running `rustup update`.

`cargo run --release`

On Linux you need to first run:

`sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev`

On Fedora Rawhide you need to run:

`dnf install clang clang-devel clang-tools-extra libxkbcommon-devel pkg-config openssl-devel libxcb-devel gtk3-devel atk fontconfig-devel`

On NixOS:

`nix-shell` to get into a working environment.

#### Web

On NixOS: `cd liquid-cad && nix-shell` then `trunk serve` or `trunk build --release`

Legacy OS'es:

1. Install the required target with `rustup target add wasm32-unknown-unknown`.
2. Install Trunk with `cargo install --locked trunk`.

Locally: Within the `liquid-cad` directory, run `trunk serve` to build and serve on `http://127.0.0.1:8080`. Trunk will rebuild automatically if you edit the project.

Deploying: Run `trunk build --release` and copy the `dist` directory.

### License

Under MIT / Apache 2.0. Some icons from Noto emoji font under Apache 2.0.