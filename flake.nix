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
        buildTimeDeps = [ pkgs.pkg-config ];
        runtimeDeps = [ ];
        devDeps = [ pkgs.cargo-nextest pkgs.arelle ];

        pythonVers = pkgs.python312Packages;

        ownPkgs.sqlalchemy-utils = pythonVers.buildPythonPackage {
          pname = "SQLAlchemy-Utils";
          version = "0.37.9";
          format = "setuptools";

          src = pkgs.fetchurl {
            url = "https://files.pythonhosted.org/packages/6b/71/8da8a230490126ac94efdbab7d78a0248a9b5e51e0c1fda1f134b5ecb4c9/SQLAlchemy-Utils-0.37.9.tar.gz";
            hash = "sha256-RmftvcsezgEQdraXcu9SS/uxfMl+A/Ee5rhdmOd0HWE=";
          };

          propagatedBuildInputs = with pythonVers; [
            sqlalchemy_1_4
          ];

          doCheck = false;

          meta = {
            description = "Various utility functions for SQLAlchemy";
            homepage = "https://github.com/kvesterod/sqlalchemy-utils";
            license = lib.licenses.bsd3;
          };
        };

        ownPkgs.piecash = pythonVers.buildPythonPackage rec {
          pname = "piecash";
          version = "1.2.1";
          format = "setuptools";

          src = pkgs.fetchFromGitHub {
            owner = "sdementen";
            repo = "piecash";
            rev = "refs/tags/${version}";
            hash = "sha256-h+F3EAWQ1UQv7znCeoWwaIUXjuA9FLXiAbbbbefydp4=";
          };

          propagatedBuildInputs = with pythonVers; [
            sqlalchemy_1_4
          ] ++ [
            ownPkgs.sqlalchemy-utils
            pytz
            tzlocal
            click
            pymysql
            python-dateutil
          ];

          buildInputs = with pythonVers; [
            setuptools
          ];

          doCheck = false;

          meta = {
            description = "Python library for GnuCash file access";
            homepage = "https://github.com/sdementen/piecash";
            license = lib.licenses.gpl2Plus;
          };
        };

        ownPkgs.ixbrl-parse = pythonVers.buildPythonPackage rec {
          pname = "ixbrl-parse";
          version = "0.11.0";
          format = "pyproject";

          src = pkgs.fetchurl {
            url = "https://files.pythonhosted.org/packages/e7/98/b8e734723b2e310727cf14dac6d5e909eaaf6b58777c99824456e4230310/ixbrl_parse-0.11.0.tar.gz";
            hash = "sha256-atAEYX+0EVawkUhMXlOIohbMbRoZKMiZnwkW3qsxMFI=";
          };

          nativeBuildInputs = [
            pythonVers.setuptools
          ];

          propagatedBuildInputs = [
            pythonVers.lxml
          ];

          doCheck = false;
          dontCheckRuntimeDeps = true;

          meta = {
            description = "Python iXBRL parser";
            homepage = "https://github.com/cybermaggedon/ixbrl-parse";
            license = lib.licenses.gpl3Plus;
          };
        };

        ownPkgs.ixbrl-reporter =
          pythonVers.buildPythonPackage rec {
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
              pythonVers.requests
              pythonVers.lxml
              ownPkgs.piecash
              pythonVers.pyyaml
            ];

            nativeBuildInputs = [
              pythonVers.setuptools
              pythonVers.pytest
              pythonVers.pytest-cov
              pythonVers.pytest-mock
              ownPkgs.ixbrl-parse
            ];

            pythonImportsCheck = [ "ixbrl_reporter" ];

            meta = {
              description = "Production of iXBRL reports from templates and accounts files";
              homepage = "https://github.com/cybermaggedon/ixbrl-reporter";
              license = lib.licenses.gpl3Plus;
              platforms = lib.platforms.unix;
            };
            passthru = { inherit src versions mkPkg; };
          };

        ixbrl-src = ownPkgs.ixbrl-reporter.src;

        bash.wd = "$(git rev-parse --show-toplevel)";
        bin = inputs.my-nix.bin.${system} // (mapAttrs (n: p: "${p}/bin/${n}") scripts) // {
          ixbrl = "${ownPkgs.ixbrl-reporter}/bin/ixbrl-reporter";
        };
        scripts = with bash; mapAttrs (n: s: pkgs.writeShellScriptBin n s) {
          run = ''cargo run -- "$@" '';
          # packages = ''if [ -n "$CRATE" ]; then echo "-p $CRATE"; else echo "--workspace"; fi '';
          # utest = ''set -x; cargo nextest run $(packages) -E "''${TEST_FILTER:-all()}" --nocapture "$@" -- $SINGLE_TEST '';
          # ftest = ''set -x; cargo nextest run --workspace -E "''${TEST_FILTER:-all()}" --nocapture "$@" '';
          backup-txs = ''mkdir -p ./.cache/backup && mv ./.cache/starling_transactions.json ./.cache/backup/starling_transactions.$(date +%Y%m%d%H%M).json'';
          report = ''
            WD="${bash.wd}";
            cd ${ixbrl-src}
            ${bin.ixbrl} ${ixbrl-src}/config-corptax.yaml report ixbrl > "$WD/.cache/corp-tax.html"
          '';
          validate = ''arelleCmdLine -f "${wd}/.cache/ct_return_example2.html" -v'';
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
