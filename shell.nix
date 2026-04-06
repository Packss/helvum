{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    cargo
    rustc
    rustfmt
    clippy
    meson
    ninja
    pkg-config
    clang
    llvmPackages.libclang
  ];

  buildInputs = with pkgs; [
    glib
    gtk4
    libadwaita
    pipewire
    gsettings-desktop-schemas
    hicolor-icon-theme
  ];

  shellHook = ''
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
    export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share:${pkgs.libadwaita}/share:${pkgs.hicolor-icon-theme}/share:''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
  '';
}