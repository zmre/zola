{
  description = "zola";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {inherit system overlays;};
      rustToolchain = pkgs.rust-bin.stable.latest.default;
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };
      #rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    in rec {
      # `nix build`
      packages.default = rustPlatform.buildRustPackage {
        name = "zola";
        pname = "zola";
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
        };
        cargoToml = ./Cargo.toml;
      };

      # `nix run`
      apps.default = flake-utils.lib.mkApp {drv = packages.default;};

      # nix develop or automatic direnv environment
      devShell = pkgs.mkShell {
        buildInputs = with pkgs; [
          cargo-watch
          cargo-insta
          rustToolchain
          rustfmt
          clippy
          rust-analyzer
        ];
      };
    });
}
