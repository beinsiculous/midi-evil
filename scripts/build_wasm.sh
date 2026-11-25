#!/usr/bin/env bash
# Builds MIDI Evil's browser bundle into target/web/dist/: the web entry compiled with the
# wasm-release profile, bound by the wasm-bindgen CLI, beside crates/evil-web/index.html; then
# prints the wasm's size, raw and gzipped, and fails when it is over the budget below.
#
# The CLI must be the version Cargo.lock resolves for the wasm-bindgen crate, or it binds the
# module wrongly without saying so. This script keeps its own under this repository's ignored
# target/tools/, installing the lock's version there when none is there or the one there is any
# other version, and calls only that one by its path: the wasm-bindgen on PATH belongs to other
# projects with other pins, and is never called, replaced or installed over.
#
# The bundle is made in a directory of its own and replaces target/web/dist/ only once the build,
# the binding, the copy of the page and the size budget have all passed, so a failed run leaves
# the last good bundle as it was.
#
# It deliberately does not serve the bundle, open a browser, or copy the bundle anywhere else —
# in particular not into the site: the smoke test serves its own copy, and the site receives a
# bundle in a later, separate step.
#
#   scripts/build_wasm.sh
#
# Needs the target installed (rustup target add wasm32-unknown-unknown), and the network the first
# time, when the CLI is installed.
set -euo pipefail

# The ceiling for the raw wasm, in mebibytes: the first good build's size rounded up to the next
# mebibyte, plus one; that build was 5.94 MiB. A batch that needs more raises this number and says
# why; it is never only a warning.
SIZE_BUDGET_MEBIBYTES=7

repository=$(cd "$(dirname "$0")/.." && pwd)
target_directory=$repository/target
tools_root=$target_directory/tools
wasm_bindgen=$tools_root/bin/wasm-bindgen
web_directory=$target_directory/web
dist=$web_directory/dist
page=$repository/crates/evil-web/index.html

lock_version=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3; exit }' \
    "$repository/Cargo.lock")
if [ -z "$lock_version" ]; then
    echo "ERROR: Cargo.lock resolves no wasm-bindgen crate for the CLI to match." >&2
    exit 1
fi

installed_version() {
    if [ -x "$wasm_bindgen" ]; then
        "$wasm_bindgen" --version 2> /dev/null | awk '{ print $2 }' || true
    fi
}

if [ "$(installed_version)" != "$lock_version" ]; then
    echo "==> installing wasm-bindgen-cli $lock_version under $tools_root"
    # --force, or cargo refuses to replace a binary of another version that is already there.
    if ! cargo install wasm-bindgen-cli --version "$lock_version" --locked --force \
        --root "$tools_root"; then
        echo "ERROR: could not install wasm-bindgen-cli $lock_version under $tools_root;" \
            "nothing was built and target/web/dist/ is as it was." >&2
        exit 1
    fi
fi
if [ "$(installed_version)" != "$lock_version" ]; then
    echo "ERROR: $wasm_bindgen reports '$(installed_version)', not $lock_version, the version" \
        "Cargo.lock resolves; nothing was built." >&2
    exit 1
fi

# An explicit target directory, so a CARGO_TARGET_DIR in the environment cannot send the wasm
# somewhere this script does not look.
echo "==> cargo build -p evil-web --profile wasm-release --target wasm32-unknown-unknown"
cargo build --locked -p evil-web --profile wasm-release --target wasm32-unknown-unknown \
    --manifest-path "$repository/Cargo.toml" --target-dir "$target_directory"
wasm=$target_directory/wasm32-unknown-unknown/wasm-release/evil_web.wasm

mkdir -p "$web_directory"
# One build at a time from here on: two that reached the swap together could each see the other's
# half-moved bundle, and the later one would report success having published nothing. And -T on
# every move below: a bundle is never moved *into* a directory that turned up under its name.
exec 9>"$web_directory/.build.lock"
flock 9
staging=$(mktemp -d "$web_directory/.dist-staging.XXXXXX")
trap 'rm -rf -- "$staging"' EXIT
"$wasm_bindgen" --target web --no-typescript --out-dir "$staging" "$wasm"
cp -- "$page" "$staging/index.html"
chmod 755 "$staging"

bound_wasm=$staging/evil_web_bg.wasm
raw_bytes=$(stat -c %s "$bound_wasm")
gzipped_bytes=$(gzip -9 -c "$bound_wasm" | wc -c)
mebibytes() { awk -v bytes="$1" 'BEGIN { printf "%.2f MiB", bytes / 1048576 }'; }
echo "wasm size: $(mebibytes "$raw_bytes") raw ($raw_bytes bytes)," \
    "$(mebibytes "$gzipped_bytes") gzipped ($gzipped_bytes bytes)"
if [ "$raw_bytes" -gt $((SIZE_BUDGET_MEBIBYTES * 1048576)) ]; then
    echo "FAIL: the wasm is over its budget of $SIZE_BUDGET_MEBIBYTES MiB; target/web/dist/ is as" \
        "it was. Raise SIZE_BUDGET_MEBIBYTES at the top of this script only with a stated reason." >&2
    exit 1
fi

# The last good bundle is set aside, never deleted, until the new one is in place, and put back if
# it cannot be. A swap that failed in an earlier run left it aside with no dist/: it goes back
# first, so that only a stale copy is ever removed.
if [ -e "$dist.previous" ] && [ ! -e "$dist" ]; then
    mv -T -- "$dist.previous" "$dist"
fi
rm -rf -- "$dist.previous"
if [ -e "$dist" ]; then
    mv -T -- "$dist" "$dist.previous"
fi
if ! mv -T -- "$staging" "$dist"; then
    if [ -e "$dist.previous" ]; then
        mv -T -- "$dist.previous" "$dist"
    fi
    echo "ERROR: the new bundle could not be moved into place; target/web/dist/ is as it was." >&2
    exit 1
fi
rm -rf -- "$dist.previous"
echo "bundle: $dist"
ls -l "$dist"
