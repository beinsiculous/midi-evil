#!/usr/bin/env bash
# Prints the identity of what the browser bundles are built from: every file under crates/, the
# workspace's Cargo.toml and Cargo.lock, and the gate's fixtures, as they are on disk now.
# scripts/build_wasm.sh compiles it into both bundles, and web-app-gate.sh refuses to start
# unless the visitor's bundle, the gate's and the tree all carry the same one, so neither bundle
# can be left from an older tree than the other.
#
#   scripts/build_identity.sh    # midi-evil-build-<32 hexadecimal digits>
set -euo pipefail
cd "$(dirname "$0")/.."

{
    find crates coordination/web/probes/fixtures -type f -print0
    printf '%s\0' Cargo.toml Cargo.lock
} | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -c1-32 | sed 's/^/midi-evil-build-/'
