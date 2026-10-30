# MIDI Evil Roadmap

Living tracker for planned features and follow-up fixes. Update items with verification notes as they land. Engine-integration order follows `docs/insiculous-crossing.md`. Completed work is moved to `ARCHIVE.md`.

## NanoStudio reverse engineering and compatibility

Scope: documented, lossless project interoperability and explicit handling of anything still unsupported. Native editor/synth feature work and original preset design are separate from reverse engineering. Keep user-owned or copyrighted fixtures local; only add self-created or appropriately licensed examples to the repository.

- [ ] Build a representative, documented fixture corpus
  - Create small NanoStudio projects that vary one feature at a time, plus one dense project that combines them; record the NanoStudio version, original settings, provenance, and checksums.
  - Cover both `Package.prj` and `Package.sng`, real sequence v5 and v6 projects, format/version variants, multiple tracks and patterns, Eden and TRG-16 instruments, sampled audio and loop settings, mixer effects, and every available automation/controller type.
  - Keep the existing minimal fixture as a smoke test, but require independent feature-specific fixtures before claiming coverage. Verify untouched `Package.sng` byte identity, semantic XML equivalence, and asset hashes automatically; add NanoStudio reopen/save checks to the acceptance harness.
  - Progress (2026-10-28): added a local, checksummed corpus of all five NanoStudio 1.42 bundled demos plus application-generated reference MIDI. See `target/fixtures/nanostudio-1.42-demos/MANIFEST.md`; these format-v5 demos complement rather than replace the required independent fixtures.

- [ ] Complete the `Package.prj` semantic map and writer coverage
  - Inventory every remaining unknown or opaque XML element, attribute, block, default, enum, and value range; document evidence and confidence rather than guessing names or meanings.
  - Cover remaining song/global settings, track and instrument references, routing, and metadata beyond the already-supported tempo, swing, meter, channel volume/pan, and effect-enable editing.
  - Preserve not-yet-understood data during unrelated edits, add one-field-at-a-time differential fixtures, and test both untouched and semantically edited serialization using semantic XML diffs rather than assuming byte-identical XML formatting.

- [ ] Complete the `Package.sng` sequence/event map and writer coverage
  - Identify all remaining record types, flags, unknown fields, pattern/arrangement metadata, and ordering rules, including the skipped header regions (`0x08..0x87` and `0xa8..0xc7`), track/note flags, controller payload/value, and part `preset_or_flags` behavior.
  - Decode and preserve all performance/automation records, with explicit handling for overlapping notes, duplicate events, extreme values, pattern/part boundaries, and unsupported record types.
  - Document and implement correct swing, loop-state, meter/beat-unit, and tempo semantics in performance/MIDI conversion; the current converter does not apply swing or loop state and uses a fixed quarter-note beat unit.
  - Add binary golden tests and edited round trips for each record family, including a real v6 fixture; do not treat opaque byte preservation alone as semantic support.
  - Progress (2026-10-28): corrected conversion to 256 ticks/quarter, preserved notes beyond part boundaries, matched repeat-boundary duplicate behavior, and decoded the observed part preset-change flags. All 7,572 notes match the five official reference exports. Evidence and remaining loop/meter/render-swing work are recorded in `docs/nanostudio-compatibility.md`.

- [ ] Decode NanoStudio automation and controller mappings
  - Map each proprietary controller/automation event to the shared performance model and Standard MIDI where a faithful mapping exists; document units, ranges, tick ordering, channel/track scope, and interpolation behavior.
  - Replace the current skipped-controller export path for decoded events while retaining an explicit count and reason for anything genuinely unmappable.
  - Verify pitch, modulation, expression, mixer, and instrument automation against source projects and exported MIDI; do not silently discard data.
  - Progress (2026-10-29): implemented signed 16-bit pitch-bend decoding plus Eden/mixer CC70–73 and TRG CC75–77/80–82 mappings. Decoded initial and part-boundary bank/program selection, merged sequencer tracks by instrument, and matched all 1,980 observed automation/setup events by value and channel. All non-empty instrument track names and ordering match the five official exports; exact same-tick ordering, conductor naming, and empty-track padding remain open. See `docs/nanostudio-compatibility.md`.

- [ ] Reverse engineer Eden instrument data for compatibility
  - Document and parse/write the stored oscillator, pitch, envelope, filter, LFO/modulation, unison, polyphony, glide, and instrument-effect settings, including defaults and version differences.
  - Preserve exact source settings independently of the native synth approximation; add semantic reader/writer tests using original factory and user-created presets with documented provenance.
  - Record any synthesis behavior that still differs from NanoStudio instead of claiming audio equivalence from matching parameters alone.

