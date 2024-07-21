{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    my-utils = {
      url = "github:nmrshll/nix-utils";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.utils.follows = "utils";
    };
  };

  outputs = { self, nixpkgs, utils, rust-overlay, my-utils }:
    utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          customRust = pkgs.rust-bin.stable."1.77.0".default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
            targets = [ ];
          };

          baseInputs = [
            customRust
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            pkgs.darwin.apple_sdk.frameworks.CoreServices
            pkgs.darwin.apple_sdk.frameworks.CoreFoundation
            pkgs.darwin.apple_sdk.frameworks.Foundation
            pkgs.libiconv
          ];

          devInputs = with pkgs; [
            nixpkgs-fmt
            # cargo-watch
            # cargo-edit
          ];

          scripts = with pkgs; [
            (writeScriptBin "run" ''cargo run -- "$@" '')
            (writeScriptBin "utest" ''cargo test --workspace -- $SINGLE_TEST --nocapture '')
          ];

        in
        {
          devShells.default = with pkgs; mkShell {
            buildInputs = baseInputs ++ devInputs ++ scripts;
            shellHook = "
              ${my-utils.binaries.${system}.configure-vscode};
              dotenv
            ";
          };
        }
      );
}




