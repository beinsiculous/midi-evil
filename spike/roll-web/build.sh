#!/bin/bash
# Builds the harness for the browser into dist/: the release wasm, bound by the wasm-bindgen CLI
# pinned under the main checkout's target/tools/ (never the one on PATH, which is the engine's
# pin), next to index.html. Serving dist/ is coordination/web/probes/web-gate.sh's job.
#
# Someone may be serving dist/ while this runs, so the new bundle is made in a directory of its
# own and swapped with dist/ in one renameat2(RENAME_EXCHANGE): a request sees the old bundle or
# the new one, never half of each, and a failed build leaves dist/ as it was. The bundle it
# replaces is kept as dist.previous/, so a server whose working directory was the old dist/ goes
# on serving it rather than a deleted directory.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
main_checkout=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
wasm_bindgen=${WASM_BINDGEN:-$main_checkout/target/tools/bin/wasm-bindgen}

if [ ! -f "$here/Cargo.lock" ]; then
    echo "no $here/Cargo.lock: it pins the wasm-bindgen the CLI must match" >&2
    exit 1
fi
expected_version=$(grep -A1 '^name = "wasm-bindgen"$' "$here/Cargo.lock" |
    sed -n 's/^version = "\(.*\)"$/\1/p' || true)
if [ -z "$expected_version" ]; then
    echo "$here/Cargo.lock has no wasm-bindgen entry to match the CLI against" >&2
    exit 1
fi
if [ ! -x "$wasm_bindgen" ]; then
    echo "no wasm-bindgen CLI at $wasm_bindgen" >&2
    exit 1
fi
if [ "$("$wasm_bindgen" --version)" != "wasm-bindgen $expected_version" ]; then
    echo "$wasm_bindgen is not $expected_version, the version Cargo.lock resolves" >&2
    exit 1
fi

# An explicit target directory, so a CARGO_TARGET_DIR in the environment cannot send the wasm
# somewhere this script does not look.
cargo build --manifest-path "$here/Cargo.toml" --locked --release \
    --target wasm32-unknown-unknown --target-dir "$here/target"

staging=$(mktemp -d "$here/.dist-staging.XXXXXX")
trap 'rm -rf "$staging"' EXIT
"$wasm_bindgen" --target web --no-typescript --out-dir "$staging" \
    "$here/target/wasm32-unknown-unknown/release/roll_web.wasm"
cp "$here/index.html" "$staging/index.html"
chmod 755 "$staging"

python3 - "$staging" "$here/dist" <<'PYTHON'
import ctypes, os, shutil, sys

staging, dist = sys.argv[1], sys.argv[2]
if not os.path.exists(dist):
    os.rename(staging, dist)
    sys.exit(0)
libc = ctypes.CDLL(None, use_errno=True)
AT_FDCWD, RENAME_EXCHANGE = -100, 2
if libc.renameat2(AT_FDCWD, staging.encode(), AT_FDCWD, dist.encode(), RENAME_EXCHANGE) != 0:
    raise OSError(ctypes.get_errno(), "renameat2(RENAME_EXCHANGE) failed", dist)
previous = dist + ".previous"
shutil.rmtree(previous, ignore_errors=True)
os.rename(staging, previous)
PYTHON
echo "dist replaced at $(date --iso-8601=seconds)"
ls -l "$here/dist"
