#!/usr/bin/env bash
# Inspect every publishable native crate without uploading anything.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

package_args=()
case "${1:-}" in
  "") ;;
  --allow-dirty) package_args=(--allow-dirty) ;;
  *)
    printf 'Usage: %s [--allow-dirty]\n' "$0" >&2
    exit 2
    ;;
esac

# Native dependency order. Dev-only evil-imports in evil-exports is already available here.
crates=(
  evil-performance
  evil-synth
  evil-beats
  evil-project
  evil-imports
  evil-render
  evil-exports
  midi-evil
)

target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$repo_root/$target_dir"
fi
audit_dir="$target_dir/public-package-audit"
mkdir -p "$audit_dir"
printf '%s\n' "${crates[@]}" > "$audit_dir/publish-order.txt"

python3 -I - "${crates[@]}" <<'PY'
import json
import subprocess
import sys

expected = sys.argv[1:]
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--no-deps", "--format-version", "1"],
    text=True,
))
packages = {package["name"]: package for package in metadata["packages"]}
assert sorted(packages) == sorted(expected), "native workspace membership changed"
for name in expected:
    package = packages[name]
    assert package["license"] == "MIT", f"{name}: missing MIT license"
    assert package["description"], f"{name}: missing description"
    assert package["repository"] == "https://github.com/magicjedi90/midi-evil"
    assert package["homepage"] == package["repository"]
    assert package["authors"], f"{name}: missing authors"
    assert package["readme"], f"{name}: missing README"
    assert package["publish"] is None, f"{name}: publishing is unexpectedly restricted"
    for dependency in package["dependencies"]:
        assert dependency["name"] not in {"oracle", "evil-porting"}
print("Native metadata and dependency audit passed for all eight crates.")
PY

for crate in "${crates[@]}"; do
  printf '\n=== %s: package file list ===\n' "$crate"
  contents="$audit_dir/$crate.contents"
  cargo package -p "$crate" "${package_args[@]}" --list | tee "$contents"

  for required in Cargo.toml LICENSE README.md; do
    if ! grep -Fxq "$required" "$contents"; then
      printf '%s is missing %s\n' "$crate" "$required" >&2
      exit 1
    fi
  done

  forbidden="$audit_dir/$crate.forbidden"
  : > "$forbidden"
  if grep -Ei '(^|/)(compat|target|oracle|evil-porting)(/|$)|\.(nsp|prj|sng|bnk|trg|wav|mid\.ron|pyc)$' \
    "$contents" > "$forbidden"; then
    printf '%s contains private or generated artifact paths; see %s\n' "$crate" "$forbidden" >&2
    exit 1
  fi

  crate_dir="$repo_root/crates/$crate"
  if ! cmp -s "$repo_root/LICENSE" "$crate_dir/LICENSE"; then
    printf '%s license differs from the repository MIT notice\n' "$crate" >&2
    exit 1
  fi
  while IFS= read -r packaged_file; do
    # These files are synthesized by Cargo and are not present in the source tree.
    case "$packaged_file" in
      .cargo_vcs_info.json|Cargo.toml.orig) continue ;;
      Cargo.lock) source_file="$repo_root/Cargo.lock" ;;
      *) source_file="$crate_dir/$packaged_file" ;;
    esac
    if [[ ! -f "$source_file" ]]; then
      printf '%s lists missing source file %s\n' "$crate" "$packaged_file" >&2
      exit 1
    fi
    if grep -aEil '/home/|/Users/|evil[-_]porting|(^|[^[:alnum:]_])oracle([^[:alnum:]_]|$)' \
      "$source_file" >> "$forbidden"; then
      :
    fi
  done < "$contents"
  if [[ -s "$forbidden" ]]; then
    printf '%s contains local paths or private dependency references; see %s\n' "$crate" "$forbidden" >&2
    exit 1
  fi

  printf '%s: %s listed files; package-content audit passed\n' \
    "$crate" "$(wc -l < "$contents")"
done

printf '\nAll native cargo package --list audits passed. No crate was published.\n'
printf 'Lists and audit reports: %s\n' "$audit_dir"
printf 'Cargo archive/registry verification remains gated on publishing dependencies in this order.\n'
