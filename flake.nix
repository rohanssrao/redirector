{
  description = "Redirector - URL redirector/cleaner for Linux desktop";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            gtk4
            glib
            cairo
            pixman
            pango
            atk
            harfbuzz
            libadwaita
            pkg-config
          ];

          # GTK4 build hints
          GTK4_MODULE_PATH = "${pkgs.adwaita-icon-theme}/share/icons/Adwaita";

          # Let cargo build scripts find libraries
          LD_LIBRARY_PATH = "${pkgs.glib}/lib:${pkgs.cairo}/lib:${pkgs.pixman}/lib:${pkgs.pango}/lib:${pkgs.atk}/lib:${pkgs.harfbuzz}/lib:${pkgs.gtk4}/lib";
        };
      }
    );
}
