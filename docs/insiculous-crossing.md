# The crossing to the Insiculous 2D engine — what the engine needs from MIDI Evil

Written 2026-10-26 by the Insiculous working set's planner (Claude Fable), at Jesse's request, for
the agents building MIDI Evil. The engine is `~/projects/insiculous/insiculous_2d`; its plan for
this seam is `coordination/external-tools/plan.md` there, § "MIDI Evil's half". Nothing here is a
request to copy engine code: it is the contract the engine will hold up its side of, and the lessons
its own music work paid for.

## The ruling

Jesse, 2026-10-26: composition leaves the engine for MIDI Evil. MIDI Evil is the music maker. The
engine keeps the conductor's half — it reads a Standard MIDI File and a sidecar beside it, and the
sidecar says which tracks are *parts* a game triggers (a hop plays the next note of a tune), on which
SoundFont instrument, under which action name. **The crossing is `.mid`, a standard file** — not
`.evil`. What `.evil` carries beyond MIDI (gain, pan, mute, the patch, your timing) is written on
export as standard events, listed below; the engine's loader already folds them.

## What the engine reads

- A Standard MIDI File, type 0 or 1, through `midly` (lenient: a track cut short reads as a shorter
  track, so write complete tracks with `EndOfTrack`). Up to 1 MiB and 262,144 events.
- Its sidecar `<file>.mid.ron` — schema in the engine's `docs/SONG_FORMAT.md`
  (`crates/music_midi/src/song_file.rs` is the code): `parts` select tracks **by name**
  (`Name("galop")`, the track-name meta event `FF 03`) or by index; `instruments` are SoundFont
  presets (bank:program) or the engine's own sample instruments; `bindings` map an action name to a
  part and an instrument. MIDI Evil never needs to write this file. If it ever does, it implements
  the schema; a hand-written one is refused loud, naming the entry.
- Every track **not** selected by a part is background and plays by itself; a selected track never
  plays in the background — it is walked by the game, one unit (the notes starting on one tick) per
  trigger. A cue is one note or one chord, never a phrase.

## The export rules (the engine's side depends on every one)

