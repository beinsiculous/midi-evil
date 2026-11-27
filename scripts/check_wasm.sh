#!/usr/bin/env bash
# The browser build's compile gate: the application and its web entry must keep compiling,
# linting clean and linking for wasm32-unknown-unknown, without the gate feature and with it, so a
# native change cannot silently break either browser build. The workflow in .github/workflows/wasm-check.yml runs exactly this script,
# so the local command and the CI gate cannot drift.
#
# It deliberately does not:
#   - lint the shared crates for the browser: `--no-deps` keeps clippy to the two packages named
#     here, because the shared crates still read and write files by path, and making them clean
#     for wasm32 is the work that changes them;
#   - pass --all-targets: the tests are native, and their dev-dependencies are not the bundle;
#   - bind, bundle, measure or run anything in a browser: that is scripts/build_wasm.sh and the
#     smoke test, and neither runs in CI.
#
# Needs the target installed: rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/.."

packages=(-p midi-evil -p evil-web)
target=(--target wasm32-unknown-unknown)

# Each feature set in its own cargo target directory: the visitor's build in the default one, and
# the gate's, which compiles the gate's tracked projects in, in one of its own, as
# scripts/build_wasm.sh --gate keeps it.
check() {
    local name=$1
    shift
    echo "==> $name: cargo check ${packages[*]} ${target[*]} $*"
    cargo check --locked "${packages[@]}" "${target[@]}" "$@"

    echo "==> $name: cargo clippy ${packages[*]} --no-deps ${target[*]} $* -- -D warnings"
    cargo clippy --locked "${packages[@]}" --no-deps "${target[@]}" "$@" -- -D warnings

    # A type check never runs the linker; building the entry does.
    echo "==> $name: cargo build -p evil-web ${target[*]} $*"
    cargo build --locked -p evil-web "${target[@]}" "$@"
}

check "the visitor's build"
check "the gate's build" --features evil-web/gate --target-dir target/wasm-check-gate

echo "wasm gate: clean, both feature sets"
