// The matrix: the native gate's rows, in the browser. Each starts from the fixture its native row
// opens (native-gate.sh's own table: a row with no fixture of its own opens the Bach project)
// and first asserts that project by its name and its track count, so no row can pass on the
// application's default project. A native step that turns on F6 is taken on the browser's region
// chord, and a native press of a named button is that button reached by Tab and pressed with
// Enter. web-app-probe.mjs runs them on the gate's bundle: the W0 and R rows as the matrix group,
// the surfaces' sequences as a group of their own, each under its own deadline.

import { PLAYBACK, enterTheEditor, plain, walkEveryStop } from "./web-app-rows.mjs";

// The project the row opened: its name in the Project name field and its track count in the
// arranger's name. Leaves egui's focus on the arranger.
async function expectFixture(probe, project, tracks) {
  await enterTheEditor(probe);
  const named = await probe.pressUntil("Tab", "Project name");
  probe.check(plain(named.current).endsWith(`text field, ${project}`), `the project opened is not ${project}: ${named.current}`);
  await probe.press("Escape");
  const song = await probe.pressUntil("Alt+PageDown", "Song,");
  probe.check(
    plain(song.current).startsWith(`Song, track 1 of ${tracks},`),
    `the project opened does not have ${tracks} tracks: ${song.current}`,
  );
  return song;
}

// The fixture, then Enter in the arranger: the first track's unique timeline in the roll, as the
// native rows' PIANO ROLL tab and F6 reach it.
async function intoTheRoll(probe, project, tracks) {
  await expectFixture(probe, project, tracks);
  const opened = await probe.press("Enter");
  probe.check(plain(opened.current).startsWith("Piano roll"), `Enter in the arranger reached ${opened.current}`);
  return opened;
}

const bach = (probe) => intoTheRoll(probe, "bach-invention-02", 2);

// A key, and what must then be current and written; `writes` of null leaves the writes unjudged.
async function step(probe, chord, current, writes = null) {
  const state = await probe.press(chord);
  if (current !== null) {
    probe.check(plain(state.current).includes(current), `after ${chord} the current element says ${JSON.stringify(state.current)}, not ${JSON.stringify(current)}`);
  }
  if (writes !== null) probe.expectWrites(state, writes, chord);
  return state;
}

// A button of the editor's, reached from the roll by Shift+Tab and pressed with Enter; then Tab
// back to the roll, whose name is returned.
async function pressFromTheRoll(probe, name, writes = null) {
  await probe.pressUntil("Shift+Tab", `${name}, button`);
  const pressed = await probe.press("Enter", `Enter on ${name}`);
  if (writes !== null) probe.expectWrites(pressed, writes, name);
  return probe.pressUntil("Tab", "Piano roll");
}

// A control anywhere, reached by Tab from where egui's focus is, and pressed with Enter.
async function pressByTab(probe, name, writes = null) {
  await probe.pressUntil("Tab", name);
  const pressed = await probe.press("Enter", `Enter on ${name}`);
  if (writes !== null) probe.expectWrites(pressed, writes, name);
  return pressed;
}

// A held key: pressed, repeated, then released. Nothing is written while it repeats; returns
// every write from the first press to the release, and the state at rest.
async function hold(probe, key, repeats) {
  await probe.page.keyboard.down(key);
  await probe.settle();
  const first = await probe.read();
  probe.describe(`${key} pressed`, first);
  for (let count = 0; count < repeats; count += 1) {
    await probe.page.keyboard.down(key);
    await probe.settle(0);
  }
  const held = await probe.read();
  probe.describe(`${key} held`, held);
  probe.expectWrites(held, [], `while ${key} is held`);
  await probe.page.keyboard.up(key);
  await probe.settle(200);
  const rested = await probe.read();
  probe.describe(`${key} released`, rested);
  return { writes: [...first.writes, ...rested.writes].map(plain), rested };
}

// The browser's own chords, every one of which the roll's description must name, and the ones a
// browser keeps, none of which it may.
const ROLL_CHORDS = [
  "Left previous note", "Right next note", "Home first note", "End last note",
  "Up a semitone higher", "Down a semitone lower", "Shift+Up an octave higher",
  "Shift+Down an octave lower", "Control+Left moves the note earlier",
  "Control+Right moves the note later", "Shift+Left shorter", "Shift+Right longer",
  "Enter adds a note", "Delete removes the note", "Escape clears the selection",
  "I reads the transport", "Alt+Page Down and Alt+Page Up",
];
const KEPT_BY_A_BROWSER = ["F6", "Alt+Left", "Alt+Right", "Control+N", "Control+O"];

