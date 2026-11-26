# NanoStudio parity Group 1 — stereo samples and performance routing, v6

Issue `beinsiculous/midi-evil#12`; branch `jesse`.

Status: SETTLED by Jesse, 2026-11-24, after five adjudicated rounds. Batch 0 completed verification
on 2026-11-25 against `5826e55`, including native/compat suites, strict Clippy, release and wasm32.
Scope is the two remaining Group 1 rows: stereo source preservation/
AIFF pads and assignable XY/knob routing, plus prerequisite export safety exposed by their review.
Existing Ring 2, bidirectional loops, Compare, envelope inversion and polyphony remain. The review
outcomes are summarized in `reviewer-comparison.md`; the plan carries every accepted decision.

## 0. Ownership, baseline and review bytes

Another session owns a large tree spanning Group 1 and unrelated parity work. Do not edit, stage,
stash, revert or run cargo until that work is **committed or parked by its owner**, including ignored
`crates/oracle` and `crates/evil-porting`. Archive/hash those private directories before any edit and
include before/after diffs and hashes in the review bundle. Hunk-level separation of mutually edited
lines is not accepted.

If the owner committed premature Verified/ROADMAP claims, the first stereo change reverts those hunks
until evidence passes. Committed inline `right_samples` files are supported legacy input through the
bounded migration path. Do not sweep unrelated project/pattern/sequence work into Group 1.

Exact-tree gates use a temporary worktree containing HEAD plus the reviewed/staged bytes. Copy ignored
`compat/`, Oracle/evil-porting and named fixtures into it so symlinks resolve internally; exclude other
`target/` content. A hash manifest identifies copied compatibility bytes. Persistent compatibility fixes
are made after the ownership gate and reviewed through their before/after diff. Never certify a main-
checkout compat run as exact-tree evidence. Re-read the plan before each handoff. Executors stage and
never commit; the planner asks before committing.

## 1. Prerequisite export safety

Each render/queued export batch snapshots project, reference base and track/part selections immediately
at creation. A non-UI loader resolves every referenced instrument once into an immutable batch snapshot.
From click until resolution succeeds/fails, Save & assign is blocked so a preset version cannot change
mid-resolution. Missing/malformed sources produce explicit batch errors; no unresolved export starts.
Every track/stem/part uses the same project/instruments despite Save-As, overwrite, edits or reordering.

One active worker and at most one canceled/draining worker may coexist; further requests wait for
capacity. Cooperative cancellation checkpoints stop synthesis/render loops promptly, and a canceled
I/O-stalled worker may detach after a grace period without acquiring another draining slot. Generation
checks prevent stale publication. New destinations use short-named temporaries beside the output;
existing destinations are written through their existing path under the per-destination lock and
generation guard, preserving OS ownership, group, ACLs, mode and symlink chains without external tools.
Only the current generation owner publishes; cancellation, error, stale completion and bounded app
shutdown clean temporaries. Existing-path cancellation may leave a visible partial file, as before.
Tests interleave same-path and new-path saves, reorder tracks, reverse
completion order and repeat cancel/retry while asserting bounded workers/snapshots and no orphan files.

This is Batch 0, independently reviewed and gated for existing mono projects before stereo work begins.

## 2. Stereo source, persistence and rendering

### Input and capture

Mono/stereo WAV and uncompressed AIFF/AIFC preserve channels in Eden and pads. Only one/two-channel files
are accepted. Dispatch recognizes `.aif`/`.aiff`/`.aifc` case-insensitively and validates FORM/COMM.
NONE/twos/sowt and float decoding remain covered. Readers inspect metadata and stream/bound source/chunk
reads before allocation; oversized audio/ancillary chunks/truncation fail cleanly.

External import retains finite-audio, 8–192 kHz and 30-second limits. Internal preprocessing may retain
bounded tails up to a separate embedded-source limit, subject to memory/byte budgets; save/reload tests
cover tails beyond 30 seconds. Such sources are not claimed readable by older builds and are labeled.
Source WAV export is external interchange, not a promise that the 30-second importer accepts long tails.
Section resampling retains stereo.

