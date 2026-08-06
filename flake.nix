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

        # HMRC Local Test Service (LTS) 8.3, with the CT600 RIM artefacts
        # bundled at build time.  The LTS Update Manager would normally
        # download the artefact zips from the services feed
        # (https://www.tpvs.hmrc.gov.uk/tools/v2/services.xml) and install
        # them into RIMArtefacts/<4-level term dir>, registering each service
        # in validatorConfig.xml; this derivation does exactly that (the feed
        # lists CT v3.9 -> CT/CT600/2014-2015 v2/3.99 and CT v1.994 ->
        # CT/CT600/2015-2016 v3/1.994, both under the CT/5 service uri, so
        # the newest is registered).
        ownPkgs.hmrc-lts = pkgs.stdenv.mkDerivation rec {
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

          # Run our Rust tally CLI over the example2 data (config + GnuCash
          # book), writing the CT600 GovTalk message to .cache/tally-cli/ct600-<number>.xml.
          tally-x2 = ''
            HERE="${bash.wd}"
            mkdir -p "$HERE/.cache/tally-cli"
            cargo run -p tally-cli -- ct600 \
              --config-path "$HERE/libs/ixbrl/example_data/example2/input_config.json" \
              --book "$HERE/libs/ixbrl/example_data/example2/input.gnucash" \
              --out "$HERE/.cache/tally-cli"
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
            ${pkgs.gawk}/bin/awk -v file="$HERE/libs/ixbrl/example_data/example2/input.gnucash" '
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
          validate = ''arelleCmdLine -f "${wd}/.cache/ixbrl-rs-tests/ct_return_example2.html" -v'';
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
            mkdir -p "$HERE/.cache/py-ixbrl-reporter" "$HERE/.cache/ixbrl-rs-tests" "$HERE/.cache/py-ct600"
            CONFIG="${ref-ct600.src}/config.json"
            ACCOUNTS="$HERE/.cache/py-ixbrl-reporter/accts-micro-gnucash.html"
            COMPUTATIONS="$HERE/.cache/ixbrl-rs-tests/ct_return_example2.html"
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

          # Regenerate the Rust-generated CT600 message (from the ct600
          # crate's generator) into .cache/ct600-rs-tests/ct600-example2.xml, by
          # running the generator test that writes it.  Runs in the devShell
          # (needs cargo).
          rct600-2 = ''
            set -eo pipefail

            rm -f .cache/ct600-rs-tests/ct600-example2.xml
            cargo test -p ct600 --lib ct600_return_from_example2_matches_reference
            [ -f .cache/ct600-rs-tests/ct600-example2.xml ] || {
              echo "rct600-rust: failed to generate .cache/ct600-rs-tests/ct600-example2.xml" >&2
              exit 1
            }
            echo "==> wrote .cache/ct600-rs-tests/ct600-example2.xml (our Rust ct600 generator)"
          '';

          # Start the HMRC Local Test Service (with bundled CT artefacts) and
          # submit the CT600 message to it, in a zellij session (via
          # l.mkZmux): the "lts" tab runs the server in the foreground (its
          # logs stay visible; its cleanup kills any stale instance first),
          # the "submit" tab waits for it to come up on :8081, POSTs
          # .cache/ct600-rs-tests/ct600-example2.xml (generated by our Rust ct600
          # generator) to /LTS/LTSPostServlet and prints the GovTalk
          # validation response.
          # The LTS writes logs/ + temp/ into its own directory, so a writable
          # copy (per store path) is kept under $XDG_CACHE_HOME/hmrc-lts.
          test-lts = '' ${l.mkZmux [
            {
              name = "lts";
              command = ''
                set -e
                HERE="${bash.wd}"
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
              cleanup = "${bin.rip} LTSStandalone";
            }
            {
              name = "submit";
              command = ''
                set -e
                HERE="${bash.wd}"
                PORT=8081
                FILE="$HERE/.cache/ct600-rs-tests/ct600-example2.xml"
                [ -f "$FILE" ] || {
                  echo "hmrc-lts-submit: missing input: $FILE" >&2
                  echo "  hint: run \`nix develop -c rct600-rust\` to regenerate it from the Rust ct600 generator" >&2
                  exit 1
                }
                echo "waiting for LTS on :$PORT ..."
                UP=0
                for i in $(seq 1 90); do
                  if curl -s -o /dev/null "http://localhost:$PORT/LTS"; then UP=1; break; fi
                  sleep 1
                done
                [ "$UP" = 1 ] || { echo "LTS not up after 90s" >&2; exit 1; }
                echo "==> submitting $FILE to http://localhost:$PORT/LTS/LTSPostServlet"
                curl -s -w '\n(http_code=%{http_code})\n' -H 'Content-Type: application/x-binary' \
                  --data-binary @"$FILE" "http://localhost:$PORT/LTS/LTSPostServlet" | tr -d '\r'
                echo
              '';
            }
          ]}
          echo "zellij session closed (tabs: lts, submit); the LTS has been stopped."
          echo "Re-run \`nix run .#hmrc-lts-submit\` to start again, or \`nix run .#hmrc-lts-stop\` to kill the server directly."
          '';

          # Kill the Local Test Service (also the lts tab's cleanup hook).
          kill-lts = ''${bin.rip} LTSStandalone'';
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