| rule | why |
|---|---|
| **Track names preserved exactly** on export, UTF-8, spaces allowed | the name is the join key to the sidecar; a renamed track silences the part that selected it, and the engine's hot reload will say "track `flute` is gone; index 1 is now `Flute (lead)` — renamed in MIDI Evil?" |
| **The time signature written at tick 0** as composed (2/4 stays 2/4, 5/4 stays 5/4), not a 4/4-based meter | a song without one is 4/4 in the engine, and its beat visuals then land on every other bar line (the Conductor effort paid for this twice) |
| **The tempo at tick 0**, and tempo changes as meta events | the engine's tempo map is exact across changes |
| **Ticks per quarter the author chooses**, 480 by default for the engine (today's export is 128) | the six shipped songs are at 480; finer grids survive quantization choices |
| **Per-track gain → CC 7 (channel volume), pan → CC 10, a muted track exported as a named track with no notes** (never left out: the engine's sidecar may select tracks by index, and a missing track shifts every index after it), a **program change at tick 0** from the patch (General MIDI number where one fits) | the engine's background folds controllers and programs at load, seek and loop wrap, so these are the only way the mix crosses; the patch itself cannot — MIDI Evil synthesizes, the engine plays SoundFont presets chosen in the sidecar |
| **The loop ends where the last written note ends** — hold the last note to the bar line if the loop must end on it | the engine loops a song at its last note-off; a final bar of rest wraps early |
| **Channel 10 is percussion** in the engine's synthesizer, bank +128 | a melodic track exported on channel 10 plays drums |
| **Atomic write**: a temp file in the same directory, then rename | the engine's watcher polls file stamps every 500 ms and debounces one poll; a half-written `.mid` must never be readable at the path |
| Never write `<file>.mid.ron` | see above |
| Never touch `midi_fingerprint` in the sidecar (you never touch the sidecar) | the engine writes it on its own saves for crash recovery and, on open, reads a mismatch with no save journal present as "the score changed outside" — your export — and carries on |

## The CLI the engine will spawn

```
midi-evil [--project <dir>] [--track <name>] [--track-index <n>] <file.mid>
```

- `<file.mid>`: an absolute path to open. For MIDI Evil that is an import; where the working `.evil`
  lives is MIDI Evil's business (beside the `.mid`, or in its own store keyed by the path). The
  engine then expects the **export back to that same `.mid`** on save.
- `--track <name>`: select and scroll to the track whose name meta event matches; unknown → open
  anyway, say so on stderr, exit 0. `--track-index <n>` (0-based, the file's track order) names
  the track exactly where two share a name; when both are given the index wins.
- **No lock.** Each file has one writer: you write the `.mid`, the engine writes the `.mid.ron`,
  and neither touches the other's file (the engine's ruling (i), 2026-10-27). Write the `.mid`
  atomically — a temp file in the same directory, then a rename — and the engine's watcher never
  reads a half-written score.
- `--project <dir>`: the game's asset base; a hint for dialogs and relative paths. Optional.
- Exit 0 once the request is accepted (the window may still be loading); non-zero with one stderr
  line otherwise — the engine logs that line and reads nothing else. The engine never waits.
- **Single instance**: a second invocation on an open file focuses the running window and applies
  `--track`, instead of a second window. The engine cannot tell whether one is open.

Until the flags exist, the engine's "Custom command" field works with `{file}` alone (`cargo run -p
midi-evil -- {file}`), so nothing in the engine waits on this list.

## The issues, in the order the engine needs them

1. **`.evil` → `.mid` export** with the rules above (today's `midi export` is for NanoStudio
   projects only). This is the gate on the engine retiring its own note editing — until a
   round trip exists, the engine keeps its authoring verbs.
2. Open a `.mid` given positionally (import), and export back to it on save.
3. `--track <name>` and `--track-index <n>` (the index wins when both are given).
4. Single instance.
5. `--project <dir>`.
6. **A fixture pair** the engine's track diff can test against: a `.mid` and the same file with one
   track renamed, under a LICENSE line (public domain, as the Bach invention is). The engine mirrors
   it into `crates/music_midi/fixtures/` with its own LICENSE line.

## Test material you already have access to

The six games' songs, each sequenced through the engine's command API and public domain:
`~/projects/insiculous/games/<game>/assets/ai/music/ai_<song>.mid` with its `.mid.ron` beside it —
`pong/ai_tong`, `breakout/ai_food_pyramid`, `snake/ai_bratdog`, `space_invaders/ai_toast_busters`,
`asteroids/ai_meatieroids` (5/4 at 160 BPM, a good meter test), `frogger/ai_chicken_coop` (2/4 at
152). Import one, export it, and the engine's loader is the oracle: `cargo run --bin editor
--features editor -- ../games/pong` opens the song in its panel, and `cargo test` in the game runs
fifteen music tests over the committed file (the meter, the loop end, the parts' units, the bank's
key ranges). A round trip that keeps those green is the acceptance.

## Lessons from the engine's own music tool, offered

- **Validate before you touch the playing thing.** The engine loads a song's whole closure (MIDI,
  sidecar, SoundFonts, samples) and validates it before the playing song is replaced; a refusal
  leaves the old one playing and names the file, the collection and the entry (`parts['drums']`).
- **The audio thread never allocates or parses.** Everything is built on another thread and handed
  over as a pointer; the retired song is dropped off the audio thread too. A `try_lock` per block is
  the most the audio thread holds.
- **A note-off names the note it ends, not its key**: the engine's voice ledger gives every note a
  ticket so a re-struck key never has its newer note cut by the older note-off. Worth stealing for
  a piano roll's preview.
- **Same-key overlaps pair first-in first-out** and are reported, never refused; export writes them
  back as they were paired.
- **Comments carry reasons, not tags**; file names and function names carry meaning; every test is
  named for the behaviour a user would notice. The engine's `training.md` § Comment Policy is the
  worked version if the house style is wanted here too.
- **Verification is adversarial**: the engine's plans and large diffs are reviewed by another
  vendor's model before they land, and every finding is accepted or rebutted in writing. The
  working set's `.claude/skills/adversarial-review/` carries the tooling; it works from any repo
  that copies the skill and `scripts/request-review.sh`.

## 2026-11-03 — the engine's settings panel landed; what MIDI Evil must change

Added 2026-11-03 after reading the engine's batch 6 (`insiculous_2d` 50962e2, the External Tools
settings panel, review rounds 32 to 38). The contract above is unchanged. What changed is that the
engine now ships a **MIDI Evil preset** and a **Test button** that spawn this repo's binary with the
full flag set, so the gaps below are hit by a click, not by a hand-typed command.

### What the engine does now

- Edit → External Tools… holds a Music category whose Tool cycle offers "MIDI Evil". Its command
  template is fixed in the engine's `crates/editor/src/external_tools/presets.rs`:
  `midi-evil [--project {project}] [--track {track}] [--track-index {track_index}] {file}`. Each
  bracketed group is dropped when its placeholder is empty.
- The preset's **Program field** replaces the bare `midi-evil` with a path the user types, for
  example `/opt/midi-evil/target/release/midi-evil`. Left blank, the engine looks for `midi-evil`
  on `PATH` only: the preset has **no fallback locations and no Flatpak id**, so a build that is not
  on `PATH` or named in that field is "not found", and the error sends the user back to the panel.
- **Every song launch names a track.** Opening a `.mid` that has a sidecar with parts resolves the
  first part's track and passes `--track <name> --track-index <n>`; a nameless track gets
  `--track-index <n>` alone. Only a song with no sidecar, or a sidecar with no parts, launches with
  the file alone. The Music **Test** button opens the project's first song the same way and prints
  the exact argv it ran, for example
  `Test Music: midi-evil --project /game/assets --track drums --track-index 2 /game/assets/music/theme.mid`,
  then the launch's own line. The engine never waits and never reads stdout.

### Required of MIDI Evil, in order

1. **Accept `--track-index <n>` (and `--track-index=<n>`). Done 2026-11-03.** Before this the
   launcher rejected it as an unknown option and exited non-zero, so the preset failed on every
   song that has a part, which is all six shipped songs. The index is 0-based in the file's track
   order and **wins when both flags are given**; `--track` alone keeps its behaviour. The import
   drops file tracks holding no events (a tempo-only track 0, a muted track exported empty), so the
   app maps the file index through the import's record of where each project track came from; a
   project opened from `.evil` uses its own order. An index with no track warns on stderr, opens
   anyway and exits 0. The field has a serde default so single-instance forwarding keeps accepting
   an older primary's JSON.
2. **Say where the binary is.** The README's install step should produce a `midi-evil` on `PATH`
   (`cargo install --path crates/midi-evil` or a symlink to `target/release/midi-evil`), or tell the
   user to paste the release path into the preset's Program field. Nothing else finds it.
3. **Keep the exit contract strict.** Exit 0 once the window is up or the request was forwarded, and
   one stderr line otherwise. An unknown future flag should still be a one-line refusal, never a
   panic, since the engine logs only the first line.

### Status of the original issue list, from this side

| issue | state |
|---|---|
| 1. `.evil` → `.mid` export with the rules above | done (`evil-exports`; see `ARCHIVE.md`) |
| 2. open a positional `.mid` and export back to it on save | done |
| 3. `--track <name>` / `--track-index <n>` | done; the index wins when both are given |
| 4. single instance | done, Unix socket forwarding |
| 5. `--project <dir>` | done |
| 6. fixture pair under a LICENSE line | done, the Bach original/renamed pair |

The engine's batch 7 (retiring its own note editing) waits on issues 1 and 2, which are done here.
What remains for the preset is item 2 above: a `midi-evil` the engine can find.

Questions go to Jesse; the engine's planner reads `coordination/external-tools/plan.md` and will
keep its § "MIDI Evil's half" in step with what lands here.
