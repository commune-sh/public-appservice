{
  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    crane.url = "github:ipetkov/crane";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nix-filter.url = "github:numtide/nix-filter";
    flake-parts.url = "github:hercules-ci/flake-parts";

    mado.url = "github:akiomik/mado";
  };

  outputs = inputs @ { flake-parts, ... }:
    flake-parts.lib.mkFlake
      {
        inherit inputs;
      }
      {
        systems = [
          "x86_64-linux"
          "aarch64-linux"
        ];

        perSystem = { system, ... }:
          let
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [ inputs.rust-overlay.overlays.default ];
            };

            # Create craneLib with our custom rust toolchain
            craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

            # Setup rust-overlay
            rustToolchain = (pkgs.rust-bin.fromRustupToolchainFile
              ./rust-toolchain.toml).override {
              extensions = pkgs.lib.lists.unique ([
                "rustc"
                "cargo"
                "rust-src"
                "rust-analyzer"
                "clippy"
              ]
              ++ (pkgs.lib.importTOML ./rust-toolchain.toml).toolchain.components);
            };

            cargoManifest = pkgs.lib.importTOML "${inputs.self}/Cargo.toml";

            commonArgs =
              {
                pname = cargoManifest.package.name;
                version = cargoManifest.package.version;

                src =
                  let
                    filter = inputs.nix-filter.lib;
                  in
                  filter {
                    root = inputs.self;

                    # Keep sorted
                    include = [
                      "Cargo.lock"
                      "Cargo.toml"
                      "src"
                    ];
                  };

                cargoArtifacts = craneLib.buildDepsOnly (commonArgs
                  // {
                  doCheck = false;

                  meta.mainProgram = cargoManifest.package.name;
                });

                buildInputs = with pkgs; [
                  pkg-config
                  openssl
                ];

                GIT_COMMIT_HASH = "${pkgs.lib.substring 0 8 (inputs.self.shortRev or inputs.self.dirtyShortRev)}";
              };
          in
          {
            # Define packages
            packages = {
              default = craneLib.buildPackage (commonArgs
                // {
                cargoExtraArgs = "--all-features";
              });

              minimal = craneLib.buildPackage (commonArgs
                // {
                cargoExtraArgs = "--no-default-features";
              });
            };

            # Define dev shell with rust-overlay toolchain
            devShells.default = pkgs.mkShell {
              packages = with pkgs; [
                pkgs.rust-bin.nightly.latest.rustfmt

                rustToolchain

                taplo
                engage
                lychee

                cargo-audit
                cargo-machete
                cargo-sort

                inputs.mado.packages.${system}.default
              ];

              # Environment variables
              RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
              PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
            };
          };
      };
}