export const MATRIX_ROWS = {
  "w0-edge-top-pitch": {
    title: "W0 edge: Up on a note at pitch 127",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      await step(probe, "ArrowRight", "C 5, bar 1 beat 1.5");
      for (let count = 0; count < 4; count += 1) await probe.press("Shift+ArrowUp");
      await step(probe, "Shift+ArrowUp", "Piano roll, G 9, bar 1 beat 1.5");
      await step(probe, "ArrowUp", "Piano roll, G 9, bar 1 beat 1.5", ["Cannot go higher."]);
      await step(probe, "ArrowUp", "Piano roll, G 9, bar 1 beat 1.5", ["Cannot go higher."]);
    },
  },

  "w0-edge-tick-zero": {
    title: "W0 edge: Control+Left on a note at tick 0",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      await step(probe, "ArrowRight", "C 5, bar 1 beat 1.5");
      await probe.press("Control+ArrowLeft");
      await step(probe, "Control+ArrowLeft", "Piano roll, C 5, bar 1 beat 1, ");
      await step(probe, "Control+ArrowLeft", "Piano roll, C 5, bar 1 beat 1, ", ["Cannot move earlier."]);
      await step(probe, "Control+ArrowLeft", "Piano roll, C 5, bar 1 beat 1, ", ["Cannot move earlier."]);
    },
  },

  "w0-edge-empty-pattern": {
    title: "W0 edge: Enter then Delete in an empty pattern",
    fixture: "bach",
    async run(probe) {
      await expectFixture(probe, "bach-invention-02", 2);
      await pressByTab(probe, "+ Pattern part", ["Added a part playing the new Pattern 1."]);
      await probe.pressUntil("Alt+PageDown", "Song,");
      await step(probe, "Enter", "Piano roll, empty");
      await step(probe, "Enter", "Piano roll, C 4, pattern bar 1 beat 1, 1 beat, velocity 100", ["Added."]);
      await step(probe, "Delete", "Piano roll, empty", ["Removed."]);
      await step(probe, "Delete", "Piano roll, empty", ["Nothing here yet."]);
      await step(probe, "Delete", "Piano roll, empty", ["Nothing here yet."]);
    },
  },

  "w0-edge-full-shared-pattern": {
    title: "W0 edge: Enter after the last note of a full shared pattern",
    fixture: "bach",
    async run(probe) {
      await expectFixture(probe, "bach-invention-02", 2);
      await pressByTab(probe, "+ Pattern part");
      await pressByTab(probe, "Duplicate loop", ["Duplicated "]);
      await probe.pressUntil("Alt+PageDown", "Song,");
      await step(probe, "Enter", "Piano roll, empty");
      for (let count = 0; count < 3; count += 1) await probe.press("Enter", "Enter, a note");
      await step(probe, "Enter", "Piano roll, C 4, pattern bar 1 beat 4, 1 beat, velocity 100", ["Added to 2 parts."]);
      await step(probe, "Enter", "pattern bar 1 beat 4", ["Nothing added: the pattern ends here."]);
      await step(probe, "Enter", "pattern bar 1 beat 4", ["Nothing added: the pattern ends here."]);
    },
  },

  "r1-identical-twins": {
    title: "R-1: two identical notes; the inspector changes one's velocity over several frames; the cursor never moves to the twin",
    fixture: "twins",
    async run(probe) {
      await intoTheRoll(probe, "twins", 1);
      await step(probe, "ArrowRight", "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 100");
      // A value set three times, each in an edit of its own and a frame apart, as the native
      // row's three SetValue actions are.
      for (const velocity of [99, 98, 97]) {
        await probe.pressUntil("Tab", "Velocity");
        await probe.press("Control+a");
        await probe.type(String(velocity));
        await step(probe, "Enter", null, [`Velocity, ${velocity}`]);
        const roll = await probe.pressUntil("Shift+Tab", "Piano roll");
        probe.check(plain(roll.current).startsWith(`Piano roll, C 4, bar 1 beat 1, 1 beat, velocity ${velocity}`), `after the velocity ${velocity} the roll is on ${roll.current}`);
      }
      await step(probe, "ArrowLeft", null);
      await step(probe, "Home", "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 97");
      await step(probe, "ArrowRight", "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 100");
      await step(probe, "ArrowLeft", "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 97");
    },
  },

  "r3-regions-from-the-roll": {
    title: "R-3: a bass part; the region chord from the roll reaches the toolbar, and back",
    fixture: "bass",
    async run(probe) {
      await intoTheRoll(probe, "bass", 1);
      await step(probe, "ArrowRight", "Piano roll, C 2, bar 1 beat 1");
      const toolbar = await step(probe, "Alt+PageDown", "Transport, ");
      probe.check(!plain(toolbar.current).includes("Piano roll"), "the region chord stayed in the roll");
      await step(probe, "Alt+PageUp", "Piano roll, C 2, bar 1 beat 1");
      await probe.expectFocus("Piano roll, C 2, bar 1 beat 1");
    },
  },

  "r4-no-automation-lane": {
    title: "R-4, not applicable: the Automation panel is absent; Delete in the roll takes the note, and no stop is a lane",
    fixture: "lane-point",
    async run(probe) {
      await intoTheRoll(probe, "Lane point", 1);
      await step(probe, "ArrowRight", "Piano roll, C 4, bar 1 beat 1");
      await step(probe, "Delete", "Piano roll, D 4, bar 1 beat 2", ["Removed. Piano roll, D 4, bar 1 beat 2"]);
      for (let count = 0; count < 40; count += 1) {
        const state = await probe.press("Tab");
        if (state.activeId === "midi-evil-leave") break;
        probe.check(!/automation/i.test(plain(state.current)), `a stop names the automation: ${state.current}`);
      }
    },
  },

  "r6-undo-an-add-in-an-empty-pattern": {
    title: "R-6: in an empty pattern, add a note and undo the add: the selection was reset",
    fixture: "bach",
    async run(probe) {
      await expectFixture(probe, "bach-invention-02", 2);
      await pressByTab(probe, "+ Pattern part");
      await probe.pressUntil("Alt+PageDown", "Song,");
      await step(probe, "Enter", "Piano roll, empty");
      await step(probe, "Enter", "Piano roll, C 4, pattern bar 1 beat 1");
      await step(probe, "Control+z", "Piano roll, empty", ["The selection was reset."]);
    },
  },

  "r7-held-keys": {
    title: "R-7: Right held across twenty notes, and Up held at the top: one write at rest",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      await step(probe, "ArrowRight", "Piano roll, C 5, bar 1 beat 1.5");
      const right = await hold(probe, "ArrowRight", 18);
      probe.check(plain(right.rested.current).startsWith("Piano roll, D 5, bar 2 beat 2.25"), `Right held rests at ${right.rested.current}`);
      probe.expectWrites(right.rested, ["Piano roll, D 5, bar 2 beat 2.25"], "Right released");
      for (let count = 0; count < 5; count += 1) await probe.press("Shift+ArrowUp");
      await step(probe, "ArrowLeft", null);
      await step(probe, "ArrowRight", "Piano roll, G 9, ");
      for (const time of ["once", "again"]) {
        const up = await hold(probe, "ArrowUp", 5);
        probe.check(
          up.writes.length === 1 && up.writes[0].includes("Cannot go higher."),
          `Up held at the top ${time}: ${JSON.stringify(up.writes)}, not "Cannot go higher." once`,
        );
        probe.check(plain(up.rested.current).startsWith("Piano roll, G 9, "), `Up held at the top moved the note: ${up.rested.current}`);
      }
    },
  },

  "r8-delete-the-last-track": {
    title: "R-8: delete the last track from the Notes view; the region chord's editor stop is the add-a-track button",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      for (let count = 0; count < 2; count += 1) {
        await probe.pressUntil("Alt+PageDown", "01");
        await pressByTab(probe, "Track actions");
        await pressByTab(probe, "Delete track", [count === 0 ? "1 track left" : "0 tracks left"]);
      }
      await probe.pressUntil("Alt+PageUp", "+ Add a track to begin composing");
      await probe.expectFocus("+ Add a track to begin composing");
      // The button goes with the empty editor it stood in, and the focus with it: one write
      // says both.
      await step(probe, "Enter", null, ["Added Track 1, a synth track with the Lead sound. Nothing in the editor has focus"]);
    },
  },

  "r9-add-after-a-twin-and-undo": {
    title: "R-9: identical notes; select the first, add after it, undo: back on the first",
    fixture: "twins",
    async run(probe) {
      await intoTheRoll(probe, "twins", 1);
      await step(probe, "ArrowRight", "Piano roll, C 4, bar 1 beat 1, 1 beat");
      await step(probe, "Enter", "Piano roll, C 4, bar 1 beat 2, 1 beat", ["Added."]);
      await step(probe, "Control+z", "Piano roll, C 4, bar 1 beat 1, 1 beat");
      await step(probe, "Delete", "Piano roll, C 4, bar 1 beat 1, 1 beat", ["Removed."]);
      await step(probe, "Delete", "Piano roll, empty", ["Removed."]);
    },
  },

  "r10-the-rolls-description": {
    title: "R-10: the roll's description names every chord of the browser's table and none a browser keeps",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      const node = await probe.focusedNode();
      for (const chord of ROLL_CHORDS) {
        probe.check(node.description.includes(chord), `the roll's description lacks ${JSON.stringify(chord)} (${node.judge})`);
      }
      for (const chord of KEPT_BY_A_BROWSER) {
        probe.check(!node.description.includes(chord), `the roll's description names ${chord} (${node.judge})`);
      }
    },
  },

  "r12-every-stop-named": {
    deadlineSeconds: 150,
    title: "R-12: every stop Tab reaches on the Song and Notes views, in a menu and in each window: named, with a role",
    fixture: "bach",
    async run(probe) {
      const named = (where) => (state) => {
        const [name, role] = plain(state.current).split(", ");
        probe.check(Boolean(name) && !name.startsWith("Nothing in the editor has focus"), `${where}: a stop has no name: ${state.current}`);
        probe.check(Boolean(role), `${where}: the stop ${JSON.stringify(name)} says no role or value`);
      };
      await expectFixture(probe, "bach-invention-02", 2);
      await walkEveryStop(probe, "the Song view", named("the Song view"));
      await probe.pressUntil("Alt+PageDown", "Song,");
      await probe.press("Enter");
      await probe.press("ArrowRight");
      await walkEveryStop(probe, "the Notes view, a note under the cursor", named("the Notes view"));
      await probe.pressUntil("Tab", "Project, button");
      await probe.press("Enter");
      await walkEveryStop(probe, "the Project menu", named("the Project menu"));
      await probe.press("Escape", "Escape, closing the menu");
      await probe.pressUntil("Tab", "Project, button");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Keyboard notes…");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Sound: ");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "About MIDI Evil, button");
      await probe.press("Enter");
      await walkEveryStop(probe, "the About, Keyboard notes and Sound library windows", named("the windows"));
    },
  },

  "r13-read-with-no-input": {
    title: "R-13, not applicable: the page is always in the tree; with no input since load the canvas is named and described",
    fixture: "bach",
    async run(probe) {
      const canvas = await probe.page.evaluate(() => {
        const element = document.getElementById("midi-evil-canvas");
        const describedBy = element.getAttribute("aria-describedby");
        return {
          role: element.getAttribute("role"),
          name: element.getAttribute("aria-label"),
          description: document.getElementById(describedBy)?.textContent ?? "",
        };
      });
      probe.check(canvas.role === "application" && canvas.name === "MIDI Evil editor", `the canvas is ${JSON.stringify(canvas)}`);
      probe.check(canvas.description !== "" && canvas.description !== "Loading the editor", `the canvas is described by ${JSON.stringify(canvas.description)}`);
      const snapshot = await probe.page.locator("#midi-evil-canvas").ariaSnapshot();
      probe.check(snapshot.includes('application "MIDI Evil editor"'), `the canvas's tree entry is ${snapshot}`);
      probe.expectWrites(await probe.read(), [], "with no input since load");
      await expectFixture(probe, "bach-invention-02", 2);
    },
  },

  "r14-one-note-buttons": {
    title: "R-14: every one-note edit from its toolbar button, with the cursor on a note",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      await step(probe, "ArrowRight", "Piano roll, C 5, bar 1 beat 1.5");
      let roll = await pressFromTheRoll(probe, "Later");
      probe.check(plain(roll.current).includes("C 5, bar 1 beat 1.75"), `after Later: ${roll.current}`);
      roll = await pressFromTheRoll(probe, "Later");
      probe.check(plain(roll.current).includes("C 5, bar 1 beat 2, "), `after Later again: ${roll.current}`);
      await step(probe, "ArrowLeft", "Piano roll, B 4, bar 1 beat 1.75");
      await step(probe, "ArrowRight", "Piano roll, C 5, bar 1 beat 2, ");
      roll = await pressFromTheRoll(probe, "Earlier");
      probe.check(plain(roll.current).includes("C 5, bar 1 beat 1.75"), `after Earlier: ${roll.current}`);
      roll = await pressFromTheRoll(probe, "Longer");
      probe.check(plain(roll.current).includes("C 5, bar 1 beat 1.75, 0.5 beats"), `after Longer: ${roll.current}`);
      roll = await pressFromTheRoll(probe, "Shorter");
      probe.check(plain(roll.current).includes("C 5, bar 1 beat 1.75, 0.25 beats"), `after Shorter: ${roll.current}`);
      roll = await pressFromTheRoll(probe, "Add", ["Added."]);
      probe.check(plain(roll.current).includes("C 5, bar 1 beat 2, 1 beat"), `after Add: ${roll.current}`);
      await pressFromTheRoll(probe, "Delete", ["Removed."]);
      await pressFromTheRoll(probe, "Add", ["Added."]);
      await probe.pressUntil("Tab", "Delete note, button");
      await step(probe, "Enter", null, ["Removed."]);
      await probe.pressUntil("Shift+Tab", "Piano roll");
      await step(probe, "Escape", "Piano roll, no note selected");
      await probe.pressUntil("Shift+Tab", "Earlier, button");
      await step(probe, "Enter", "Earlier, button, unavailable", ["Select a note first."]);
      await probe.pressUntil("Tab", "Delete, button");
      await step(probe, "Enter", "Delete, button, unavailable", ["Select a note first."]);
    },
  },

  "r15-duplicate-a-track-and-undo": {
    title: "R-15: duplicate a track, select the copy's note, undo the duplication",
    fixture: "two-tracks",
    async run(probe) {
      await expectFixture(probe, "two-tracks", 2);
      await probe.pressUntil("Alt+PageDown", "01");
      await pressByTab(probe, "Track actions");
      await pressByTab(probe, "Duplicate track", ["Duplicated as one Copy, track 2 of 3."]);
      await probe.pressUntil("Alt+PageDown", "Song,");
      await step(probe, "Enter", "Piano roll");
      await step(probe, "ArrowRight", "Piano roll, C 4, bar 1 beat 1");
      await step(probe, "Control+z", "Piano roll, no note selected", ["The selection was reset."]);
      await step(probe, "Delete", null, ["Select a note first."]);
    },
  },

  "r16-nudge-past-a-neighbour": {
    title: "R-16: cursor on the first of three; Control+Right past the second; undo",
    fixture: "three-notes",
    async run(probe) {
      await intoTheRoll(probe, "three-notes", 1);
      await step(probe, "ArrowRight", "Piano roll, C 4, bar 1 beat 1, ");
      await step(probe, "Control+ArrowRight", "Piano roll, C 4, bar 1 beat 1.25, ");
      await step(probe, "Control+z", "Piano roll, C 4, bar 1 beat 1, ");
      await step(probe, "ArrowRight", "Piano roll, D 4, bar 1 beat 1.125, ");
    },
  },

  "r17-clean-overlaps-re-sorts": {
    title: "R-17, first fixture: select a note, Clean overlaps re-sorts it, return, delete the same note",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      await probe.press("ArrowRight");
      await step(probe, "ArrowRight", "Piano roll, B 4, bar 1 beat 1.75, ");
      await probe.press("Control+ArrowLeft");
      await step(probe, "Control+ArrowLeft", "Piano roll, B 4, bar 1 beat 1.25, ");
      const roll = await pressFromTheRoll(probe, "Clean overlaps");
      probe.check(plain(roll.current).includes("Piano roll, B 4, bar 1 beat 1.25, "), `after Clean overlaps: ${roll.current}`);
      await step(probe, "Delete", "Piano roll, C 5, bar 1 beat 1.5, ", ["Removed."]);
      await step(probe, "ArrowLeft", "Piano roll, C 5, bar 1 beat 1.5, ", ["Nothing before this one."]);
    },
  },

  "r17-clean-overlaps-shortens": {
    title: "R-17, second fixture: a same-pitch overlap that Clean shortens; the cursor follows by start and pitch",
    fixture: "same-pitch-overlap",
    async run(probe) {
      await intoTheRoll(probe, "same-pitch-overlap", 1);
      await step(probe, "ArrowRight", "Piano roll, C 4, bar 1 beat 1, 2 beats");
      const roll = await pressFromTheRoll(probe, "Clean overlaps", ["Cleaned the overlaps of all 2 notes."]);
      probe.check(plain(roll.current).includes("Piano roll, C 4, bar 1 beat 1, 1 beat"), `after Clean overlaps: ${roll.current}`);
    },
  },

  "r19-escape-clears-then-leaves": {
    title: "R-19: Escape clears the cursor and says operations apply to all notes; again, it leaves",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      await step(probe, "ArrowRight", "Piano roll, C 5, bar 1 beat 1.5");
      await step(probe, "Escape", "Piano roll, no note selected", ["No note selected; Quantize, transpose and Copy apply to all notes."]);
      await pressFromTheRoll(probe, "Quantize");
      const left = await probe.press("Escape");
      probe.check(!plain(left.current).startsWith("Piano roll"), `a second Escape stayed in the roll: ${left.current}`);
    },
  },

  "r21-tempo-and-a-new-project": {
    title: "R-21: change the tempo and read; New with unsaved work refuses and says why; on a fresh page, New and read",
    fixture: "tempo",
    async run(probe) {
      await expectFixture(probe, "tempo", 1);
      await step(probe, "Alt+PageDown", "Transport, stopped, tempo 90 beats per minute");
      await probe.pressUntil("Tab", "Tempo in beats per minute");
      await probe.press("Control+a");
      await probe.type("100");
      await step(probe, "Enter", null, ["Tempo in beats per minute, 100"]);
      await probe.pressUntil("Shift+Tab", "Transport");
      await step(probe, "Enter", "Transport, stopped, tempo 100 beats per minute", ["Transport, stopped, tempo 100 beats per minute"]);
      await probe.pressUntil("Shift+Tab", "Project, button");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "New, button");
      const refused = await probe.press("Enter");
      probe.check(refused.writes.length === 1 && !plain(refused.writes[0]).startsWith("New project"), `New with unsaved work wrote ${JSON.stringify(refused.writes)}`);
      await probe.open(probe.url);
      await expectFixture(probe, "tempo", 1);
      await probe.pressUntil("Shift+Tab", "Project, button");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "New, button");
      await step(probe, "Enter", null, ["New project, 1 track."]);
      await probe.pressUntil("Alt+PageDown", "Transport");
      await step(probe, "Enter", "Transport, stopped, tempo 120 beats per minute");
    },
  },

  "r22-unavailable-controls": {
    title: "R-22: an unavailable control says why, and says nothing once available: Undo, Redo, and the roll's one-note buttons",
    fixture: "bach",
    async run(probe) {
      await bach(probe);
      await probe.pressUntil("Shift+Tab", "Earlier, button");
      await probe.expectFocus("Earlier, button, unavailable. Select a note first.");
      await probe.pressUntil("Tab", "Piano roll");
      await probe.press("ArrowRight");
      const earlier = await probe.pressUntil("Shift+Tab", "Earlier, button");
      probe.check(!plain(earlier.current).includes("unavailable"), `with a note selected Earlier reads ${earlier.current}`);
      await probe.pressUntil("Tab", "Piano roll");
      await probe.press("Control+ArrowRight");
      const undo = await probe.pressUntil("Shift+Tab", "Undo, button");
      probe.check(plain(undo.current) === "Undo, button", `after an edit Undo reads ${undo.current}`);
      const redo = await probe.pressUntil("Shift+Tab", "Redo, button");
      probe.check(plain(redo.current) === "Redo, button, unavailable. Nothing to redo.", `Redo reads ${redo.current}`);
      await step(probe, "Enter", null, ["Nothing to redo."]);
      // Each undoes or redoes the one edit there is and is then unavailable: one write says
      // both.
      await probe.pressUntil("Tab", "Undo, button");
      await step(probe, "Enter", "Undo, button, unavailable. Nothing to undo.", ["Undid the last edit. unavailable"]);
      await probe.pressUntil("Shift+Tab", "Redo, button");
      await step(probe, "Enter", "Redo, button, unavailable. Nothing to redo.", ["Redid the edit. unavailable"]);
      for (const control of ["PLAY, button", "Record, toggle button"]) {
        const state = await probe.pressUntil("Shift+Tab", control, 150);
        probe.check(plain(state.current).includes(PLAYBACK), `${control} reads ${state.current}`);
      }
    },
  },

  "instrument-list": {
    group: "surfaces",
    title: "the instrument list: the Next and Previous buttons say the sound a track now has and its place",
    fixture: "bach",
    async run(probe) {
      await expectFixture(probe, "bach-invention-02", 2);
      await probe.pressUntil("Shift+Tab", "Sound: Lead, 7 of 12 synth sounds");
      await pressByTab(probe, "Next sound", [" 8 of 12 synth sounds"]);
      await probe.pressUntil("Shift+Tab", "Previous sound");
      await step(probe, "Enter", null, ["Lead, 7 of 12 synth sounds"]);
    },
  },

  "song-by-keyboard": {
    group: "surfaces",
    title: "Song: move between tracks and parts, hear a placement, move a part, B says why it cannot play, read the transport, undo, open it in the roll",
    fixture: "arrangement",
    async run(probe) {
      const song = await expectFixture(probe, "Arrangement", 2);
      probe.check(plain(song.current).startsWith("Song, track 1 of 2, Lead, sound Lead, unique timeline, empty, 2 parts"), `the arranger reads ${song.current}`);
      await step(probe, "ArrowRight", "part 1 of 2, Riff, bar 1 beat 1, 1 bar, shared by 2 parts");
      await step(probe, "ArrowRight", "part 2 of 2, Riff, bar 3 beat 1, 1 bar");
      await step(probe, "ArrowRight", "part 2 of 2, Riff, bar 3 beat 1", ["Nothing after this one."]);
      await step(probe, "Control+ArrowRight", "part 2 of 2, Riff, bar 4 beat 1");
      await step(probe, "Control+z", "part 2 of 2, Riff, bar 3 beat 1");
      await step(probe, "ArrowDown", "Song, track 2 of 2, Bass, sound Lead, unique timeline, 1 note, no parts");
      await step(probe, "ArrowDown", "Song, track 2 of 2, Bass", ["No track below."]);
      await step(probe, "ArrowUp", "Song, track 1 of 2, Lead, sound Lead, unique timeline");
      await step(probe, "End", "part 2 of 2, Riff, bar 3 beat 1");
      await step(probe, "b", "part 2 of 2, Riff, bar 3 beat 1", [PLAYBACK]);
      await step(probe, "i", null, ["Transport, stopped, tempo 120 beats per minute, 4 4 time"]);
      await step(probe, "Enter", "Piano roll", ["Riff of Lead, in the piano roll."]);
      await step(probe, "ArrowRight", "Piano roll, C 4, pattern bar 1 beat 1, 1 beat");
    },
  },

  "song-delete-a-part": {
    group: "surfaces",
    title: "Song: Delete removes the part under the cursor and says so; the cursor goes to the next part",
    fixture: "arrangement",
    async run(probe) {
      await expectFixture(probe, "Arrangement", 2);
      await step(probe, "ArrowRight", "part 1 of 2, Riff, bar 1 beat 1");
      await step(probe, "Delete", "part 1 of 1, Riff, bar 3 beat 1", ["Removed."]);
      await probe.press("Home");
      await step(probe, "Delete", null, ["Select a part first."]);
    },
  },

  "song-buttons": {
    group: "surfaces",
    title: "Song: every arranger button says what it did",
    fixture: "arrangement",
    async run(probe) {
      await expectFixture(probe, "Arrangement", 2);
      await step(probe, "ArrowRight", "part 1 of 2, Riff, bar 1 beat 1");
      for (const [button, said] of [
        ["Loop part", "Looping from bar 1 beat 1 to bar 2 beat 1."],
        ["Duplicate shared", "Duplicated the part; 3 parts now play Riff."],
        ["Make unique", "This part now plays its own pattern, Riff Unique."],
        // The part's buttons go with the part it merges, and the focus with them: one write
        // says both.
        ["Merge to timeline", "Merged into the unique timeline. Nothing in the editor has focus"],
        ["+ Pattern part", "Added a part playing the new Pattern 3."],
        ["Delete part", "Removed."],
        ["Duplicate loop", "Duplicated "],
      ]) {
        await probe.pressUntil("Alt+PageDown", "Song,");
        await pressByTab(probe, button, [said]);
      }
      await probe.pressUntil("Alt+PageDown", "Song,");
      await pressByTab(probe, "Unique timeline", ["The unique timeline of Lead, in the piano roll."]);
    },
  },

  "roll-toolbar": {
    group: "surfaces",
    title: "the roll's toolbar: Quantize, a transpose, Copy and Paste each say what they did",
    fixture: "three-notes",
    async run(probe) {
      await intoTheRoll(probe, "three-notes", 1);
      await pressFromTheRoll(probe, "Quantize", ["Quantized all 3 notes."]);
      await pressFromTheRoll(probe, "+12", ["Moved all 3 notes an octave up."]);
      await pressFromTheRoll(probe, "Copy", ["Copied 3 notes."]);
      await pressFromTheRoll(probe, "Paste", ["Pasted."]);
      await probe.pressUntil("Shift+Tab", "Arranger, button");
      // The button goes with the Notes view, and the focus with it: one write says both.
      await step(probe, "Enter", null, ["The arranger is shown; Alt+Page Down reaches it. Nothing in the editor has focus"]);
    },
  },

  "track-panel": {
    group: "surfaces",
    title: "the track panel: rename, mute and a sound from the library, each said once",
    fixture: "clean",
    async run(probe) {
      await expectFixture(probe, "clean", 1);
      await probe.pressUntil("Alt+PageDown", "01");
      await probe.pressUntil("Tab", "Track name");
      await probe.press("End");
      probe.expectWrites(await probe.type("x"), [], "typing into the name");
      await step(probe, "Enter", null, ["Track name, Cleanx"]);
      await pressByTab(probe, "Mute track", ["muted."]);
      await step(probe, "Space", null, ["unmuted."]);
      await probe.pressUntil("Shift+Tab", "Sound: Lead, 7 of 12 synth sounds");
      await step(probe, "Enter", null, ["The Sound library window is open"]);
      // The sound's button is pressed once it has assigned the sound: one write says both.
      await pressByTab(probe, "Bass   ·   FACTORY", [" assigned to Cleanx; Bass, 11 of 12 synth sounds. pressed"]);
    },
  },

  "track-list": {
    group: "surfaces",
    title: "the track list: duplicate, move down and up, delete, and add a drum track, each said, the view staying where it was",
    fixture: "two-tracks",
    async run(probe) {
      await expectFixture(probe, "two-tracks", 2);
      for (const [item, said] of [
        ["Duplicate track", "Duplicated as one Copy, track 2 of 3."],
        ["Move down", "Moved one Copy to track 3 of 3."],
        ["Move up", "Moved one Copy to track 2 of 3."],
        ["Delete track", "Removed one Copy; 2 tracks left."],
      ]) {
        await probe.pressUntil("Alt+PageDown", "0");
        await pressByTab(probe, "Track actions");
        await pressByTab(probe, item, [said]);
      }
      await probe.pressUntil("Shift+Tab", "+ Track");
      await probe.press("Enter");
      await pressByTab(probe, "Drum track", ["Added Drums 3, a drum track with the Maxwell kit sound."]);
      const song = await probe.pressUntil("Alt+PageDown", "Song,");
      probe.check(plain(song.current).startsWith("Song, track 3 of 3, Drums 3"), `after adding a drum track the editor is ${song.current}`);
    },
  },

  "windows-and-the-project-menu": {
    group: "surfaces",
    title: "the windows and the Project menu without a file: Keyboard notes, Open and Save as saying what they did, About's Open project and Start composing",
    fixture: "clean",
    async run(probe) {
      await expectFixture(probe, "clean", 1);
      await probe.pressUntil("Shift+Tab", "Project, button");
      await probe.press("Enter");
      await pressByTab(probe, "Keyboard notes…", ["The Keyboard notes window is open; Tab reaches it."]);
      // A page cannot yet pick a file: Open and Save as say so, until the browser has dialogs.
      for (const [item, said] of [["Open…", "Nothing opened."], ["Save as…", "Not saved."]]) {
        await probe.pressUntil("Shift+Tab", "Project, button");
        await probe.press("Enter");
        await pressByTab(probe, item, [said]);
      }
      await probe.pressUntil("Shift+Tab", "About MIDI Evil, button");
      await step(probe, "Enter", null, ["The About window is open; Tab reaches it."]);
      // The window closes with the button that had the focus: one write says both.
      await pressByTab(probe, "Open project…", ["Nothing opened. Nothing in the editor has focus"]);
      await probe.pressUntil("Shift+Tab", "About MIDI Evil, button");
      await step(probe, "Enter", null, ["The About window is open; Tab reaches it."]);
      // The window closes with the button that had the focus: one write says both.
      await pressByTab(probe, "Start composing", ["The piano roll is open. Alt+Page Down moves between the toolbar, the side panel and the editor. Nothing in the editor has focus"]);
    },
  },
};
