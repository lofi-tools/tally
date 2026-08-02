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

        ownPkgs.number-parser = pythonVers.buildPythonPackage {
          pname = "number-parser";
          version = "0.3.2";
          format = "setuptools";

          src = pkgs.fetchurl {
            url = "https://files.pythonhosted.org/packages/28/c1/1a3ea159327b442d2202fda38845124a51a3abe11637cbd3111479fd815f/number-parser-0.3.2.tar.gz";
            hash = "sha256-dlDpGv1G3sL0A5b2LCuryEZFZ5ocqOzbUpK90lJoXWo=";
          };

          propagatedBuildInputs = [
            pythonVers.attrs
          ];

          doCheck = false;

          meta = {
            description = "Convert natural language number expressions into ints and floats";
            homepage = "https://github.com/scrapinghub/number-parser";
            license = lib.licenses.bsd3;
          };
        };

        ownPkgs.ixbrl-parse = pythonVers.buildPythonPackage {
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
            pythonVers.requests
            pythonVers.rdflib
            ownPkgs.number-parser
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

            # The dev shell exports a python3.13 PYTHONPATH (e.g. from arelle)
            # which would shadow this python3.12 package's own site-packages at
            # runtime (lxml's compiled extensions are version-specific), so the
            # generated wrapper must not inherit it.
            makeWrapperArgs = [ "--unset" "PYTHONPATH" ];

            pythonImportsCheck = [ "ixbrl_reporter" ];

            meta = {
              description = "Production of iXBRL reports from templates and accounts files";
              homepage = "https://github.com/cybermaggedon/ixbrl-reporter";
              license = lib.licenses.gpl3Plus;
              platforms = lib.platforms.unix;
            };
            passthru = { inherit src versions mkPkg; };
          };

        ownPkgs.ct600-py = pythonVers.buildPythonPackage rec {
          pname = "ct600";
          # v1.4.1 is the last release on the 2023 taxonomies (ct-comp 2023,
          # FRC core 2023), matching the iXBRL emitted by ixbrl-reporter 1.2.1
          # and this repo's Rust generators.  v1.4.2+ moved to the CT 2024 / FRC
          # 2025 taxonomies and cannot parse those files (nor even its own
          # bundled example).
          version = "1.4.1";
          format = "pyproject";

          src = pkgs.fetchFromGitHub {
            owner = "cybermaggedon";
            repo = "ct600";
            rev = "v${version}";
            hash = "sha256-LUA1O1RhvXSXvXCxEGQRpumJH1je9jwE34ecQGwT0cc=";
          };
          nativeBuildInputs = [
            pythonVers.setuptools
          ];
          propagatedBuildInputs = [
            pythonVers.aiohttp
            pythonVers.requests
            pythonVers.pyaml
            pythonVers.lxml
            ownPkgs.ixbrl-parse
          ];

          # Same as ixbrl-reporter: the dev shell's python3.13 PYTHONPATH would
          # shadow this python3.12 package's site-packages at runtime.
          makeWrapperArgs = [ "--unset" "PYTHONPATH" ];

          # py-dmidecode is declared upstream but unused in the code, and is
          # Linux-only in nixpkgs, so it's omitted here.
          dontCheckRuntimeDeps = true;
          pythonImportsCheck = [ "ct600" ];

          meta = {
            description = "UK HMRC Corporation Tax submission";
            homepage = "https://github.com/cybermaggedon/ct600";
            license = lib.licenses.gpl3Plus;
            platforms = lib.platforms.unix;
          };
          passthru = { inherit src version; };
        };

        ref-ixbrl = ownPkgs.ixbrl-reporter;
        ref-ct600 = ownPkgs.ct600-py;

        bash.wd = "$(git rev-parse --show-toplevel)";
        bin = inputs.my-nix.bin.${system} // (mapAttrs (n: p: "${p}/bin/${n}") scripts) // {
          ref-ixbrl = "${ownPkgs.ixbrl-reporter}/bin/ixbrl-reporter";
          ref-ct600 = "${ownPkgs.ct600-py}/bin/ct600";
        };
        scripts = with bash; mapAttrs pkgs.writeShellScriptBin {
          run = ''cargo run -- "$@" '';
          # packages = ''if [ -n "$CRATE" ]; then echo "-p $CRATE"; else echo "--workspace"; fi '';
          # utest = ''set -x; cargo nextest run $(packages) -E "''${TEST_FILTER:-all()}" --nocapture "$@" -- $SINGLE_TEST '';
          # ftest = ''set -x; cargo nextest run --workspace -E "''${TEST_FILTER:-all()}" --nocapture "$@" '';
          backup-txs = ''mkdir -p ./.cache/backup && mv ./.cache/starling_transactions.json ./.cache/backup/starling_transactions.$(date +%Y%m%d%H%M).json'';

          rixsrc = ''printf "%s\n" ${ref-ixbrl.src}'';
          racc = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            ${bin.ref-ixbrl} ${ref-ixbrl.src}/config.yaml report ixbrl > $HERE/.cache/accts-micro.html '';

          # Run the reference accounts report on the example GnuCash book.
          racc-gnucash = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            mkdir -p "$HERE/.cache"
            ${pkgs.gawk}/bin/awk -v file="$HERE/libs/ixbrl/example_data/example2/input.gnucash" '
              /^accounts:/ { print "accounts:"; print "  kind: piecash"; print "  file: " file; f=1; next }
              /^report:/ { f=0 }
              !f
            ' config.yaml > "$HERE/.cache/config-gnucash.yaml"
            ${bin.ref-ixbrl} "$HERE/.cache/config-gnucash.yaml" report ixbrl > "$HERE/.cache/accts-micro-gnucash.html"
          '';

          # Run the reference accounts report on the example CSV accounts.
          racc-csv = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            mkdir -p "$HERE/.cache"
            ${pkgs.gawk}/bin/awk -v file="$HERE/libs/ixbrl/example_data/example3.csv" '
              /^accounts:/ { print "accounts:"; print "  kind: csv"; print "  file: " file; f=1; next }
              /^report:/ { f=0 }
              !f
            ' config.yaml > "$HERE/.cache/config-csv.yaml"
            ${bin.ref-ixbrl} "$HERE/.cache/config-csv.yaml" report ixbrl > "$HERE/.cache/accts-micro-csv.html"
          '';
          report = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            ${bin.ref-ixbrl} ${ref-ixbrl.src}/config-corptax.yaml report ixbrl > "$HERE/.cache/corp-tax.html"
          '';
          validate = ''arelleCmdLine -f "${wd}/.cache/ct_return_example2.html" -v'';
          rct600 = ''${bin.ref-ct600} "$@" '';

          # Run the reference ct600 tool over the example2 pair: step 1
          # generates the CT600 form-values YAML from the computations iXBRL,
          # step 2 renders the CT message (--output-ct, no submission) into
          # .cache.  Inputs default to the example2 pair but can be
          # overridden:
          #   rct600-run [--config F] [--accounts F] [--computations F] [--form-values F]
          rct600-run = ''
            set -e
            HERE="${bash.wd}"
            mkdir -p "$HERE/.cache"
            CONFIG="${ref-ct600.src}/config.json"
            ACCOUNTS="$HERE/.cache/accts-micro-gnucash.html"
            COMPUTATIONS="$HERE/.cache/ct_return_example2.html"
            FORM_VALUES="$HERE/.cache/form-values.yaml"
            OUT="$HERE/.cache/ct600.xml"
            while [ $# -gt 0 ]; do
              case "$1" in
                --config) CONFIG="''${2:?missing value for $1}"; shift 2 ;;
                --accounts) ACCOUNTS="''${2:?missing value for $1}"; shift 2 ;;
                --computations|--comps) COMPUTATIONS="''${2:?missing value for $1}"; shift 2 ;;
                --form-values) FORM_VALUES="''${2:?missing value for $1}"; shift 2 ;;
                *) echo "rct600-run: unknown option: $1" >&2; exit 1 ;;
              esac
            done
            for f in "$CONFIG" "$ACCOUNTS" "$COMPUTATIONS"; do
              [ -f "$f" ] || {
                echo "rct600-run: missing input: $f" >&2
                echo "  hint: run \`racc-gnucash\` first, or pass --config/--accounts/--computations" >&2
                exit 1
              }
            done
            echo "==> 1/2 form values -> $FORM_VALUES"
            ${bin.ref-ct600} --computations "$COMPUTATIONS" --output-form-values > "$FORM_VALUES"
            echo "==> 2/2 CT message (no submission) -> $OUT"
            ${bin.ref-ct600} \
              --config "$CONFIG" \
              --accounts "$ACCOUNTS" \
              --computations "$COMPUTATIONS" \
              --form-values "$FORM_VALUES" \
              --output-ct > "$OUT"
            echo "Tip: fill boxes 975/980/985 (declaration name/date/status) in $FORM_VALUES before filing;"
            echo "     note re-running this script regenerates that file from the computations."
          '';
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
