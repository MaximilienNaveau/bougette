{
  description = "Bougette: local-first app for individual and shared budgets";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        # A check that runs a command over the committed sources and
        # passes if it exits 0 (its output is an empty marker file).
        sourceCheck =
          name: inputs: command:
          pkgs.runCommand "bougette-${name}" { nativeBuildInputs = inputs; } ''
            cd ${./.}
            ${command}
            touch "$out"
          '';

        # Only what Cargo needs, so editing docs doesn't rebuild the Rust.
        rustSrc = pkgs.lib.fileset.toSource {
          root = ./.;
          fileset = pkgs.lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./crates
          ];
        };

        # A Cargo command over the workspace, with the crates of Cargo.lock
        # fetched by Nix (no network inside the build).
        cargoCheck =
          name: inputs: command:
          pkgs.rustPlatform.buildRustPackage {
            pname = "bougette-${name}";
            version = "0.1.0";
            src = rustSrc;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = inputs;
            buildPhase = ''
              runHook preBuild
              ${command}
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''touch "$out"'';
          };
      in
      {
        # `nix develop`: the Rust toolchain from nixpkgs, pinned by
        # flake.lock. Flutter and the Android targets come with the first
        # UI code.
        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
            pkgs.rust-analyzer
            pkgs.nixfmt
          ];
        };

        # `nix fmt`: format the Nix files.
        formatter = pkgs.nixfmt;

        # `nix flake check -L`: everything CI runs.
        checks = {
          nix-format = sourceCheck "nix-format" [ pkgs.nixfmt ] ''
            nixfmt --check $(find . -name '*.nix')
          '';
          rust-format = sourceCheck "rust-format" [ pkgs.rustfmt ] ''
            rustfmt --check --edition 2024 $(find crates -name '*.rs')
          '';
          clippy = cargoCheck "clippy" [ pkgs.clippy ] ''
            cargo clippy --offline --all-targets -- -D warnings
          '';
          tests = cargoCheck "tests" [ ] ''
            cargo test --offline
          '';
        };
      }
    );
}
