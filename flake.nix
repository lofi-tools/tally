{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
    utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    my-utils = {
      url = "github:nmrshll/nix-utils";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.utils.follows = "utils";
    };
  };

  outputs = { self, nixpkgs, utils, rust-overlay, my-utils }:
    with builtins; utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        customRust = pkgs.rust-bin.stable."1.86.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ ];
        };

        baseInputs = [
          customRust
          pkgs.pkg-config
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.apple-sdk_15
          pkgs.libiconv
        ];

        devInputs = with pkgs; [
          nixpkgs-fmt
          cargo-nextest
          # cargo-watch
          # cargo-edit
        ];

        env = {
          RUST_BACKTRACE = "1";
        };

        scripts = mapAttrs (name: value: pkgs.writeShellScriptBin name value) {
          run = ''cargo run -- "$@" '';
          packages = ''if [ -n "$CRATE" ]; then echo "-p $CRATE"; else echo "--workspace"; fi '';
          # utest = ''cargo nextest run --workspace -E '!test(nordigen)' --nocapture -- $SINGLE_TEST '';
          utest = ''set -x; cargo nextest run $(packages) -E "$TEST_FILTER" --nocapture "$@" -- $SINGLE_TEST '';
          ftest = ''set -x; cargo nextest run --workspace -E "$TEST_FILTER" --nocapture "$@" '';
          backup-txs = ''mkdir -p ./.cache/backup && mv ./.cache/starling_transactions.json ./.cache/backup/starling_transactions.$(date +%Y%m%d%H%M).json'';
        };

      in
      {
        devShells.default = with pkgs; mkShell {
          inherit env;
          buildInputs = baseInputs ++ devInputs ++ (attrValues scripts);
          shellHook = "
              ${my-utils.binaries.${system}.configure-vscode};
              dotenv
            ";
        };
      }
    );
}




