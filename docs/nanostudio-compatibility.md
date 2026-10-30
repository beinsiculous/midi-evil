# NanoStudio compatibility evidence

This document records observed NanoStudio behavior separately from the native `.evil` format. Structural preservation, MIDI conversion, and faithful audio rendering are different compatibility levels. Local fixture provenance and checksums are in `target/fixtures/nanostudio-1.42-demos/MANIFEST.md`.

## Verified with NanoStudio 1.42 MIDI exports

The five bundled demo projects were loaded in NanoStudio 1.42 and exported through **Song / Tools / MIDI File / Export**. Comparing those files with `oracle midi export` established the following behavior for their format-v5 sequences:

- `Package.sng` uses **256 ticks per quarter note**. NanoStudio's MIDI headers also use division 256, and source/destination absolute note ticks match without rescaling.
- Timeline parts repeat their patterns for the full part length. A note that starts inside a part retains its complete duration when its note-off falls beyond the part boundary.
- A note stored at the pattern's end tick is exported. If the same pitch, velocity, and duration is also stored at tick 0 and another repeat follows, only one of the two coincident notes is exported. Ordinary duplicate notes elsewhere in a pattern remain distinct.
- Track flag bit 0 being clear does **not** by itself exclude a track's notes from MIDI export. Its meaning remains unknown; do not treat it as a mute flag.
- All five projects have swing 0.7, but their official MIDI exports retain the unswung source ticks. Swing therefore must not be applied to this MIDI export path. Playback/render swing remains a separate behavior to investigate.

On 2026-10-28, the oracle matched all **7,572 notes (15,144 note-on/note-off events)** across the five references. `tools/compare_midi.py` compares absolute ticks, event kind, pitch, and velocity, grouping tracks by instrument name.

## Controller and pitch-bend records

For controller events, `data[2]` is the observed automation type and `data[0]` carries the value. The following mappings are implemented from exact event-by-event comparisons:

| Instrument kind | Type | Standard MIDI mapping |
| --- | ---: | --- |
| Eden | 0 | Signed pitch bend |
| Eden / Mixer | 4–7 | CC70–73 (`66 + type`) |
| TRG | 7–9 | CC75–77 (`68 + type`) |
| TRG | 10–12 | CC80–82 (`70 + type`) |

For pitch bend, `data[0..2]` is a big-endian signed 16-bit integer. Dividing it by 4 with truncation toward zero produces the shared model's signed 14-bit value; zero maps to MIDI center 8192. For CC events, values 0–127 are emitted directly. The observed `0xff` value is exported as 0; unobserved `0x80..0xfe` values are rejected explicitly. The second data byte for non-pitch controllers and the trailing record value (16 in these fixtures) remain undocumented.

`tools/compare_automation.py` found zero differences across **1,896 mapped automation events: 1,537 CC events and 359 pitch bends** before preset initialization was added. The comparison now also checks MIDI channels, CC32 bank selects, and program changes.

## Preset initialization and timeline changes

For Eden instruments in single-preset mode, `Package.prj`'s `AM SingleI` value uses a 768-based index: `SingleI - 768` is the initial MIDI program. Missing `SingleI` defaults to program 0; values that cannot map to 0–127 are rejected. Eden uses CC32 bank select 7, while Mixer and TRG tracks initialize bank 0/program 0. Initialization is emitted at tick 0 for each non-empty sequencer track, even when its first part begins later, so multiple sequencer tracks using one instrument can produce duplicate initialization pairs.

A timeline part's `preset_or_flags` field is a preset-change record when bit 24 (`0x01000000`) is set. Bits 0–7 select the program, and bit 8 (`0x00000100`) selects bank 7 when set or bank 0 when clear. The bank/program pair is emitted at the part's start tick, including when it repeats the current program. Programs above 127 are rejected rather than truncated.

Across the five official exports, all **42 program changes and 42 CC32 bank selects** match, including every observed part-boundary change. Together with the earlier mappings, **1,980 automation/setup events** now match by absolute tick, value, and MIDI channel. Sequencer tracks that share an instrument are merged into one MIDI track, while retaining initialization from each non-empty source track; the resulting non-empty track names and order match all five references. Exact same-tick event ordering, the project-name conductor track, and empty padding tracks are not yet matched.

## Still open

- The demo corpus has looping disabled and uses 4/4. Loop-region and non-quarter beat-unit conversion are not established by these references.
- Exact ordering of simultaneous note/controller/preset records remains open; NanoStudio often reverses simultaneous source records, while the shared model currently stores notes and automation separately. The references also use the project name for the conductor track and pad the file with empty tracks, which the oracle does not yet reproduce.
- Untouched binary/XML preservation must be checked independently of MIDI equivalence.
- No audio-equivalence claim follows from matching MIDI notes and automation. Synthesis, effects, automation interpolation, sample playback, and render swing require separate differential fixtures.
- A real format-v6 project and independent one-feature-at-a-time NanoStudio fixtures are still required.
