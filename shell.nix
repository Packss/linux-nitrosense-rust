{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    rustfmt
    clippy
    pkg-config
    gtk4
    glib
  ];

  shellHook = ''
    echo "NitroSense Rust development shell loaded"
  '';
}