Microphone capture opens the device's actual channel count and validates selections at each start. Mono
selects one input (default 1); Stereo selects two. Selection happens inside the callback without buffering
unused channels. Every callback bounds-checks indices against the current frame; a device mismatch mutes
and reports an error rather than panicking or silently selecting another input. File downmix is `(L+R)/2`.

### Immutable sources and portable deduplication

A private audio-buffer newtype wraps shared Arc storage, validates finite/equal-length channels at
construction and exposes no mutation/`Arc::make_mut`. Replacements receive process-global never-reused,
never-serialized source IDs/revisions. Value equality compares content with a fast path; live/cache
invalidation uses ID + revision. Retired sources/patches return to the UI thread for destruction.

The process-wide content interner holds weak references, uses bitwise equality (including signed zero),
hashes outside its lock and locks only for map lookup/insertion. Loading equal content shares one identity
without retaining expired audio.

New patch JSON writes single-reference sources inline in the legacy shape. Shared sources are written once
in root `sample_sources`; their zones carry indices plus metadata. Mixed files use the table only for
shared sources and require the new reader; all-single-source sources within the old embedded limits retain
legacy readability. Validate indices and inline/index exclusivity. Save/preflight prunes unreferenced
sources and compacts references atomically.

New table-based files have bounded raw reads/output. One streaming legacy reader handles all-inline files,
coalescing bitwise-identical content as decoded. It enforces per-source, aggregate retained-audio and
incremental collection budgets. Distinct legacy sources beyond the new save ceiling may load only within
a higher bounded recovery budget, with a visible read-only warning and source export/removal remedy;
beyond that loading fails clearly. Over-ceiling duplicated slices load/dedupe/re-save; non-coalescing
sources hit the bounded outcome before exhaustion.

Imports enforce deterministic worst-case portable-byte and frame/time limits. Preflight includes maximum
sample/encoding width, syntax/padding, source/reference metadata, performance values and twelve route
rows, so passing guarantees exact save fits. Exact save retains atomic rollback. Destructive operations
preflight the actual post-copy-on-write source set; non-growing edits are not blanket-exempt when COW
adds a distinct source. Kit ceiling remains 128 MiB; patch ceiling is explicit and no larger. Thirty-
second decoder eligibility does not promise portable capacity.

Destructive shared-source edits are COW: one zone gets a new source while siblings/bounds stay unchanged.
Crop/normalize/effects/undo retain both channels/settings. Normalize uses one cross-channel peak. Stereo
preprocessing uses the shared engine including cross-channel delay/tails. Undo restores both channels and
settings.

### Stereo engine

Voices produce stereo frames with one shared zone cursor/interpolation fraction. Non-sample/modal sources
are centered. Stereo survives oscillator modes, envelopes, per-channel filter/DC, allocation, instrument/
channel effects, sends, master processing and tails. Note validation/modulation/timing/polyphony/caching
are shared.

Mono APIs/MonoTrack patches retain exact legacy output/effect order. Stereo synth projects use StereoTrack
with instrument waveshaper/delay/controllers, ordered inserts, output DC, sends and analysis. A named
stereo-synth pan law makes dual-mono stereo match MonoTrack at center and through nonlinear processing;
drum balance is unchanged. Pads use per-channel voice/bus filters and stereo direct/send before balance.

Patch-definition comparison uses source identity/revision; validated immutable audio is not rescanned.
Control updates do not clone/revalidate patches and performance values never travel as Patch commands.
Batch 2 owns these structural live costs.

Pads loads WAV/AIFF through one action and exports stereo. Patch Lab/Pads show channel count and frame-
based duration; mono peak drawings are labeled views. Portable workflows remove originals, move/reload
and compare distinct-channel offline/live output.

## 3. Runtime performance model

`Patch.performance: Option<PerformancePatchbay>` is omitted from legacy JSON and stores routes plus five
explicit normalized values. Center `0.5` means no offset. Gestures never mutate base preset parameters.

