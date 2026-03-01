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
    gsettings-desktop-schemas
    kmod
    libclang
  ];

  LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

  shellHook = ''
    export XDG_DATA_DIRS=$XDG_DATA_DIRS:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}
    echo "NitroSense Rust development shell loaded"
  '';
}
