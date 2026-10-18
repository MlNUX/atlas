# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
{
  description = "Atlas Workspace – Linux-first, file-centric desktop workspace (Phase 0 prototype)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      qtDeps =
        pkgs: with pkgs.qt6; [
          qtbase
          qtdeclarative
          qtwayland
          qtsvg
        ];

      # cxx-qt-build asks `qmake -query` where Qt headers, libraries and tools
      # live. In nixpkgs every Qt module is a separate store path and qmake
      # only knows qtbase, so build one merged prefix with a real qmake copy
      # and a qt.conf that points it at the merged tree.
      qtEnv =
        pkgs:
        pkgs.runCommand "atlas-qt-prefix-${pkgs.qt6.qtbase.version}" { } ''
          mkdir -p $out/bin
          for p in ${pkgs.lib.concatStringsSep " " (qtDeps pkgs)}; do
            for d in include lib libexec metatypes mkspecs modules; do
              if [ -d "$p/$d" ]; then
                mkdir -p "$out/$d"
                cp -rsn "$p/$d/." "$out/$d/"
                chmod -R u+w "$out/$d"
              fi
            done
          done
          cp ${pkgs.qt6.qtbase}/bin/qmake $out/bin/qmake
          cat > $out/bin/qt.conf <<EOF
          [Paths]
          Prefix = $out
          Headers = include
          Libraries = lib
          LibraryExecutables = libexec
          Binaries = bin
          Plugins = lib/qt-6/plugins
          Qml2Imports = lib/qt-6/qml
          ArchData = .
          Data = .
          HostPrefix = $out
          HostBinaries = bin
          HostLibraries = lib
          HostLibraryExecutables = libexec
          HostData = .
          EOF
        '';

      nativeDeps =
        pkgs: with pkgs; [
          pkg-config
          (qtEnv pkgs)
          makeWrapper
        ];

      libDeps =
        pkgs: with pkgs; [
          poppler # poppler-glib
          cairo
          glib
          pdfium
          fontconfig
        ];
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "atlas";
          version = "0.0.1";
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = nativeDeps pkgs ++ [ pkgs.qt6.wrapQtAppsHook ];
          buildInputs = qtDeps pkgs ++ libDeps pkgs;

          ATLAS_PDFIUM_LIB = "${pkgs.pdfium}/lib/libpdfium.so";
          ATLAS_RG_BIN = "${pkgs.ripgrep}/bin/rg";

          # The qtbase setup hook exports QMAKE; point it at the merged prefix.
          preBuild = ''
            export QMAKE=${qtEnv pkgs}/bin/qmake
          '';

          postInstall = ''
            install -Dm644 data/atlas.desktop $out/share/applications/atlas.desktop
          '';

          # GUI tests need a display; core tests run in `nix flake check`.
          cargoTestFlags = [
            "-p"
            "atlas-core"
          ];

          meta = {
            description = "File-centric desktop workspace with PDF, Markdown and terminal";
            license = pkgs.lib.licenses.gpl3Plus;
            mainProgram = "atlas";
            platforms = pkgs.lib.platforms.linux;
          };
        };
      });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.system}.default}/bin/atlas";
        };
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages =
            nativeDeps pkgs
            ++ qtDeps pkgs
            ++ libDeps pkgs
            ++ (with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              ripgrep
              hyperfine
            ]);

          ATLAS_PDFIUM_LIB = "${pkgs.pdfium}/lib/libpdfium.so";
          ATLAS_RG_BIN = "${pkgs.ripgrep}/bin/rg";
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          # Debug builds compile C++ without -O, which _FORTIFY_SOURCE warns about.
          hardeningDisable = [ "fortify" ];

          # Make Qt plugins (incl. the Wayland platform plugin) and QML modules
          # visible to `cargo run` without wrapping.
          shellHook = ''
            export QMAKE=${qtEnv pkgs}/bin/qmake
            export QT_PLUGIN_PATH="${pkgs.lib.makeSearchPath pkgs.qt6.qtbase.qtPluginPrefix (qtDeps pkgs)}"
            export QML_IMPORT_PATH="${pkgs.lib.makeSearchPath pkgs.qt6.qtbase.qtQmlPrefix (qtDeps pkgs)}"
            export QT_QPA_PLATFORM="''${QT_QPA_PLATFORM:-wayland}"
          '';
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
