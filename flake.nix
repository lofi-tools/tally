{
  description = "accounting";

  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, utils }:
    utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              (import rust-overlay)
              customRustOverlay
            ];
          };
          customRustOverlay = final: prev: {
            customRust = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain).override {
              extensions = [ "rust-src" ];
              targets = [ ];
            };
          };

          # static_ui = pkgs.stdenv.mkDerivation {
          #   name = "static_ui";
          #   src = ./static-ui/.;
          #   buildInputs = [ ];
          #   buildPhase = ''
          #     mkdir -p dist/
          #     cp -R ui dist/
          #   '';
          #   installPhase = ''
          #     mkdir $out
          #     cp -R dist/* $out/
          #   '';
          # };

          baseInputs = with pkgs; [ 
            pkgs.customRust
            # pkg-config
            # openssl
            # openssl.dev  
          ]++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            pkgs.darwin.apple_sdk.frameworks.CoreServices
            pkgs.darwin.apple_sdk.frameworks.CoreFoundation
            pkgs.darwin.apple_sdk.frameworks.Foundation
            pkgs.libiconv
          ];

          devInputs = with pkgs; [
            nixpkgs-fmt
            cargo-watch
            cargo-edit
            jq
            tmux
          ];


          server-bin = pkgs.rustPlatform.buildRustPackage rec  {
            name = "accounting";
            src = self;
            nativeBuildInputs = baseInputs;
            buildInputs = baseInputs;
            checkPhase = ''
              cargo fmt --all -- --check
              cargo clippy -- -D warnings
            '';
            buildPhase = ''
              mkdir -p dist/
              cargo build --release --bin server
            '';
            installPhase = ''
              mkdir $out
              cp -R ./target/release/server $out/
            '';
            # cargoSha256 = "";
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
          };

          ubuntuImage = pkgs.dockerTools.pullImage {
            imageName = "ubuntu";
            finalImageTag = "23.04";
            imageDigest = "sha256:52293638ba652a2e8f9e1c1cfcc905839b1f2a9e671ddcc9bf77909b6bf527d0";
            sha256 = "sha256-jcmKxrXel+CRLjpSi222HgVtliFO3BzrQiib7z5kyE8=";
          };

          containerImage = pkgs.dockerTools.buildImage {
            name = "accounting";
            tag = "latest";
            fromImage = ubuntuImage;
            copyToRoot = pkgs.buildEnv {
              name = "image-root";
              paths = [ server-bin ];
            };
            config.Cmd = [ "/server" ];
          };

        in
        {
          defaultPackage = server-bin;
          packages = {
            server = server-bin;
            docker = containerImage;
          };
          devShells.default = with pkgs; mkShell {
            name = "devshell__accounting";
            buildInputs = baseInputs ++ devInputs;
            RUST_LOG = "debug,actix_web=info";
          };
        }
      );
}




