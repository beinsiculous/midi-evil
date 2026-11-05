#!/usr/bin/env bash
# Reproducible, hardware-independent native-parity audit.
# Compatibility/Oracle tests are intentionally separate from this public workflow.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace native_parity -- --nocapture
cargo build --release -p midi-evil

printf '\nNative automated parity audit passed.\n'
printf 'Interactive desktop, physical MIDI, and real audio-device checks remain separate release gates.\n'