```rust
pub struct PerformancePatchbay {
    pub values: [f32; 5],
    pub routes: Vec<PerformanceRoute>, // <= 12 total stored, active or inactive
}
pub struct PerformanceRoute {
    pub source: PerformanceSource, // XY1 X/Y, XY2 X/Y, Knob
    pub destination: PerformanceDestination,
    pub downward_depth: f32,
    pub upward_depth: f32,
    pub derived: bool,
}
```

Below center subtracts `(1 - 2*value)*downward_depth`; above adds `(2*value - 1)*upward_depth`. Cutoff
depths are octaves. Opposite signs invert. Fan-out reuses a source; fan-in sums all additive offsets,
clamps summed cutoff octaves to the safe logarithmic range **before** one powf, then clamps final cutoff
to positive/effective Nyquist. Evaluation is order-independent across centered/opposing/reordered routes
and reload.

`None` reads as four derived legacy routes with centered values: XY1 cutoff/resonance and XY2 delay time/
mix; knob unassigned. Depths derive from current base toward old ranges where possible and clamp at zero
at boundaries. Base edits re-derive only `derived` routes; editing source, destination or either depth
marks a route authored (`derived = false`), persisting that distinction through reload. Centered offsets
intentionally change the old absolute transfer curve; boundary/dead-zone behavior is documented/tested.
Viewing does not materialize/dirty; first edit does. Inactive legacy targets offer undoable Enable that
creates the default base and derives depths. Other inactive routes are visible, bypassed and counted
toward twelve. Inactive means missing/disabled target, not a separate bypass feature.

Every destination has safe two-sided limits/depth validation. Envelope stages have a 0.001-second positive
minimum; whole-patch clamps include duration-plus-release, cutoff, stable resonance, pulse width, delay
and 0–1 mix/amplitude. Clamping precedes DSP. LFO disable retains inactive routes. Permanent removal
deletes its routes and rewrites higher lanes; existing voices retain immutable voice-local patch/LFO
snapshots so indices remain valid. Patch Lab Undo restores complete historical topology atomically;
active/inactive routes share one twelve-route capacity.

Shared loaders return structured performance degradations through desktop, `RenderOutput` and CLI.
Unknown/excess routes are omitted and missing values center with a visible warning while audio/base
survive. Degraded patches are read-only by default; Save & assign overwrite requires explicit confirmation
naming the routes to be discarded. Strict malformed-audio validation remains.

### Stage-latched timing and continuous controls

Attack/decay and Key/Off LFO timing latch for a fresh voice. **Release latches on entry to note-off/
release**, using current values/current envelope level without retroactive jumps and driving remaining
lifetime. Offline uses each note's known note-off value. Non-retriggering legato retains current envelope/
timing/phase; the next fresh voice uses current values. Beat-synced LFO timing updates globally at a
control boundary to retain one song grid.

Route evaluation produces separate immutable latched and continuous snapshots. Continuous filter,
oscillator amplitude/detune/pulse width, depth/resonance and effect controls update held/releasing voices
at block boundaries. Value changes use deterministic short ramps for discontinuous destinations;
stationary values reuse cached math with no per-frame route powf/audio work. Sweep bounds and stationary
offline/live equivalence are tested.

Delay routes reuse maximum-size preallocated rings/smoothed fractional reads. Voice-local topology edits
affect subsequent voices; shared-effect topology edits apply globally at a control boundary to current
audio/tails through one chain. Caches carry definition generations owned per voice or globally for shared
effects. Base/target/definition changes invalidate caches even when values are unchanged.

Seconds/musical-time destinations are distinct; synced captions select beat length and audible tests cover
timing. Effective waveshaper/delay settings cross evil-render's dry-render/mixer boundary exactly once
with existing automation/order.

### Preset/draft boundary

Positions become preset state through Patch Lab **Save & assign**. Nearby copy says so and offers Reset
to centre. Unsaved Keyboard gestures audition the draft and leave project renders unchanged; assignment
intentionally updates all references. Compare/Reset preserve base/performance snapshots. No UI claims
timeline automation recording (Group 2).

## 4. UI and Patch Lab history

