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
    perSystem = { pkgs, system, l, ... }:
      let
        buildTimeDeps = [ pkgs.pkg-config ];
        runtimeDeps = [ ];
        # node + pnpm back the JS workspace (apps/design-system-showcase, apps/tally-web, packages/design-system).
        devDeps = [ pkgs.cargo-nextest pkgs.arelle pkgs.nodejs pkgs.pnpm ];

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

        # HMRC Local Test Service (LTS) 8.3, with the CT600 RIM artefacts
        # bundled at build time.  The LTS Update Manager would normally
        # download the artefact zips from the services feed
        # (https://www.tpvs.hmrc.gov.uk/tools/v2/services.xml) and install
        # them into RIMArtefacts/<4-level term dir>, registering each service
        # in validatorConfig.xml; this derivation does exactly that (the feed
        # lists CT v3.9 -> CT/CT600/2014-2015 v2/3.99 and CT v1.994 ->
        # CT/CT600/2015-2016 v3/1.994, both under the CT/5 service uri, so
        # the newest is registered).
        ownPkgs.hmrc-lts = pkgs.stdenv.mkDerivation {
          pname = "hmrc-lts";
          version = "8.3";
          dontUnpack = true;
          src = pkgs.fetchurl {
            url = "https://www.tpvs.hmrc.gov.uk/tools/v2/LTS8.3.zip";
            hash = "sha256-QjUull+MTuHRFWU4YGGKtFFx333zYk80/4XEcpmQVnQ=";
          };
          ct2009 = pkgs.fetchurl {
            url = "https://www.tpvs.hmrc.gov.uk/tools/v2/ct_ct600_v3-9.zip";
            hash = "sha256-VyPnRUeFawcA51FeZrG/oxgzc6G6Rl+EnUH05oiAVcc=";
          };
          ct2014 = pkgs.fetchurl {
            url = "https://www.tpvs.hmrc.gov.uk/tools/v2/ct_ct600_v1-994.zip";
            hash = "sha256-a6lR6DDll8aYEfGJCJzQufiQGAtzrYRd16/O8D5UPzU=";
          };
          nativeBuildInputs = [ pkgs.unzip pkgs.python3 ];
          installPhase = ''
            runHook preInstall
            mkdir -p "$out"
            ${pkgs.unzip}/bin/unzip -q "$src" 'HMRCTools/*' -d "$out"
            rm -rf "$out/__MACOSX" "$out/HMRCTools/.DS_Store"

            # Install the CT600 RIM artefacts into the same 4-level directory
            # layout the Update Manager would produce from the feed terms.
            mkdir -p "$out/HMRCTools/RIMArtefacts/CT/CT600/2014-2015 v2/3.99" \
                     "$out/HMRCTools/RIMArtefacts/CT/CT600/2015-2016 v3/1.994"
            (cd "$out/HMRCTools/RIMArtefacts/CT/CT600/2014-2015 v2/3.99" && ${pkgs.unzip}/bin/unzip -q "$ct2009")
            (cd "$out/HMRCTools/RIMArtefacts/CT/CT600/2015-2016 v3/1.994" && ${pkgs.unzip}/bin/unzip -q "$ct2014")

            # Register the CT service in validatorConfig.xml, mirroring the
            # Update Manager's ConfigUpdate step when installing an artefact.
            ${pkgs.python3}/bin/python3 -c '
            import sys
            p = sys.argv[1]
            xml = open(p, encoding="utf-8", errors="replace").read()
            svc = "\t\t<Service uri=\"http://www.govtalk.gov.uk/taxation/CT/5\">\n" \
                  "\t\t\t<TotalErrorCap>100</TotalErrorCap>\n" \
                  "\t\t\t<ValidationType>COMPLETE</ValidationType>\n" \
                  "\t\t\t<RIMArtefactsDirectory>CT/CT600/2015-2016 v3/1.994</RIMArtefactsDirectory>\n" \
                  "\t\t</Service>\n\t"
            assert "\t</Envelope>" in xml, "Envelope close not found"
            xml = xml.replace("\t</Envelope>", svc + "\t</Envelope>")
            open(p, "w", encoding="utf-8").write(xml)
            print("registered CT service in validatorConfig.xml")
            ' "$out/HMRCTools/LTS/resources/config/NonConfigurable/validatorConfig.xml"

            # Default the web server port to 8081 (matches the ct600
            # config-test.json url http://localhost:8081/).
            ${pkgs.python3}/bin/python3 -c '
            import sys
            p = sys.argv[1]
            xml = open(p, encoding="utf-8", errors="replace").read()
            assert "default=\"5665\"" in xml, "port default not found"
            xml = xml.replace("default=\"5665\"", "default=\"8081\"")
            open(p, "w", encoding="utf-8").write(xml)
            print("LTS port default set to 8081")
            ' "$out/HMRCTools/LTS/resources/config/UserConfigurable/LTSConfig.xml"

            runHook postInstall
          '';
          meta = {
            description = "HMRC Local Test Service 8.3 with bundled CT600 RIM artefacts";
            homepage = "https://www.tpvs.hmrc.gov.uk/tools/v2/";
            platforms = lib.platforms.unix;
          };
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

          # The Tally web app only (Vite + Solid): install the JS workspace
          # deps, run Panda codegen, start Vite on :5173.  pnpm/node are
          # referenced from the flake, so this works both in the devShell
          # (`nix develop -c web`) and standalone (`nix run .#web`).  Vite
          # runs directly (via exec) rather than through `pnpm dev`, so
          # stopping the server with Ctrl+C shuts down cleanly instead of
          # pnpm reporting the signal as a failed run (exit 143 / "Command
          # failed with signal").
          web = ''
            set -e
            cd "${wd}"
            "${pkgs.pnpm}/bin/pnpm" install
            cd apps/tally-web
            ./node_modules/.bin/panda codegen
            ./node_modules/.bin/panda cssgen
            exec "${pkgs.nodejs}/bin/node" node_modules/vite/bin/vite.js
          '';

          # The full dev stack in one zellij session (via l.mkZmux): postgres
          # (db tab), the tally-api (api tab), and the Vite web app (web tab)
          # — one tab per process so each one's logs stay visible.  Only the
          # db tab runs `docker compose up` (it owns the container); the api
          # tab waits for the healthcheck via db-wait (read-only, no race)
          # before `cargo run`.  The db tab's cleanup stops the container
          # before starting (any stale instance) and again when the session
          # ends (data persists in the named volume, so the db survives
          # restarts).  Run from the devShell — the
          # api tab needs cargo: `nix develop -c dev` (or
          # `nix develop -c nix run .#dev`).
          dev = ''
            cd "${wd}"
            ${l.mkZmux [
              {
                name = "db";
                command = "docker compose up db";
                cleanup = "docker compose stop db 2>/dev/null || true";
              }
              {
                name = "api";
                command = ''
                  ${bin.db-wait}
                  cargo run -p tally-api
                '';
              }
              { name = "web"; command = "${bin.web}"; }
            ]}
          '';

          # Run our Rust tally CLI over the basic-1 data (config + GnuCash
          # book), writing the CT600 GovTalk message to .cache/tally-cli/ct600-<number>.xml.
          ex2 = ''
            cargo run -p tally-cli -- ct600 \
              --config-path "${wd}/libs/ixbrl/example_data/basic-1/input_config.jsonc" \
              --book "${wd}/libs/ixbrl/example_data/basic-1/input.gnucash"
          '';
          # packages = ''if [ -n "$CRATE" ]; then echo "-p $CRATE"; else echo "--workspace"; fi '';
          # utest = ''set -x; cargo nextest run $(packages) -E "''${TEST_FILTER:-all()}" --nocapture "$@" -- $SINGLE_TEST '';
          # ftest = ''set -x; cargo nextest run --workspace -E "''${TEST_FILTER:-all()}" --nocapture "$@" '';
          backup-txs = ''mkdir -p ./.cache/backup && mv ./.cache/starling_transactions.json ./.cache/backup/starling_transactions.$(date +%Y%m%d%H%M).json'';

          rixsrc = ''printf "%s\n" ${ref-ixbrl.src}'';
          racc = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            mkdir -p "$HERE/.cache/py-ixbrl-reporter"
            ${bin.ref-ixbrl} ${ref-ixbrl.src}/config.yaml report ixbrl > $HERE/.cache/py-ixbrl-reporter/accts-micro.html '';

          # Run the reference accounts report on the example GnuCash book.
          racc-gnucash = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            mkdir -p "$HERE/.cache/py-ixbrl-reporter"
            ${pkgs.gawk}/bin/awk -v file="$HERE/libs/ixbrl/example_data/basic-1/input.gnucash" '
              /^accounts:/ { print "accounts:"; print "  kind: piecash"; print "  file: " file; f=1; next }
              /^report:/ { f=0 }
              !f
            ' config.yaml > "$HERE/.cache/py-ixbrl-reporter/config-gnucash.yaml"
            ${bin.ref-ixbrl} "$HERE/.cache/py-ixbrl-reporter/config-gnucash.yaml" report ixbrl > "$HERE/.cache/py-ixbrl-reporter/accts-micro-gnucash.html"
          '';

          # Run the reference accounts report on the example CSV accounts.
          racc-csv = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            mkdir -p "$HERE/.cache/py-ixbrl-reporter"
            ${pkgs.gawk}/bin/awk -v file="$HERE/libs/ixbrl/example_data/example3.csv" '
              /^accounts:/ { print "accounts:"; print "  kind: csv"; print "  file: " file; f=1; next }
              /^report:/ { f=0 }
              !f
            ' config.yaml > "$HERE/.cache/py-ixbrl-reporter/config-csv.yaml"
            ${bin.ref-ixbrl} "$HERE/.cache/py-ixbrl-reporter/config-csv.yaml" report ixbrl > "$HERE/.cache/py-ixbrl-reporter/accts-micro-csv.html"
          '';
          report = ''
            HERE="${bash.wd}"; cd ${ref-ixbrl.src}
            mkdir -p "$HERE/.cache/py-ixbrl-reporter"
            ${bin.ref-ixbrl} ${ref-ixbrl.src}/config-corptax.yaml report ixbrl > "$HERE/.cache/py-ixbrl-reporter/corp-tax.html"
          '';
          validate = ''arelleCmdLine -f "${wd}/.cache/ixbrl-rs-tests/ct_return_basic-1.html" -v --validationExitCode --captureWarnings'';

          # E2E: regenerate every report the Rust test suite produces, then
          # validate each one with Arelle.  Prints the path before
          # validating it and stops at the first failing report (arelle
          # exits 3 on validation errors or warnings via
          # --validationExitCode --captureWarnings).  Runs in the devShell
          # (needs cargo + arelle): `nix develop -c validate-all`.
          validate-all = ''
            set -e
            cargo test -p ixbrl --lib
            for f in \
              accts-micro-basic-1.html \
              accts-micro-roundtrip-basic-1.html \
              ct_return_basic-1.html \
              ct_roundtrip_basic-1.html
            do
              path="${wd}/.cache/ixbrl-rs-tests/$f"
              [ -f "$path" ] || { echo "validate-all: missing report: $path" >&2; exit 1; }
              echo "==> validating $path"
              # --captureWarnings: warnings fail the run too (exit 3).
              arelleCmdLine -f "$path" -v --validationExitCode --captureWarnings
            done
            echo "all reports validate OK"
          '';
          rct600 = ''${bin.ref-ct600} "$@" '';

          # Run the reference ct600 tool over the basic-1 pair: step 1
          # generates the CT600 form-values YAML from the computations iXBRL,
          # step 2 renders the CT message (--output-ct, no submission) into
          # .cache.  Inputs default to the basic-1 pair but can be
          # overridden:
          #   rct600-run [--config F] [--accounts F] [--computations F] [--form-values F]
          rct600-run = ''
            set -e
            HERE="${bash.wd}"
            mkdir -p "$HERE/.cache/py-ixbrl-reporter" "$HERE/.cache/ixbrl-rs-tests" "$HERE/.cache/py-ct600"
            CONFIG="${ref-ct600.src}/config.json"
            ACCOUNTS="$HERE/.cache/py-ixbrl-reporter/accts-micro-gnucash.html"
            COMPUTATIONS="$HERE/.cache/ixbrl-rs-tests/ct_return_basic-1.html"
            FORM_VALUES="$HERE/.cache/py-ct600/form-values.yaml"
            OUT="$HERE/.cache/py-ct600/ct600.xml"
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

          # Regenerate the Rust-generated CT600 messages (from the ct600
          # crate's generator tests) into .cache/ct600-rs-tests/: the basic-1
          # message and the ctm03955-marginal-relief loss-company message.
          # Runs in the devShell (needs cargo).
          rct600-2 = ''
            set -eo pipefail

            rm -f \
              .cache/ct600-rs-tests/ct600-basic-1.xml \
              .cache/ct600-rs-tests/ct600-ctm03955-losses.xml
            cargo test -p ct600 --lib ct600_return_from_basic_1_matches_reference
            cargo test -p ct600 --lib ct600_ctm03955_loss_company_message_generates
            for f in \
              .cache/ct600-rs-tests/ct600-basic-1.xml \
              .cache/ct600-rs-tests/ct600-ctm03955-losses.xml
            do
              [ -f "$f" ] || {
                echo "rct600-2: failed to generate $f" >&2
                exit 1
              }
            done
            echo "==> wrote the Rust ct600 generator messages in .cache/ct600-rs-tests/"
          '';

          # Start the HMRC Local Test Service (with bundled CT artefacts):
          # unpack it into a writable per-store-path cache copy and run the
          # server in the foreground.  The LTS writes logs/ + temp/ into its
          # own directory, hence the copy under $XDG_CACHE_HOME/hmrc-lts.
          hmrc-lts-run = ''
            set -e
            LTS="${ownPkgs.hmrc-lts}"
            CACHE_ROOT="''${XDG_CACHE_HOME:-$HOME/.cache}/hmrc-lts"
            CACHE="$CACHE_ROOT/$(basename "$LTS")"
            if [ ! -d "$CACHE/LTS" ]; then
              mkdir -p "$CACHE"
              cp -r "$LTS/HMRCTools"/. "$CACHE/"
              chmod -R u+w "$CACHE"
            fi
            export PATH="${pkgs.jdk21}/bin:$PATH"
            cd "$CACHE/LTS"
            echo "LTS starting on http://localhost:8081/LTS ..."
            LTS_HOME="$PWD" sh RunLTSStandalone.sh
          '';

          # Wait for a running Local Test Service to accept connections on
          # :8081 (shared by the submit flows).
          hmrc-lts-wait = ''
            set -e
            PORT=8081
            echo "waiting for LTS on :$PORT ..."
            UP=0
            for i in $(seq 1 90); do
              if curl -s -o /dev/null "http://localhost:$PORT/LTS"; then UP=1; break; fi
              sleep 1
            done
            [ "$UP" = 1 ] || { echo "hmrc-lts-wait: LTS not up after 90s" >&2; exit 1; }
          '';

          # Submit every Rust ct600 test message
          # (.cache/ct600-rs-tests/ct600-*.xml) to a running LTS.  For each
          # file: print its path, POST it to /LTS/LTSPostServlet, print the
          # GovTalk response, and stop at the first validation failure (the
          # response carries an ErrorResponse envelope), so the failing file
          # and its errors stay on screen.
          hmrc-lts-submit = ''
            set -e
            ${bin.hmrc-lts-wait}
            shopt -s nullglob
            FILES=("${wd}"/.cache/ct600-rs-tests/ct600-*.xml)
            [ ''${#FILES[@]} -gt 0 ] || {
              echo "hmrc-lts-submit: no ct600-*.xml messages in ${wd}/.cache/ct600-rs-tests" >&2
              echo "  hint: run \`nix develop -c rct600-2\` to generate them from the Rust ct600 generator" >&2
              exit 1
            }
            for FILE in "''${FILES[@]}"; do
              echo "==> submitting $FILE"
              RESP="$(curl -s -w '\n(http_code=%{http_code})\n' -H 'Content-Type: application/x-binary' \
                --data-binary @"$FILE" "http://localhost:8081/LTS/LTSPostServlet" | tr -d '\r')"
              printf '%s\n' "$RESP"
              if printf '%s' "$RESP" | grep -qi 'ErrorResponse'; then
                echo "hmrc-lts-submit: LTS validation FAILED for $FILE" >&2
                exit 1
              fi
            done
            echo "==> all messages accepted by the LTS"
          '';

          # Full LTS round-trip: regenerate the Rust ct600 test messages
          # first (so the submits always exercise the latest generators),
          # then start LTS + submit them together, in a zellij session (via
          # l.mkZmux): the "lts" tab runs `nix run .#hmrc-lts-run` in the
          # foreground (its logs stay visible; its cleanup kills any stale
          # instance first), the "submit" tab runs `nix run .#hmrc-lts-submit`
          # once the server is up.  Needs the devShell (runs `cargo test`
          # via rct600-2): `nix develop -c nix run .#test-lts`.
          test-lts = ''
            set -e
            echo "==> regenerating the Rust ct600 test messages"
            ${bin.rct600-2}
            ${l.mkZmux [
              { name = "lts"; command = "${bin.hmrc-lts-run}"; cleanup = "${bin.hmrc-lts-stop}"; }
              { name = "submit"; command = "${bin.hmrc-lts-submit}"; }
            ]}
          '';

          # Kill the Local Test Service (also the lts tab's cleanup hook).
          hmrc-lts-stop = ''${bin.rip} LTSStandalone'';

          # The tally-api dev database: start the compose postgres and wait
          # for its healthcheck (`--wait` needs docker compose v2).  Standalone
          # use, or from `apitest`.
          dev-db = ''cd "${wd}"; docker compose up -d --wait db'';
          db-down = ''cd "${wd}"; docker compose down'';

          # Wait until the compose postgres reports healthy — used by the `dev`
          # api tab so `cargo run` starts only once the db is ready.  Read-only
          # (`docker compose ps` + `docker inspect`, never `up`): unlike
          # dev-db it cannot create a container, so it can't race the db tab's
          # foreground `docker compose up db` over the container name.
          db-wait = ''
            cd "${wd}"
            command -v docker >/dev/null 2>&1 || {
              echo "db-wait: docker not available; the api will fail to reach the db" >&2
              exit 0
            }
            UP=0
            for i in $(seq 1 90); do
              CID=$(docker compose ps -q db 2>/dev/null | head -1)
              if [ -n "$CID" ] && [ "$(docker inspect -f '{{.State.Health.Status}}' "$CID" 2>/dev/null)" = healthy ]; then
                UP=1; break
              fi
              sleep 1
            done
            [ "$UP" = 1 ] || { echo "db-wait: db not healthy after 90s; starting the api anyway" >&2; }
          '';

          # Wipe the dev database (compose container + named volume) and any
          # uploaded ledger files — a clean slate.  The next `nix develop -c
          # dev` recreates the db and its schema from scratch.  The web app's
          # localStorage is browser-side and untouched; clear it in devtools
          # to also reset the UI to the empty onboarding state.  (Stop a
          # running `dev` session first — its db tab holds the container.)
          reset = ''
            cd "${wd}"
            docker compose down -v --remove-orphans
            # Belt-and-braces: force-remove any leftover container/volume that
            # compose couldn't clean up (e.g. a stale instance mid-race), so a
            # later `dev` can't hit a "container name already in use" conflict.
            docker rm -f accounting-db-1 2>/dev/null || true
            docker volume rm tally-pg 2>/dev/null || true
            rm -rf "${wd}/.cache/tally-api/uploads"
          '';

          # Run the tally-api service (env from .env / defaults; see
          # apps/tally-api/README.md).
          api = ''cargo run -p tally-api'';

          # The tally-api suites.  `test-api` is self-sufficient: it first
          # ensures the compose Postgres is up (`docker compose up -d --wait db`
          # is idempotent), then runs the full suite.  If docker isn't
          # available the pg-gated tests print a notice and skip rather than
          # failing; `test-api-offline` is the first-clone / DB-less variant.
          apitest = ''
            cd "${wd}"
            docker compose up -d --wait db 2>/dev/null \
              || echo "warning: docker db not available; pg-gated tests will skip"
            cargo test -p tally-api
          '';
          test-api-offline = ''cargo test -p tally-api --no-default-features'';
        };

        env = {
          CARGO_NET_GIT_FETCH_WITH_CLI = "true";
          # Default log filter for the dev shell: the app + http layers at
          # info, the toasty postgres driver crates at debug (the
          # `toasty::query` events render each evaluated request — SQL +
          # params — for every query).  main.rs reads RUST_LOG via
          # EnvFilter::try_from_default_env, so a shell-set RUST_LOG fully
          # overrides this default.
          RUST_LOG = "tally_api=info,tower_http=info,toasty_driver_postgresql=debug,toasty::query=debug";
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
