{
  description = "Local-first quality gates for ratcore";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    tigerstyle = {
      url = "git+ssh://git@github.com/brittonr/tigerstyle-rs.git?ref=main";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.crane.follows = "crane";
      inputs.flake-utils.follows = "flake-utils";
      inputs.rust-overlay.follows = "rust-overlay";
    };
  };

  outputs = {
    self,
    crane,
    flake-utils,
    nixpkgs,
    rust-overlay,
    tigerstyle,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            (craneLib.filterCargoSources path type)
            || builtins.elem (builtins.baseNameOf path) [
              "clippy.toml"
              "dylint.toml"
            ];
        };
        fmtSrc = pkgs.lib.cleanSource ./.;
        nativeBuildInputs = [ pkgs.gcc ];
        commonArgs = {
          inherit nativeBuildInputs src;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        ratcore = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;
          }
        );
      in
      {
        packages.default = ratcore;

        checks = {
          build = ratcore;

          test = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );

          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets --all-features -- -D warnings";
            }
          );

          fmt = craneLib.cargoFmt {
            src = fmtSrc;
          };

          tigerstyle = tigerstyle.lib.mkConsumerCheck {
            inherit system;
            src = ./.;
            cargoLock = ./Cargo.lock;
          };
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [
            pkgs.gcc
            rustToolchain
            tigerstyle.packages.${system}.cargo-tigerstyle
          ];
        };
      }
    );
}
