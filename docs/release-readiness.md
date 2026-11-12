# Public packaging and release readiness

Status: **engineering verification, not release certification**. No crate has been published, no remote has been pushed, and the interactive/physical-device gates below remain separate.

## Verified on 2026-11-05

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --workspace` | 290 passed, 0 failed; 2 opt-in tests ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --manifest-path compat/Cargo.toml --workspace` | 322 passed, 0 failed; 2 opt-in tests ignored |
| `cargo clippy --manifest-path compat/Cargo.toml --workspace --all-targets -- -D warnings` | Passed |
| `tools/audit-native.sh` | Passed, including focused parity workflows and the release desktop build |
| README virtual ALSA null-input microphone command | 1 passed; no physical microphone used |
| `tools/audit-public-packages.sh --allow-dirty` | All eight native package lists and content scans passed |
| Clean public clone of implementation revision `41b5b22` | Native audit, release build, all eight clean package audits, and install/launch smoke passed; no compat/Oracle/evil-porting sources present |
| `cargo package -p evil-performance` | Archive created and verified in both the working tree and clean clone; this crate has no native registry dependency |
| `cargo install --path crates/midi-evil --locked` | Installed the release executable into an isolated smoke root; installed `--help` passed |
| Release binary launch arguments | `--help` passed; an unknown option exited 1 with one stderr line |
| Release desktop launch | Working-tree and clean-clone binaries each remained running for a five-second timeout with no stderr; no interactive gestures claimed |

## Chiptune follow-up verified on 2026-11-11

- Native formatting, build, 291 tests and strict Clippy passed.
- Compatibility formatting, 326 tests and strict Clippy passed, including all three local-only dense-fixture example tests run by the standard workspace command. Their ignored source is backed up under `compat/reference/chiptune/tooling/`; it is not part of a public clone.
- `tools/audit-public-packages.sh --allow-dirty` passed again after the native test/dev-dependency change; the older clean-clone result above does not certify this follow-up.
- NanoStudio 1.42 accepted, saved and reopened clean Chiptune probe/dense fixtures. Stress and below-limit source renders were each captured twice with no clipping, fully verified against native v2 JSON, and compared with archived source-repeat and complete-tail 10 ms envelope metrics. See `docs/chiptune.md` and the read-only manifests under `compat/reference/chiptune/`.

The focused desktop tests exercise full-song and exact-section PCM16 WAV output, `.evil` save/reload, MIDI export/reimport, queued track-stem and arrangement-part WAV workers, and preview-source/meter consumption without an audio device. These are hardware-independent workflow checks, not a substitute for operating the GUI.

The clean clone was created from the committed implementation revision with `--no-local`, contained only tracked public files, and had no compat workspace or Oracle/evil-porting directories. Its `tools/audit-native.sh` run passed all 290 native tests plus focused workflows, strict Clippy, and the release build. `tools/audit-public-packages.sh` passed without `--allow-dirty`. The desktop was also installed into an isolated root and launched from the clean release build. Later documentation-only commits do not change that verified implementation.

## Package inventory and publish order

`tools/audit-public-packages.sh` runs `cargo package --list` in native dependency order, requires `Cargo.toml`, `README.md`, and `LICENSE`, and rejects private compatibility dependencies, local absolute paths, proprietary fixture/archive extensions, and generated WAVs. Reports are written to `target/public-package-audit/`; they are local verification artifacts, not release assets.

| Order | Crate | Listed files |
| ---: | --- | ---: |
| 1 | `evil-performance` | 8 |
| 2 | `evil-synth` | 34 |
| 3 | `evil-beats` | 13 |
| 4 | `evil-project` | 11 |
| 5 | `evil-imports` | 10 |
| 6 | `evil-render` | 8 |
| 7 | `evil-exports` | 9 |
| 8 | `midi-evil` | 50 |

Total: 143 listed files, including Cargo-generated `.cargo_vcs_info.json`, normalized/original manifests, and lockfiles. The desktop's 50 files include only current Maxwell/MIDI Evil artwork, editable Aseprite masters, app-ready PNGs, drawing scripts, provenance, Rust sources, manifest, README, license, and Cargo-generated files. Unused legacy images are excluded.

Cargo cannot archive/verify packages whose native path dependencies are not yet available from the registry, even with `--no-verify`. After explicit publish approval, publish in the order above, wait for each crate to become available, and run full `cargo package` verification for the next. Do not interpret the file-list audit as registry publication or certification.

## Artwork and public-data audit

- The shipped MIDI Evil icon/splash Aseprite and PNG files are byte-identical to Insiculous `deion_assets` revision `1a7b363`; hashes and provenance are in `crates/midi-evil/assets/SOURCE.md`.
- The source policy permits these AI-generated assets in free releases. Paid distribution remains gated on a documented human hand-cleanup pass or replacement artwork.
- No NanoStudio artwork, proprietary bank/project fixture, reference WAV, Oracle/evil-porting source, or private compatibility workspace is included in any native package list.
- The public-domain Bach MIDI and source notice remain repository fixtures; they are not duplicated into native crate packages.
- The Chiptune kit is original deterministic synthesis. Clean source fixtures and both 256-hit source/native comparisons are now measured and archived, including source-repeat and 10 ms envelope metrics. The result remains **Partial**, not sample equivalence: direct waveform alignment is invalid, source allocation/choke behavior is not fully explained, and per-pad approximation limits remain. See `docs/nanostudio1-parity.md` and `docs/chiptune.md`.

## Remaining manual release gates

1. Exercise direct arranger block dragging and editing during playback.
2. Exercise piano-roll add/move/resize/delete and automation gestures in shared-pattern and unique-timeline contexts.
3. Test physical MIDI note/velocity, pitch bend, channel/polyphonic aftertouch, CC, disconnect/reconnect, and loop recording.
4. Listen through real audio output and test a real microphone; the ALSA null-input test captures silence only.
5. Test MIDI source save-back/relink and full/section/stem/part/MIDI exports through actual file dialogs, then reopen and audition the artifacts.
6. Complete paid-release artwork cleanup clearance if the distribution is not free.
7. After explicit approval, verify registry-backed packages in publish order and perform any publish/push/release action separately.
