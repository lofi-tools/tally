{
  # inputs = {
  #   nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
  #   utils.url = "github:numtide/flake-utils";
  #   rust-overlay.url = "github:oxalica/rust-overlay";
  #   my-utils = {
  #     url = "github:nmrshll/nix-utils";
  #     inputs.nixpkgs.follows = "nixpkgs";
  #     inputs.utils.follows = "utils";
  #   };
  # };
  inputs.nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";
  inputs.parts.url = "github:hercules-ci/flake-parts";
  inputs.my-nix = { url = "github:nmrshll/my-nix"; inputs.nixpkgs.follows = "nixpkgs"; inputs.fp.follows = "parts"; };

  # outputs = { self, nixpkgs, utils, rust-overlay, my-utils }:
  #   with builtins; utils.lib.eachDefaultSystem (system:
  #     let
  #       pkgs = import nixpkgs {
  #         inherit system;
  #         overlays = [ (import rust-overlay) ];
  #       };
  #       customRust = pkgs.rust-bin.stable."1.86.0".default.override {
  #         extensions = [ "rust-src" "rust-analyzer" ];
  #         targets = [ ];
  #       };

  #       baseInputs = [
  #         customRust
  #         pkgs.pkg-config
  #       ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
  #         pkgs.apple-sdk_15
  #         pkgs.libiconv
  #       ];

  #       devInputs = with pkgs; [
  #         nixpkgs-fmt
  #         cargo-nextest
  #         # cargo-watch
  #         # cargo-edit
  #       ];

  #       env = {
  #         RUST_BACKTRACE = "1";
  #         CARGO_NET_GIT_FETCH_WITH_CLI = "true";
  #       };

  #       scripts = mapAttrs (name: value: pkgs.writeShellScriptBin name value) {
  #         run = ''cargo run -- "$@" '';
  #         packages = ''if [ -n "$CRATE" ]; then echo "-p $CRATE"; else echo "--workspace"; fi '';
  #         # utest = ''cargo nextest run --workspace -E '!test(nordigen)' --nocapture -- $SINGLE_TEST '';
  #         utest = ''set -x; cargo nextest run $(packages) -E "''${TEST_FILTER:-all()}" --nocapture "$@" -- $SINGLE_TEST '';
  #         ftest = ''set -x; cargo nextest run --workspace -E "''${TEST_FILTER:-all()}" --nocapture "$@" '';
  #         backup-txs = ''mkdir -p ./.cache/backup && mv ./.cache/starling_transactions.json ./.cache/backup/starling_transactions.$(date +%Y%m%d%H%M).json'';
  #       };

  #     in
  #     {
  #       devShells.default = with pkgs; mkShell {
  #         inherit env;
  #         buildInputs = baseInputs ++ devInputs ++ (attrValues scripts);
  #         shellHook = "
  #             ${my-utils.binaries.${system}.configure-vscode};
  #             dotenv
  #           ";
  #       };
  #     }
  #   );
  outputs = inputs@{ parts, ... }: parts.lib.mkFlake { inherit inputs; } ({ lib, ... }: with builtins; {
    systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
    imports = lib.flatten [
      (attrValues inputs.my-nix.flakeModules.essentials)
      inputs.my-nix.flakeModules.rust
      (inputs.my-nix.lib.findFlakePartFilesRec ./.)
    ];
    perSystem = { pkgs, system, ... }:
      let
        bin = inputs.my-nix.bin.${system} // (mapAttrs (n: p: "${p}/bin/${n}") scripts);
        buildTimeDeps = [
          pkgs.pkg-config
        ];
        runtimeDeps = [ ];
        devDeps = [
          pkgs.cargo-nextest
        ];

        bash.wd = "$(git rev-parse --show-toplevel)";
        scripts = mapAttrs (n: s: pkgs.writeShellScriptBin n s) {
          run = ''cargo run -- "$@" '';
          packages = ''if [ -n "$CRATE" ]; then echo "-p $CRATE"; else echo "--workspace"; fi '';
          # utest = ''cargo nextest run --workspace -E '!test(nordigen)' --nocapture -- $SINGLE_TEST '';
          utest = ''set -x; cargo nextest run $(packages) -E "''${TEST_FILTER:-all()}" --nocapture "$@" -- $SINGLE_TEST '';
          ftest = ''set -x; cargo nextest run --workspace -E "''${TEST_FILTER:-all()}" --nocapture "$@" '';
          backup-txs = ''mkdir -p ./.cache/backup && mv ./.cache/starling_transactions.json ./.cache/backup/starling_transactions.$(date +%Y%m%d%H%M).json'';
        };

        env = {
          # RUST_BACKTRACE = "1";
          CARGO_NET_GIT_FETCH_WITH_CLI = "true";
        };
      in
      {
        packages = scripts;
        rust.buildInputs = runtimeDeps;
        rust.nativeBuildInputs = buildTimeDeps;
        rust.buildEnv = env;
        myDevShell.env = env;
        myDevShell.buildInputs = buildTimeDeps ++ runtimeDeps ++ devDeps ++ (attrValues scripts);
        myDevShell.shellHooks = { };
      };
  });
}
