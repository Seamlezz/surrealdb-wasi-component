# Script names are shared with the other repos of this stack. They avoid `test` and `fmt`,
# which lose to the shell builtin and coreutils.
# `devenv shell` gives Rust (rust-toolchain.toml), wkg and the scripts below.
# `devenv test` runs the full local check.
{ config, pkgs, ... }:

let
  ociPackage = "ghcr.io/seamlezz/surrealdb";
  ociFile = "dist/seamlezz-surrealdb.wasm";
in
{
  languages.rust = {
    enable = true;
    toolchainFile = ./rust-toolchain.toml;
  };

  env.WKG_CONFIG_FILE = "${config.devenv.root}/wkg-config.toml";

  # curl and jq are for .github/scripts.
  packages = [ pkgs.curl pkgs.git pkgs.jq pkgs.wkg ];

  scripts = {
    wit-fetch = {
      description = "Fetch WIT dependencies; updates wkg.lock";
      exec = ''cd "$DEVENV_ROOT" && wkg wit fetch --wit-dir wit'';
    };
    format = {
      description = "Format all Rust code";
      exec = ''cd "$DEVENV_ROOT" && cargo fmt --all'';
    };
    format-check = {
      description = "Fail when Rust code is not formatted";
      exec = ''cd "$DEVENV_ROOT" && cargo fmt --all --check'';
    };
    lint = {
      description = "Run clippy";
      exec = ''cd "$DEVENV_ROOT" && cargo clippy --all-targets --all-features -- -D warnings'';
    };
    tests = {
      description = "Run the workspace tests";
      exec = ''cd "$DEVENV_ROOT" && cargo test --workspace'';
    };
    test-examples = {
      description = "Run the guest demo through host-wasmtime against in memory SurrealDB";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo build -p guest-demo --target wasm32-wasip2
        cargo run -p host-wasmtime --features surrealdb-host-adapter/debug-logs -- target/wasm32-wasip2/debug/guest_demo.wasm
      '';
    };
    build-host = {
      description = "Build the host adapter";
      exec = ''cd "$DEVENV_ROOT" && cargo build -p surrealdb-host-adapter --features debug-logs'';
    };
    build-sdk = {
      description = "Build the guest SDK";
      exec = ''cd "$DEVENV_ROOT" && cargo build -p surrealdb-component-sdk --target wasm32-wasip2'';
    };
    build-demo = {
      description = "Build the guest demo";
      exec = ''cd "$DEVENV_ROOT" && cargo build -p guest-demo --target wasm32-wasip2'';
    };
    build = {
      description = "Build the host adapter, the SDK and the demo";
      exec = ''
        set -euo pipefail
        build-host
        build-sdk
        build-demo
      '';
    };
    check = {
      description = "Fetch WIT, format check, lint, test and build everything";
      exec = ''
        set -euo pipefail
        wit-fetch
        format-check
        lint
        tests
        test-examples
        build
      '';
    };
    ghcr-login = {
      description = "Log Docker in to ghcr.io with GHCR_USERNAME and GHCR_TOKEN; wkg uses those credentials";
      exec = ''printf '%s' "$GHCR_TOKEN" | docker login ghcr.io --username "$GHCR_USERNAME" --password-stdin'';
    };
    release-dry-run = {
      description = "Build the WIT package without pushing it";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        tag="''${1:-0.1.0}"
        mkdir -p dist
        wkg wit build --wit-dir wit --output ${ociFile}
        test -f ${ociFile}
        echo "Prepared artifact ${ociFile} for ${ociPackage}:$tag"
      '';
    };
    release-publish = {
      description = "Build the WIT package and push it as <version>";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        version="''${1:?usage: release-publish <semver>}"
        if ! grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$' <<<"$version"; then
          echo "release-publish: $version is not a semver string" >&2
          exit 1
        fi
        mkdir -p dist
        wkg wit build --wit-dir wit --output ${ociFile}
        wkg oci push "${ociPackage}:$version" ${ociFile}
      '';
    };
  };

  enterTest = "check";
}
