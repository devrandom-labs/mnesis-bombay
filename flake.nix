{
  description = "mnesis-bombay — runtime-neutral Mnesis integration with Bombay";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, utils, crane, fenix, advisory-db, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rustToolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-mvUGEOHYJpn3ikC5hckneuGixaC+yGrkMM/liDIDgoU=";
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
        src = pkgs.lib.fileset.toSource {
          root = ./.;
          fileset = pkgs.lib.fileset.unions [
            (craneLib.fileset.commonCargoSources ./.)
            ./docs
          ];
        };
        commonArgs = {
          inherit src;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        checks = {
          cargo-test = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            cargoNextestExtraArgs = "--workspace --all-features";
          });
          cargo-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace --all-features --all-targets -- --deny warnings";
          });
          cargo-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
            cargoDocExtraArgs = "--workspace --all-features --no-deps";
          });
          cargo-fmt = craneLib.cargoFmt { inherit src; };
          cargo-toml-fmt = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };
          cargo-deny = craneLib.cargoDeny { inherit src; };
          cargo-deny-advisories = craneLib.cargoDeny (commonArgs // {
            cargoDenyChecks = "advisories --disable-fetch";
            nativeBuildInputs = [ pkgs.git ];
            preBuild = ''
              mkdir -p "$CARGO_HOME/advisory-dbs"
              db="$CARGO_HOME/advisory-dbs/advisory-db-3157b0e258782691"
              cp -R ${advisory-db} "$db"
              chmod -R u+w "$db"
              git -C "$db" init --quiet
              git -C "$db" add .
              git -C "$db" \
                -c user.name=RustSec -c user.email=rustsec@example.invalid \
                commit --quiet -m "Pinned RustSec advisory database"
              git -C "$db" rev-parse HEAD > "$db/.git/FETCH_HEAD"
            '';
          });
          core-nostd = craneLib.mkCargoDerivation (commonArgs // {
            inherit cargoArtifacts;
            pname = "mnesis-bombay-core-nostd";
            doInstallCargoArtifacts = false;
            buildPhaseCargoCommand = ''
              cargo build -p mnesis-bombay-core --target thumbv7em-none-eabihf
            '';
          });
        };

        packages.default = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "--workspace";
        });

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [
            cargo-audit
            cargo-deny
            cargo-nextest
            git
            just
            taplo
          ];
          shellHook = ''
            git config core.hooksPath .githooks 2>/dev/null || true
          '';
        };
      });
}
