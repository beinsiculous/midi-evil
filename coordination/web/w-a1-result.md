# W-A1 — the keyboard model, natively: the result

`coordination/web/plan.md` § W-A1. Three batches: W-A1a (`4926728`), W-A1b (`0e7ac2e`) and W-A1c
(`a15dc49`), all on `jesse`.

**Verdict: W-A1 PASSED on both kinds of evidence (D4), 2026-11-25, with one gate still owed:
`compat/` verified on Iroh (midi-evil#22).** W1 is re-planned and re-reviewed next, as the plan
says.

## The mechanical evidence

`coordination/web/probes/native-gate.sh` on `a15dc49`, run by the planner on 2026-11-25: every
sequence passed, 455 expectations. The suite is at 502 tests; clippy and fmt are clean. The nine
steps, as the gate heard them through AT-SPI on a private bus:

| step | what the gate did | what was heard or named |
|---|---|---|
| 1 a new project | Control+N on a clean project | "New project, 1 track." |
| 2 add an instrument | "Add a synth track" in the side panel | "Added Track 2, a synth track with the Lead sound." |
| 3 discover the instruments | Alt+Right, Alt+Left | "Max Nasty, 8 of 12 synth sounds", "Lead, 7 of 12 synth sounds" |
| 4 add a note | F6 to the roll, Enter | "Added.", then "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 100" |
| 5 select it, hear pitch and timing | Escape, Right | "Piano roll, no note selected", then the note's name again |
| 6 change it | Up | "Piano roll, C sharp 4, bar 1 beat 1, 1 beat" |
| 7 delete it | Delete | "Removed.", "Piano roll, empty" |
| 8 read the transport | I | "Transport, stopped, tempo 120 beats per minute, 4 4 time, bar 1 beat 1, position 0 minutes, 0 seconds" |
| 9 save | on a project opened by launch argument: an edit, Control+S | "Saved saved.evil."; relaunched on the file, the edit is there |

Not in any transcript, by design: Open from the menu, and Save As on a new project. Both pass
through the desktop's file dialog, which the probe cannot drive. They are the sitting's alone,
under Jesse's written waiver of 2026-11-24.

The per-surface sequences (Song, the track panel, files), the recording guard and a row for each
announced command that needs no dialog pass in the same run. The inventory of every control on the
shown surfaces is `w-a1-inventory.md`: 65 announced, 9 opening a held-back surface, 40 standard,
six failing items — one W3's, five in midi-evil#23.

## What Jesse heard

Wayland-native, with Orca 49.4 on COSMIC.

- **W-A1a, 2026-11-24:** "yep it passes, speaking at rest is fine" — the roll's moves and edits;
  a held key speaks once, when it comes to rest.
- **W-A1b, 2026-11-24, the transport proof:** a focused name that follows the clock is spoken
  every second, so the readout's name rests while it has focus and Enter reads it. No separate
  word was given on the batch's own sitting; the nine steps below read the transport and pass
  through the toolbar and the side panel.
- **W-A1c, 2026-11-25, the nine steps:** "okay it's all working out alright". Unitemised: it is
  recorded as the nine steps passing, the two dialog steps of the waiver among them, without a
  line per step.

## Left open

- `compat/` on Iroh against W-A1b's change to `evil-project` — midi-evil#22.
- The inventory's five failing lines that are not W3's, the side panel's width handle among them —
  midi-evil#23.
- The surfaces held back from the first web release — midi-evil#17 to #21.
- Built as is, and not ruled on: Enter on a transport reading that has not changed announces it.
