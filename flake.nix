{
  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    rust-manifest.url = "https://static.rust-lang.org/dist/channel-rust-1.83.0.toml";
    rust-manifest.flake = false;

    crane.url = "github:ipetkov/crane";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nix-filter.url = "github:numtide/nix-filter";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs = inputs @ {
    flake-parts,
    rust-overlay,
    nixpkgs,
    ...
  }:
    flake-parts.lib.mkFlake {
      inherit inputs;
    } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem = {
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [rust-overlay.overlays.default];
        };

        # Create craneLib with our custom rust toolchain
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Setup rust-overlay
        rustToolchain =
          (pkgs.rust-bin.fromRustupToolchainFile
            ./rust-toolchain.toml)
          .override {
            extensions =
              [
                "rustc"
                "cargo"
                "rust-docs"
                "rustfmt"
                "clippy"
              ]
              ++ (pkgs.lib.importTOML ./rust-toolchain.toml).toolchain.components;
          };

        cargoManifest = pkgs.lib.importTOML "${inputs.self}/Cargo.toml";

        commonArgs = {
          inherit (cargoManifest.package) name version;
          src = inputs.nix-filter.lib {
            root = inputs.self;
            include = [
              "Cargo.toml"
              "Cargo.lock"
              "src/**"
              "tests/**" # If you have tests
            ];
          };

          cargoArtifacts = craneLib.buildDepsOnly (commonArgs
            // {
              doCheck = false;
            });
        };
      in {
        # Define packages
        packages = {
          default = craneLib.buildPackage (commonArgs
            // {
              cargoExtraArgs = "--locked --all-features";
            });

          minimal = craneLib.buildPackage (commonArgs
            // {
              cargoExtraArgs = "--locked --no-default-features";
            });
        };

        # Define dev shell with rust-overlay toolchain
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            pkgs.rust-bin.nightly.latest.rustfmt
            taplo
            rustToolchain
          ];

          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        checks = {
          fmt = craneLib.cargoFmt (commonArgs
            // {
              cargoExtraArgs = "-- --check";
            });
          clippy = craneLib.cargoClippy (commonArgs
            // {
              cargoClippyExtraArgs = "-- -D warnings";
            });
          audit = craneLib.cargoAudit (commonArgs
            // {
              inherit (inputs) advisory-db;
            });
        };
      };
    };
}
