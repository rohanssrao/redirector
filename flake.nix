{
  description = "Redirector - URL redirector/cleaner for Linux desktop";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        inherit (pkgs) lib;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            pkg-config
            gtk4
            glib
            libadwaita
            adwaita-icon-theme
          ];

          GTK4_MODULE_PATH = "${pkgs.adwaita-icon-theme}/share/icons/Adwaita";
          LD_LIBRARY_PATH = lib.makeLibraryPath (with pkgs; [ gtk4 glib ]);
        };

        packages.default = rustPlatform.buildRustPackage {
          pname = "redirector";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = with pkgs; [ gtk4 libadwaita glib ];
          doCheck = false;
        };
      }
    );
}