- [ ] Reverse engineer `Eden.bnk` and instrument-bank references
  - Document the bank container, preset records, names, defaults, parameter encoding, and version differences; resolve project/bank references without confusing factory presets with user edits.
  - Add safe bank parsing and semantic serialization tests across multiple factory and user-created banks, while retaining unknown data needed for lossless compatibility.
  - Keep bank import/conversion on the compatibility path; a one-off native preset recreation is not evidence that the bank format is understood.

- [ ] Reverse engineer TRG-16 and sampled-instrument data for compatibility
  - Document pad/sample references, sample metadata, tuning, amp envelopes, output-filter routes, velocity behavior, pan/gain, loop points, and any remaining pad settings.
  - Preserve referenced audio and unknown asset data through unpack/edit/repack workflows, including missing, duplicate, and non-ASCII asset names.
  - Add reader/writer fixtures for each setting and explicitly report any playback behavior the native engine does not implement.

- [ ] Finish mixer and effect-parameter compatibility
  - Decode effect types, parameter layouts, defaults, ranges, routing/send behavior, and channel/global settings beyond the existing volume, pan, and enable/disable controls.
  - Add safe semantic editing that preserves unrelated mixer data, with fixtures for every supported effect and explicit rejection or preservation of unknown effect versions.
  - Distinguish correct project serialization from native audio rendering; document effects that are preserved but not rendered.

- [ ] Reverse engineer and validate faithful NanoStudio playback/render behavior
  - Treat this as a separate, extra-large compatibility phase after structural editing: reproduce Eden synthesis, TRG sample playback, automation, mixer/send routing, and original effects rather than substituting one original synth patch for every track.
  - Build differential audio/behavior fixtures for envelopes, filters, modulation, sample loops, swing, loop playback, effect routing, and parameter automation; keep this compatibility renderer separate from the native `evil-*` architecture.
  - Compare against NanoStudio renders with documented tolerances and explicitly report any remaining sonic differences; matching stored parameters alone does not close this item.

- [ ] Validate complete projects against NanoStudio and publish the compatibility matrix
  - Open untouched round trips, newly generated projects, and one-feature-at-a-time edits in NanoStudio; check for repair prompts, missing assets, changed settings, timing differences, and audible mismatches.
  - Run dense real-world projects through unpack/pack, semantic edits, MIDI export, and reopen/save cycles, keeping compatibility results separate from native `.evil` tests.
  - Document supported, preserved-only, exported-only, and unsupported features with fixture references and remaining unknowns; no roadmap item should close on serialization tests alone when real-app validation is available.
  - Progress (2026-10-29): an official Minimal-template project with a replaced full-bar sequence reopened successfully in NanoStudio 1.42 and rendered with its tail. The blank `song create` template failed the same application-open check and needs a minimal differential fix before it can be considered accepted.

## Follow-up editor work

- [ ] Give the Evil Synth instrument window a focused, polished layout
  - Replace the long parameter stack with clearly grouped oscillator, envelope, filter, modulation, and delay sections; use responsive columns and consistent control sizing so common edits remain visible without excessive scrolling.
  - Add useful visual feedback for ADSR and LFO shapes, clearer enable/bypass states, and a prominent preset/audition workflow while retaining access to every JSON patch parameter.
  - Check the layout at small and large window sizes and verify that editing, reset, save, and audition still operate on the intended track.
  - Progress (2026-10-30): renamed the former Eden-like instrument window to **Evil Synth**; the layout and visual-feedback work remains open.

- [ ] Port reference-guided synthesis behaviors into the native engine
  - Implement tempo-map-aware LFO beat sync, accurate oscillator sync/Osc Mix behavior, source-shaped filter and amplitude envelopes, glide/accent routing, and tempo-synchronized instrument effects.
  - Progress (2026-10-30): added `beats_per_cycle` with ordered `SynthTempoChange` events through native project, mono, and stereo-mix rendering, plus regression coverage for fixed-rate equivalence and mid-render tempo changes. Added shared offline/live sync-reset anti-aliasing for square and saw voices. True Osc Mix behavior, filter-envelope curves, glide/accent, and effect-tail matching remain open.

