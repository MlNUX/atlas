# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
{
  description = "Atlas Workspace – Linux-first, file-centric desktop workspace ";

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
          qtimageformats
          qtwebengine
          qtwebchannel
          qtpositioning
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

      # QML modules needed at run time (not part of the merged Qt prefix).
      qmlDeps = pkgs: [ pkgs.kdePackages.syntax-highlighting ];

      version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;

      libDeps =
        pkgs: with pkgs; [
          poppler # poppler-glib
          cairo
          glib
          fontconfig
          wayland # touchpad hold gesture (stops momentum scrolling)
        ];
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "atlas";
          inherit version;
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = nativeDeps pkgs ++ [ pkgs.qt6.wrapQtAppsHook ];
          buildInputs = qtDeps pkgs ++ qmlDeps pkgs ++ libDeps pkgs;

          ATLAS_RG_BIN = "${pkgs.ripgrep}/bin/rg";
          # "Formel als LaTeX": ONNX Runtime, loaded at run time (the path in
          # the binary keeps it in the closure).
          ATLAS_ORT_DYLIB = "${pkgs.onnxruntime}/lib/libonnxruntime.so";
          # Spellcheck: German and English Hunspell dictionaries.
          ATLAS_HUNSPELL_DIRS = "${pkgs.hunspellDicts.de_DE}/share/hunspell:${pkgs.hunspellDicts.en_US}/share/hunspell";

          # The qtbase setup hook exports QMAKE; point it at the merged prefix.
          preBuild = ''
            export QMAKE=${qtEnv pkgs}/bin/qmake
          '';

          postInstall = ''
            install -Dm644 data/atlas.desktop $out/share/applications/atlas.desktop
            install -Dm644 data/atlas.svg $out/share/icons/hicolor/scalable/apps/atlas.svg
          '';

          # GUI tests need a display; core tests run in `nix flake check`.
          cargoTestFlags = [
            "-p"
            "atlas-core"
          ];

          meta = {
            description = "File-centric desktop workspace with PDF, Markdown and terminal";
            homepage = "https://github.com/MlNUX/atlas";
            license = pkgs.lib.licenses.gpl3Plus;
            mainProgram = "atlas";
            platforms = pkgs.lib.platforms.linux;
          };
        };
      });

      # `pkgs.atlas` in a NixOS configuration: nixpkgs.overlays = [ atlas.overlays.default ];
      overlays.default = final: prev: {
        atlas = self.packages.${final.stdenv.hostPlatform.system}.default;
      };

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
            ++ qmlDeps pkgs
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

          ATLAS_RG_BIN = "${pkgs.ripgrep}/bin/rg";
          ATLAS_ORT_DYLIB = "${pkgs.onnxruntime}/lib/libonnxruntime.so";
          ATLAS_HUNSPELL_DIRS = "${pkgs.hunspellDicts.de_DE}/share/hunspell:${pkgs.hunspellDicts.en_US}/share/hunspell";
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          # Debug builds compile C++ without -O, which _FORTIFY_SOURCE warns about.
          hardeningDisable = [ "fortify" ];

          # Make Qt plugins (incl. the Wayland platform plugin) and QML modules
          # visible to `cargo run` without wrapping.
          shellHook = ''
            export QMAKE=${qtEnv pkgs}/bin/qmake
            export QT_PLUGIN_PATH="${pkgs.lib.makeSearchPath pkgs.qt6.qtbase.qtPluginPrefix (qtDeps pkgs)}"
            export QML_IMPORT_PATH="${pkgs.lib.makeSearchPath pkgs.qt6.qtbase.qtQmlPrefix (qtDeps pkgs ++ qmlDeps pkgs)}"
            export QT_QPA_PLATFORM="''${QT_QPA_PLATFORM:-wayland}"
          '';
        };
      });

      # `nix flake check`: package build (incl. core tests) and formatting.
      checks = forAllSystems (pkgs: {
        package = self.packages.${pkgs.system}.default;
        rustfmt = pkgs.runCommand "atlas-rustfmt" { nativeBuildInputs = [ pkgs.rustfmt ]; } ''
          cd ${./.}
          find crates -name "*.rs" -print0 | xargs -0 rustfmt --edition 2024 --config-path rustfmt.toml --check
          touch $out
        '';
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