Patch Lab adds **PERFORMANCE** with up to twelve route rows (source, destination, downward/upward depth,
derived/authored state, remove), Add/capacity display and labeled keyboard-accessible controls for all
five axes. Keyboard has two routed XY pads and a labeled knob. Captions name destinations/depths/offset/
active/timing state.

A bounded Patch Lab draft Undo stack records one entry per completed structural gesture, not per-frame
values. Structural Undo restores base/routes/topology atomically and carries current performance values
forward; value gestures do not evict structural history. Compare/Reset interactions are tested. Enable
and LFO removal use this actual UI; Pads retains its separate stereo-complete sample undo.

XY pads support dragging, focus and arrow keys. A focused pad consumes/locks arrows against egui focus
navigation; modifier-arrow fine steps are supported. Performance-widget focus does not release computer-
keyboard notes; only text entry does. Real egui input tests hold chords while entering/operating/leaving
each control, including arrows and modifier-arrows. Route/value editors provide non-pointer alternatives.
A real-app gate proves a value gesture sends no Patch command and changes a held voice.

## 5. Sequential batches

### Batch 0 — export snapshot, cancellation and publication

Scope: `crates/midi-evil/src/render_job.rs`, `app.rs`, `app/keys.rs` and `beats.rs` for writer guards,
`crates/evil-render/src/lib.rs` and `export.rs`, and cancellation-aware loops in shared synth/beats/mix/
WAV/effect-processing code, their same-file tests, plus persistent compat callers only if compilation
requires. Keep one worker; only explicit Cancel aborts file output, and queue a subsequent transport
action with its request-time snapshot while draining. Re-verified at `c16e6d5`: `RenderJob` holds only a result receiver;
`WavExportQueue` holds specs; `StudioApp::start_render` clones the current project for every queued
item; `evil-render` resolves patches/kits inside each render. No snapshot/cancellation/publication API
exists. Implement §1 before stereo. Gates: exact/working native and copied-compat suites, strict
Clippy, full/section/stem/part exports for mono and existing stereo drums, interleaved saves/reordering,
cancellation/resource/temp tests, release build.

### Batch 1 — reconcile and complete stereo

Scope: existing Group 1 changes in `evil-synth`, `evil-beats`, `evil-render`, stereo-specific
`evil-imports`, desktop `patch_lab.rs`, `beats*.rs`, `sample_recorder.rs`, manifests and persistent compat
fixes. Exclude unrelated work. Implement §2, including reverting unverified claims if needed. Gates: all
Batch 0 gates plus distinct WAV/all AIFF encodings, long tails, oversized legacy migration/recovery,
portable/offline/live workflows and release build. No intermediate left-only import or Verified claim.

### Batch 2 — performance model and shared engine

Scope: `evil-synth` performance/patch/voice/live/render and required delay/effect/mix; production
`evil-render` and structured warning outputs; compat/desktop only as compilation requires. Implement §3
model/engine and test every destination/clamp, provenance histories, fan-out/fan-in/order, disabled/synced
targets, capacity/snapshot restoration, legato/beat/note-off timing, allocation, held/release controls,
topology/tails, stationary/swept values, degraded loading and offline/live equivalence.

### Batch 3 — performance UI and verified ledger

Scope: desktop `keyboard.rs`, `patch_lab.rs`, `app/keyboard_page.rs`, `live_player.rs`, `app/chrome.rs` if
needed, tests, `docs/nanostudio1-parity.md` and `ROADMAP.md`. Implement §4 and all previous gates plus
complete `tools/audit-native.sh`, copied-compat checks and release build. Test Undo/Compare/Reset/Save &
assign, degraded-save confirmation, unsaved-render isolation, saved/reloaded setup, held chords and the
no-Patch-command held-note gate. Mark both rows Verified only when complete workflows pass.

## 6. Review record

Kimi planned; Claude, Gemini and Codex reviewed rounds 1–5 and Jesse adjudicated every finding.
Gemini's headless executor was unavailable under its command-permission setup, so Jesse selected the
built-in coding agent for Batch 0. Its diff goes to Claude and Codex plus the planner; later executor/
reviewer assignments keep the cross-vendor rule. `reviewer-comparison.md` records outcomes/timings.
Unfinished work becomes an issue before completion.