- [ ] Build `evil-beats` into a complete native drum instrument
  - Add original WAV pad loading, pad-bank JSON, per-pad mixer/output-filter settings, CLI rendering, and desktop editing/audition without depending on Oracle packages.
  - Progress (2026-10-30): created the `evil-beats` crate with pitched sample pads, velocity, gate/release envelopes, and independent overlapping-hit rendering plus unit tests. Chiptune capture and bank integration remain open.
  - Progress (2026-10-30): added the Maxwell **Pads** desktop workspace, original synthesized 16-pad kit, WAV import/export, portable `.evilkit` banks with embedded audio, pad tuning/envelopes/timed gates, stereo gain/pan/mute, and sample-clocked held-pad repeat. Native song-track bank references, recording, output filters, and CLI rendering remain open.

- [ ] Improve reference-guided recreations of the user's preferred instruments
  - A17.Nasty: compare a full 4/4 bar plus release/delay tail against NanoStudio; measure and address synthesis differences rather than claiming equivalence from stored parameters alone.
    - Progress (2026-10-29): captured the clean NanoStudio reference and corrected resonant slope units and middle-C filter tracking. Calibrated gain/cutoff/delay brought held-note RMS from 0.541 to 0.146 (reference 0.147) and spectral centroid from 1.75 to 3.93 kHz (reference 4.20 kHz). Waveform correlation, high-frequency shape, and exact tail behavior remain open. See `docs/a17-nasty.md`.
    - Progress (2026-10-29): added dry/modulation isolation banks, LFO-to-LFO rate routing, square LFOs, naive-square support, and sync-scoped output DC removal. A17 now has LFO4→LFO3 rate modulation at calibrated depth 0.25; RMS is 0.1461 (reference 0.1473) and waveform correlation improved from 0.0038 to 0.0294. Exact filter/LFO phase and depth, high-frequency shape, envelope curves, and tail remain open.
    - Progress (2026-10-30): matched the dry harmonic structure with 1.25 sync, 15.5-cent tuning, and a near-open 20 kHz filter, then recalibrated stereo gain. Spectral centroid improved from 3.40 to 4.13 kHz (reference 4.20), waveform correlation from 0.0294 to 0.0448, and 6–7 second tail RMS from 0.000397 to 0.000447 (reference 0.000455). High-note key-tracked cutoff clamping is covered by a regression test. LFO/envelope behavior and the early tail remain open.
    - Progress (2026-10-30): confirmed from the source UI that LFO3 uses Beat Sync. A local phase-continuous modulation plus 8 Hz amplitude-articulation experiment reached 0.57 10 ms envelope correlation, but its delayed full-bar render was not better overall and was not adopted. Add tempo-aware beat-synced LFO timing and calibrate the rhythmic articulation jointly with spectrum, gain, and delay.
    - Progress (2026-10-30): decoded LFO3 sync mode 6 as sixteenth-note timing (8 Hz at 120 BPM), kept LFO2/LFO4 free-running, and adopted phase-continuous rate modulation plus jointly calibrated 8 Hz amplitude articulation. Ten-millisecond envelope correlation improved from 0.0789 to 0.4841, normalized envelope variation is 0.378 versus the reference 0.386, and RMS/centroid remain close at 0.1459/4.24 kHz versus 0.1473/4.20 kHz. Tempo-aware beat sync and tail behavior remain open.
    - Progress (2026-10-30): captured verified NanoStudio C2/C3/C5 fixtures from the same NanoStudio-saved project as C4 and slowed the added articulation to 4 Hz after multi-pitch comparison. Added 0.3-per-octave pitch gain around middle C: C2 RMS improved from 0.143 to 0.211 (reference 0.224), C2 0–200 Hz energy now matches at 78.52 versus 78.46 dB, and C3/C4/C5 RMS is also close. Low-note aliasing, C3/C5 spectral shape, and tail behavior remain open.
    - Progress (2026-10-30): Nasty's LFO3 and amplitude lanes now use `beats_per_cycle` and follow complete project tempo maps, verified with 60/120/240 BPM renders and a mid-song tempo change. Added shared sync-reset anti-aliasing; exact Osc Mix/filter/envelope and tail behavior remain open.
  - B14.Nano-303: include filter-envelope, glide, and accent fixtures.
  - A09.GlockenBlocks: capture several pitches and the complete decay to guide tuning and envelopes.
  - Chiptune drumkit: capture pads individually and in overlapping patterns; preserve source audio only on the compatibility path, while preferring original synthesis for native instruments.
  - Keep fixture creation, inspection, and rendering scriptable through the agent-friendly CLI in `docs/cli-ai.md`.
