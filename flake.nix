{
  inputs.nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";
  inputs.parts.url = "github:hercules-ci/flake-parts";
  inputs.my-nix = { url = "github:nmrshll/my-nix"; inputs.nixpkgs.follows = "nixpkgs"; inputs.fp.follows = "parts"; };

  outputs = inputs@{ parts, ... }: parts.lib.mkFlake { inherit inputs; } ({ lib, ... }: with builtins; {
    systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
    imports = lib.flatten [
      (attrValues inputs.my-nix.flakeModules.essentials)
      inputs.my-nix.flakeModules.rust
      (inputs.my-nix.lib.findFlakePartFilesRec ./.)
    ];
    perSystem = { pkgs, system, ... }:
      let
        buildTimeDeps = [
          pkgs.pkg-config
        ];
        runtimeDeps = [ ];
        devDeps = [
          pkgs.cargo-nextest
        ];

        ownPkgs.piecash = pkgs.python3Packages.buildPythonPackage rec {
          pname = "piecash";
          version = "1.2.0";
          format = "setuptools";

          src = pkgs.fetchPypi {
            inherit pname version;
            hash = "sha256-iWOfBmHUkiQng/OcjRR+pFwyHcQRH5PsopefBw9fF20=";
          };

          propagatedBuildInputs = [
            pkgs.python3Packages.sqlalchemy
            pkgs.python3Packages.pyyaml
          ];

          pythonImportsCheck = [ "piecash" ];

          meta = {
            description = "Python library for GnuCash file access";
            homepage = "https://github.com/sdementen/piecash";
            license = lib.licenses.gpl2Plus;
          };
        };

        ownPkgs.ixbrl-parse = pkgs.python3Packages.buildPythonPackage rec {
          pname = "ixbrl-parse";
          version = "0.3";
          format = "setuptools";

          src = pkgs.fetchPypi {
            inherit pname version;
            hash = "";
          };

          propagatedBuildInputs = [
            pkgs.python3Packages.lxml
          ];

          pythonImportsCheck = [ "ixbrl" ];

          meta = {
            description = "Python iXBRL parser";
            homepage = "https://github.com/cybermaggedon/ixbrl-parse";
            license = lib.licenses.gpl3Plus;
          };
        };

        ownPkgs.ixbrl-reporter =
          pkgs.python3Packages.buildPythonPackage rec {
            pname = "ixbrl-reporter";
            version = "1.2.1";
            format = "pyproject";

            src = pkgs.fetchFromGitHub {
              owner = "cybermaggedon";
              repo = "ixbrl-reporter";
              rev = "v${version}";
              hash = "sha256-AMAO3ygDiIVkCsHmHy1fdGp4CVgb7YRV1M8w1mymUhY=";
            };

            propagatedBuildInputs = [
              pkgs.python3Packages.requests
              pkgs.python3Packages.lxml
              ownPkgs.piecash
              pkgs.python3Packages.pyyaml
            ];

            nativeBuildInputs = [
              pkgs.python3Packages.setuptools
              pkgs.python3Packages.pytest
              pkgs.python3Packages.pytest-cov
              pkgs.python3Packages.pytest-mock
              ownPkgs.ixbrl-parse
            ];

            pythonImportsCheck = [ "ixbrl_reporter" ];

            meta = {
              description = "Production of iXBRL reports from templates and accounts files";
              homepage = "https://github.com/cybermaggedon/ixbrl-reporter";
              license = lib.licenses.gpl3Plus;
              platforms = lib.platforms.unix;
            };
          };

        bash.wd = "$(git rev-parse --show-toplevel)";
        bin = inputs.my-nix.bin.${system} // (mapAttrs (n: p: "${p}/bin/${n}") scripts) // {
          ixbrl = "${ownPkgs.ixbrl-reporter}/bin/ixbrl-reporter";
        };
        scripts = mapAttrs (n: s: pkgs.writeShellScriptBin n s) {
          run = ''cargo run -- "$@" '';
          packages = ''if [ -n "$CRATE" ]; then echo "-p $CRATE"; else echo "--workspace"; fi '';
          utest = ''set -x; cargo nextest run $(packages) -E "''${TEST_FILTER:-all()}" --nocapture "$@" -- $SINGLE_TEST '';
          ftest = ''set -x; cargo nextest run --workspace -E "''${TEST_FILTER:-all()}" --nocapture "$@" '';
          backup-txs = ''mkdir -p ./.cache/backup && mv ./.cache/starling_transactions.json ./.cache/backup/starling_transactions.$(date +%Y%m%d%H%M).json'';
          report = ''${bin.ixbrl} report/hmrc/corp-tax.yaml report ixbrl > .cache/corp-tax.html'';
        };

        env = {
          CARGO_NET_GIT_FETCH_WITH_CLI = "true";
        };
      in
      {
        packages = scripts // ownPkgs;
        rust.buildInputs = runtimeDeps;
        rust.nativeBuildInputs = buildTimeDeps;
        rust.buildEnv = env;
        myDevShell.env = env;
        myDevShell.buildInputs = buildTimeDeps ++ runtimeDeps ++ devDeps ++ (attrValues scripts);
        myDevShell.shellHooks = { };
      };
  });
}
