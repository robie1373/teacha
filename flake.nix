{
  description = "Teacha — FSR-based CLI tip reminder system";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        rustToolchain = with pkgs; [
          cargo
          rustc
          rust-analyzer
          clippy
          rustfmt
        ];

        # Libraries required to link and run the Tauri 2 GUI binary.
        # webkitgtk_4_1 is required — Tauri 2 dropped support for 4.0.
        tauriLibs = with pkgs; [
          at-spi2-atk
          cairo
          dbus
          gdk-pixbuf
          glib
          glib-networking   # GIO TLS module (WebKit needs HTTPS)
          gtk3
          harfbuzz
          libayatana-appindicator
          librsvg
          libsoup_3
          openssl
          pango
          webkitgtk_4_1
        ];

        teacha-daemon = pkgs.rustPlatform.buildRustPackage {
          pname   = "teacha-daemon";
          version = "0.3.0";
          # src points directly at src-tauri/ so Cargo.toml is at the root.
          src     = ./src-tauri;
          cargoLock.lockFile = ./src-tauri/Cargo.lock;

          # Daemon only — no WebKitGTK or Tauri GUI deps.
          cargoBuildFlags = [ "--bin" "teacha-daemon" "--no-default-features" ];

          # ureq uses native-tls which links against openssl on Linux.
          buildInputs    = with pkgs; [ openssl ];
          nativeBuildInputs = with pkgs; [ pkg-config ];

          doCheck = false;
        };

      in {
        packages.teacha-daemon = teacha-daemon;

        devShells = {
          # Full Tauri dev environment.
          # Provides: `cargo tauri dev`, `cargo tauri build`, full GUI compilation.
          # Enter with: nix develop
          default = pkgs.mkShell {
            packages = rustToolchain ++ (with pkgs; [
              cargo-tauri    # tauri CLI (cargo tauri dev / build)
              nodejs         # required for the frontend asset pipeline
              pkg-config
              gobject-introspection
            ]) ++ tauriLibs;

            # Prevents WebKit DMA buffer issues on NixOS/Wayland.
            WEBKIT_DISABLE_DMABUF_RENDERER = "1";

            # Point GIO to the TLS module so WebKit can load HTTPS.
            GIO_MODULE_DIR = "${pkgs.glib-networking}/lib/gio/modules/";
          };

          # Minimal shell for daemon + core lib work — no WebKitGTK.
          # Use when you only need:
          #   cargo test --lib --no-default-features
          #   cargo test --bin teacha-daemon --no-default-features
          #   cargo build --bin teacha-daemon --no-default-features
          # Enter with: nix develop .#core
          core = pkgs.mkShell {
            packages = rustToolchain ++ (with pkgs; [
              gcc
              pkg-config
              openssl
            ]);
          };
        };
      }
    );
}
