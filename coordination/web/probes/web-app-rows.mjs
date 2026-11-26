// The rows of the web gate, each a sequence through the real application in a fresh page, with
// what it must find. web-app-probe.mjs runs them; `probe` is its Probe.

// No-break spaces are the bridge's repeat mark; matching ignores them.
export function plain(text) {
  return text.replaceAll("\u00a0", " ").trimEnd();
}

const PLAYBACK = "Playback is not in the browser build yet.";

// Into the editor from the page, then Project › New: the About window the application opens on
// is closed, and the document is a new, clean one.
async function newProjectFromTheMenu(probe) {
  await probe.press("Tab", "Tab from the page");
  await probe.pressUntil("Tab", "Project, button");
  const opened = await probe.press("Enter");
  probe.check(plain(opened.current).includes("menu open"), "the Project menu does not say it is open");
  await probe.pressUntil("Tab", "New, button");
  const created = await probe.press("Enter");
  probe.expectWrites(created, ["New project, 1 track."], "Project › New");
}

// From a new project, the region chord to the arranger, and Enter: the track's unique timeline in
// the roll.
async function openTheRoll(probe) {
  await newProjectFromTheMenu(probe);
  await probe.pressUntil("Alt+PageDown", "Song,");
  const opened = await probe.press("Enter");
  probe.check(plain(opened.current).startsWith("Piano roll, empty"), "Enter in the arranger did not open the roll");
}

async function addNotes(probe, count) {
  for (let index = 0; index < count; index += 1) await probe.press("Enter", "Enter, a note");
}

// A chord the browser keeps: it never reaches eframe, is not cancelled, and the application does
// nothing and writes nothing.
async function expectTheBrowsers(probe, chord) {
  const before = probe.last.current;
  const state = await probe.press(chord);
  // A chord's modifiers are keys of their own, pressed first; the record wanted is the chord's key.
  const wanted = chord.split("+").at(-1).toLowerCase();
  const key = state.keys.find((record) => record.key.toLowerCase() === wanted);
  probe.check(key && !key.reached, `${chord} reached eframe`);
  probe.check(key && !key.cancelled, `${chord} was cancelled`);
  probe.expectWrites(state, [], `${chord}, which is the browser's`);
  probe.check(state.current === before, `${chord} changed what is current`);
}

// Shift+Tab at the first stop: the page keeps the key from eframe and leaves it to the browser,
// uncancelled. Where the browser puts focus next is its own: Chromium moves it off the canvas;
// headless Firefox, with nothing before the editor and no window around the page, leaves it
// where it is, which a real Firefox, with its toolbar, does not — the sitting's. Either way the
// focus is then put on the heading, as on any element before the editor.
async function leaveBackwards(probe) {
  const state = await probe.press("Shift+Tab", "Shift+Tab at the first stop");
  const key = state.keys.find((record) => record.key === "Tab");
  probe.check(key && !key.reached && !key.cancelled, "Shift+Tab at the first stop was not left to the browser");
  probe.expectWrites(state, [], "leaving backwards");
  if (probe.browserName === "chromium") {
    probe.check(state.activeId !== "midi-evil-canvas" && state.activeId !== "midi-evil-text-entry", "Shift+Tab at the first stop stayed in the editor");
  } else {
    probe.log(`    Firefox left the focus on ${state.activeTag}#${state.activeId}: the real browser's next stop is the sitting's`);
  }
  await probe.page.evaluate(() => document.getElementById("midi-evil-heading").focus());
  await probe.settle();
  await probe.read();
}

export const ROWS = {
  "enter-from-the-page": {
    title: "Tab from the page enters at the first stop, named and described",
    async run(probe) {
      const state = await probe.press("Tab", "Tab from the page");
      probe.check(state.activeId === "midi-evil-canvas" && state.edge === "first", "Tab from the page did not enter at the first stop");
      probe.expectWrites(state, [], "entering");
      await probe.expectFocus("About MIDI Evil, button");
    },
  },

  "nine-steps": {
    title: "the nine steps as far as the browser build goes",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      await probe.pressUntil("Tab", "Add a synth track");
      probe.expectWrites(await probe.press("Enter"), ["Added Track 2, a synth track"], "add an instrument");
      // The sound bar's buttons come before the track rack's in Tab's walk.
      await probe.pressUntil("Shift+Tab", "Next sound");
      probe.expectWrites(await probe.press("Enter"), ["synth sounds"], "Next sound");
      await probe.pressUntil("Shift+Tab", "Previous sound");
      probe.expectWrites(await probe.press("Enter"), ["synth sounds"], "Previous sound");
      await probe.pressUntil("Alt+PageDown", "Song,");
      probe.expectWrites(await probe.press("Enter"), ["in the piano roll. Piano roll, empty"], "open the roll");
      probe.expectWrites(await probe.press("Enter"), ["Added. Piano roll, C 4, bar 1 beat 1"], "add a note");
      await probe.expectFocus("Piano roll, C 4, bar 1 beat 1");
      probe.expectWrites(await probe.press("Escape"), ["No note selected"], "clear the selection");
      probe.expectWrites(await probe.press("Home"), ["Piano roll, C 4, bar 1 beat 1"], "select it");
      probe.expectWrites(await probe.press("ArrowUp"), ["Piano roll, C sharp 4"], "change it");
      await probe.expectFocus("Piano roll, C sharp 4");
      probe.expectWrites(await probe.press("Delete"), ["Removed. Piano roll, empty"], "delete it");
      probe.expectWrites(await probe.press("i"), ["Transport, stopped"], "read the transport");
      probe.expectWrites(await probe.press("Control+s"), ["Not saved."], "save");
      await probe.expectFocus("Piano roll, empty");
    },
  },

  "delete-and-undo": {
    title: "Delete and Undo, across every frame they take: one write, the final selection in it",
    async run(probe) {
      await openTheRoll(probe);
      await addNotes(probe, 3);
      await probe.press("Home");
      const deleted = await probe.press("Delete");
      probe.expectWrites(deleted, ["Removed. Piano roll, C 4, bar 1 beat 2"], "Delete");
      await probe.settle(400);
      probe.expectWrites(await probe.read(), [], "the frames after the Delete");
      const undone = await probe.press("Control+z");
      probe.expectWrites(undone, ["Undid the last edit. Piano roll, C 4, bar 1 beat 1"], "Undo");
      await probe.settle(400);
      probe.expectWrites(await probe.read(), [], "the frames after the Undo");
      await probe.expectFocus("Piano roll, C 4, bar 1 beat 1");
    },
  },

  "three-notices": {
    title: "three identical notices, three writes",
    async run(probe) {
      await openTheRoll(probe);
      await addNotes(probe, 1);
      const raw = [];
      for (let count = 0; count < 3; count += 1) {
        const state = await probe.press("ArrowLeft", "Left at the first note");
        probe.expectWrites(state, ["Nothing before this one."], `notice ${count + 1}`);
        raw.push(...state.writes);
      }
      probe.check(raw[0] !== raw[1] && raw[1] !== raw[2], `consecutive writes are identical: ${JSON.stringify(raw)}`);
    },
  },

  "focus-boundary": {
    title: "into the canvas and out in both directions, through every stop, with a window, a menu, the side panel's fields and the leave control",
    async run(probe) {
      // The application opens with its About window: a window open.
      let state = await probe.press("Tab", "Tab from the page");
      probe.check(state.activeId === "midi-evil-canvas" && state.edge === "first", "Tab from the page did not enter at the first stop");
      await probe.expectFocus("About MIDI Evil, button");
      await leaveBackwards(probe);
      state = await probe.press("Tab", "Tab back in");
      probe.check(state.edge === "first", "Tab from before the editor did not enter at the first stop");
      probe.expectWrites(state, [], "returning");
      let stops = 1;
      let sawText = false;
      while (stops < 120) {
        state = await probe.press("Tab");
        if (state.activeId !== "midi-evil-canvas" && state.activeId !== "midi-evil-text-entry") break;
        stops += 1;
        sawText ||= state.activeId === "midi-evil-text-entry";
        probe.check(state.writes.length <= 1, `a Tab wrote ${state.writes.length} times`);
        if (state.activeId === "midi-evil-text-entry") {
          const name = await probe.page.evaluate(() => document.activeElement.getAttribute("aria-label") ?? "");
          probe.check(name !== "" && plain(state.current).startsWith(name), `the text entry is named ${JSON.stringify(name)} where the current element says ${state.current}`);
          await probe.expectFocus(name, name || "a name");
        } else {
          await probe.expectFocus(plain(state.current).slice(0, 40));
        }
      }
      probe.check(state.activeId === "midi-evil-leave", `Tab from the last stop went to ${state.activeId}, not the leave control`);
      probe.check(sawText, "no text field was passed through");
      probe.log(`    ${stops} stops forwards, the About window's among them`);
      state = await probe.press("Shift+Tab", "Shift+Tab from the leave control");
      probe.check(state.edge === "last", "Shift+Tab from after the editor did not enter at the last stop");
      let backwards = 1;
      while (backwards < 120 && state.edge !== "first") {
        state = await probe.press("Shift+Tab");
        backwards += 1;
      }
      probe.check(backwards === stops, `${backwards} stops backwards, ${stops} forwards`);
      await leaveBackwards(probe);
      // A menu open: its items are stops, and the walk still leaves at the end.
      await newProjectFromTheMenu(probe);
      await probe.pressUntil("Tab", "Project, button");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Keyboard notes…");
      state = await probe.press("Enter");
      probe.expectWrites(state, ["The Keyboard notes window is open"], "the Keyboard notes window");
      // The window's text is the last stop, and a text entry: Tab leaves from it.
      state = await probe.pressUntil("Tab", "Keyboard notes, one chord a line");
      probe.check(state.edge === "last", "the window's text is not the last stop");
      state = await probe.press("Tab", "Tab at the window's text");
      probe.check(state.activeId === "midi-evil-leave", `Tab from the window's text went to ${state.activeId}`);
      state = await probe.press("Enter", "Enter on Leave the editor");
      probe.check(state.activeId === "midi-evil-heading", "Leave the editor did not move focus to the heading");
    },
  },

  "held-key": {
    title: "a held key: nothing written while it repeats, one write at rest, matching where the cursor is",
    async run(probe) {
      await openTheRoll(probe);
      await addNotes(probe, 6);
      await probe.press("Home");
      await probe.page.keyboard.down("ArrowRight");
      await probe.settle();
      await probe.read();
      for (let count = 0; count < 4; count += 1) {
        await probe.page.keyboard.down("ArrowRight");
        await probe.settle(0);
      }
      const held = await probe.read();
      probe.describe("ArrowRight held", held);
      probe.expectWrites(held, [], "while Right is held");
      await probe.page.keyboard.up("ArrowRight");
      await probe.settle(200);
      const rested = await probe.read();
      probe.describe("ArrowRight released", rested);
      probe.check(rested.writes.length === 1, `${rested.writes.length} writes at rest, not one`);
      const where = plain(rested.current).split(". ")[0];
      probe.check(plain(rested.writes[0] ?? "") === where, `the write ${JSON.stringify(rested.writes[0])} is not where the cursor is, ${JSON.stringify(where)}`);
      probe.check(where.includes("bar 2 beat 2"), `the cursor rests at ${where}, not the sixth note`);
    },
  },

  "leave-and-return": {
    title: "leaving and returning: nothing written, the current thing readable",
    async run(probe) {
      await openTheRoll(probe);
      await addNotes(probe, 2);
      let state = await probe.press("Tab", "Tab from the roll, the last stop");
      probe.check(state.activeId === "midi-evil-leave", "Tab from the roll did not leave the editor");
      probe.expectWrites(state, [], "leaving");
      state = await probe.press("Shift+Tab", "Shift+Tab back");
      probe.expectWrites(state, [], "returning");
      await probe.expectFocus("Piano roll, C 4, bar 1 beat 2");
    },
  },

  "text-fields": {
    title: "the tempo box and a name field: named, typed into, true after each key and an arrow, committed and heard once, the next key reaching the canvas",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      let state = await probe.pressUntil("Tab", "Tempo in beats per minute");
      probe.check(state.activeId === "midi-evil-text-entry", "the tempo box does not hold the page's focus");
      await probe.expectFocus("Tempo in beats per minute", "Tempo in beats per minute");
      await probe.press("Control+a");
      for (const [key, shown] of [["1", "1"], ["3", "13"], ["0", "130"]]) {
        state = await probe.type(key);
        probe.check(plain(state.current).endsWith(`, ${shown}`), `after typing ${key} the current element says ${state.current}`);
        probe.expectWrites(state, [], `typing ${key}`);
      }
      state = await probe.press("ArrowUp");
      probe.check(plain(state.current).includes("130.1"), `after Up the current element says ${state.current}`);
      probe.expectWrites(state, ["130.1"], "Up");
      state = await probe.press("Enter");
      probe.expectWrites(state, ["Tempo in beats per minute, 130.1"], "Enter");
      await probe.settle(300);
      probe.expectWrites(await probe.read(), [], "after the commit");
      state = await probe.press("Tab", "the next key");
      probe.expectWrites(state, ["Meter, beats per bar over 4"], "the next key reaching the application");
      state = await probe.pressUntil("Tab", "Track name");
      await probe.expectFocus("Track name, text field, Track 1", "Track name");
      await probe.press("End");
      state = await probe.type("x");
      probe.check(plain(state.current).includes("Track 1x"), `the name field says ${state.current}`);
      state = await probe.press("Enter");
      probe.expectWrites(state, ["Track name, Track 1x"], "the name committed");
      await probe.settle(300);
      probe.expectWrites(await probe.read(), [], "after the name's commit");
    },
  },

  "states": {
    title: "a check box, a menu and a disabled button: role and state in the current element, a change of state written once",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      let state = await probe.pressUntil("Tab", "Repeat section");
      probe.check(plain(state.current).startsWith("Repeat section, check box, not checked"), `the check box reads ${state.current}`);
      state = await probe.press("Space");
      probe.expectWrites(state, ["checked"], "Space on the check box");
      probe.check(plain(state.current).startsWith("Repeat section, check box, checked"), `the check box reads ${state.current}`);
      await probe.settle(300);
      probe.expectWrites(await probe.read(), [], "after the check box's change");
      state = await probe.pressUntil("Tab", "Undo");
      probe.check(plain(state.current) === "Undo, button, unavailable. Nothing to undo.", `Undo reads ${state.current}`);
      state = await probe.pressUntil("Shift+Tab", "Project, button");
      state = await probe.press("Enter");
      probe.expectWrites(state, ["menu open"], "opening the menu");
      await probe.expectFocus("Project, button, menu open");
    },
  },

  "browser-chords": {
    title: "chords outside the table never reach eframe and are not cancelled; kept chords cancel the browser's default; the page asks before unsaved work is left",
    sitting: "what the browser then does with each chord it keeps",
    async run(probe) {
      await openTheRoll(probe);
      probe.check(!(await probe.pageAnswersUnsaved()), "a new project's page answers unsaved");
      // An edit and the chord that leaves, in one task: the page must already answer unsaved.
      const sameTask = await probe.pageAnswersUnsaved(
        "document.getElementById('midi-evil-canvas').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));",
      );
      probe.check(sameTask, "an edit and beforeunload in one task: the page did not answer unsaved");
      await probe.settle(300);
      await probe.read();
      probe.check(await probe.pageAnswersUnsaved(), "after an edit the page does not answer unsaved");
      for (const chord of ["Control+n", "Control+o", "Alt+ArrowLeft", "Alt+ArrowRight", "F6", "Shift+F6", "Control+p", "Control+t", "Control+w", "Control+l", "Control+Shift+s", "Control+Alt+s"]) {
        await expectTheBrowsers(probe, chord);
      }
      for (const chord of ["Home", "End", "Enter", "Delete", "Alt+PageDown"]) {
        const state = await probe.press(chord, `${chord}, the application's`);
        const key = state.keys.at(-1);
        probe.check(key?.reached, `${chord} did not reach eframe`);
        probe.check(key?.cancelled, `${chord}'s default was not cancelled`);
        probe.check(state.scroll.every((offset) => offset === 0), `${chord} scrolled the page to ${state.scroll}`);
      }
    },
  },

  "name-field-editing": {
    title: "in a name field: select all and type replaces, a word is deleted, a selection is extended; typing and a paste make the page answer unsaved in the same task",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      await probe.pressUntil("Tab", "Project name");
      probe.check(!(await probe.pageAnswersUnsaved()), "a new project's page answers unsaved");
      const typed = await probe.pageAnswersUnsaved(
        "const entry = document.getElementById('midi-evil-text-entry'); entry.value = 'a'; entry.dispatchEvent(new InputEvent('input', { data: 'a', inputType: 'insertText', bubbles: true }));",
      );
      probe.check(typed, "typing and beforeunload in one task: the page did not answer unsaved");
      await probe.settle(300);
      await probe.press("Control+a");
      let state = await probe.type("Song two");
      probe.check(plain(state.current).includes("text field, Song two"), `select all and type left ${state.current}`);
      state = await probe.press("Control+Backspace");
      probe.check(/text field, Song ?$/.test(plain(state.current)), `a word was not deleted: ${state.current}`);
      await probe.press("Control+Shift+ArrowLeft");
      state = await probe.type("Tune");
      probe.check(plain(state.current).includes("text field, Tune"), `the extended selection was not replaced: ${state.current}`);
      await probe.press("Enter");
      // The document is unsaved now, and New refuses to discard it: a fresh page is a clean one.
      await probe.open(probe.url);
      await newProjectFromTheMenu(probe);
      await probe.pressUntil("Tab", "Project name");
      const pasted = await probe.pageAnswersUnsaved(
        "const data = new DataTransfer(); data.setData('text/plain', 'pasted'); document.getElementById('midi-evil-text-entry').dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true }));",
      );
      probe.check(pasted, "a paste and beforeunload in one task: the page did not answer unsaved");
    },
  },

  "held-up-on-tempo": {
    title: "a held Up on the tempo box: nothing written while it is held, one write at release",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      await probe.pressUntil("Tab", "Tempo in beats per minute");
      await probe.page.keyboard.down("ArrowUp");
      await probe.settle();
      await probe.read();
      for (let count = 0; count < 5; count += 1) {
        await probe.page.keyboard.down("ArrowUp");
        await probe.settle(0);
      }
      const held = await probe.read();
      probe.describe("Up held", held);
      probe.expectWrites(held, [], "while Up is held");
      await probe.page.keyboard.up("ArrowUp");
      await probe.settle(200);
      const rested = await probe.read();
      probe.describe("Up released", rested);
      probe.check(rested.writes.length === 1, `${rested.writes.length} writes at release, not one`);
      probe.check(plain(rested.current).endsWith(`, ${plain(rested.writes[0] ?? "-")}`), `the write ${JSON.stringify(rested.writes[0])} is not the value ${rested.current}`);
    },
  },

  "unavailable": {
    title: "Undo on a clean start and Play: reached by Tab, described as unavailable with the reason, activated, the reason written, nothing changed; Space and B say it too",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      let state = await probe.pressUntil("Tab", "PLAY");
      await probe.expectFocus(`PLAY, button, unavailable. ${PLAYBACK}`);
      state = await probe.press("Enter", "Enter on PLAY");
      probe.expectWrites(state, [PLAYBACK], "PLAY activated");
      probe.check(plain(state.current).startsWith("PLAY,"), "focus left PLAY");
      state = await probe.pressUntil("Tab", "Undo");
      await probe.expectFocus("Undo, button, unavailable. Nothing to undo.");
      state = await probe.press("Enter", "Enter on Undo");
      probe.expectWrites(state, ["Nothing to undo."], "Undo activated");
      probe.check(!state.unsaved, "Undo changed the document");
      await probe.pressUntil("Alt+PageDown", "Song,");
      probe.expectWrites(await probe.press("b", "B in the arranger"), [PLAYBACK], "B");
      probe.expectWrites(await probe.press("Space", "Space in the arranger"), [PLAYBACK], "Space");
    },
  },

  "regions": {
    title: "the region chord reaches the toolbar, the side panel and the editor, and back",
    async run(probe) {
      await openTheRoll(probe);
      const order = [];
      for (let count = 0; count < 3; count += 1) {
        const state = await probe.press("Alt+PageDown");
        order.push(plain(state.current).slice(0, 12));
      }
      probe.check(order[0].startsWith("Transport"), `the first region is ${order[0]}`);
      probe.check(order[1].startsWith("01"), `the second region is ${order[1]}`);
      probe.check(order[2].startsWith("Piano roll"), `the third region is ${order[2]}`);
      const back = await probe.press("Alt+PageUp");
      probe.check(plain(back.current).startsWith("01"), `Alt+Page Up from the editor reached ${back.current}`);
      await probe.expectFocus("01   Track 1");
    },
  },

  "two-windows": {
    title: "with the About window open and the Keyboard notes opened after it, Tab's ends are the ones it walks to",
    async run(probe) {
      // The application opens with its About window; Project › Keyboard notes… opens a second.
      await probe.press("Tab", "Tab from the page");
      await probe.pressUntil("Tab", "Project, button");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Keyboard notes…");
      probe.expectWrites(await probe.press("Enter"), ["The Keyboard notes window is open"], "the second window");
      // The menu closed with the focus on its item, so nothing has the focus: Tab starts at the
      // first stop.
      let state = await probe.press("Tab", "Tab from nothing");
      probe.check(state.edge === "first", `Tab from nothing reached ${state.current}, reported ${state.edge}`);
      let stops = 0;
      let previous = state;
      while (stops < 150) {
        state = await probe.press("Tab");
        if (state.activeId !== "midi-evil-canvas" && state.activeId !== "midi-evil-text-entry") break;
        probe.check(!plain(state.current).startsWith("About MIDI Evil, button"), "Tab wrapped inside the editor before leaving it");
        previous = state;
        stops += 1;
      }
      probe.check(state.activeId === "midi-evil-leave", `Tab left the editor for ${state.activeId}`);
      probe.check(previous.edge === "last", `the stop Tab left from was reported ${previous.edge}, not last`);
      state = await probe.press("Shift+Tab", "Shift+Tab back in");
      probe.check(plain(state.current) === plain(previous.current), `entering from after reached ${state.current}, not the last stop ${previous.current}`);
      let back = 0;
      while (back < 150 && state.edge !== "first") {
        state = await probe.press("Shift+Tab");
        probe.check(state.activeId === "midi-evil-canvas" || state.activeId === "midi-evil-text-entry", "Shift+Tab left the editor before the first stop");
        back += 1;
      }
      probe.check(plain(state.current).startsWith("About MIDI Evil, button"), `the first stop is ${state.current}`);
    },
  },

  "mac-text-editing": {
    title: "on a Mac's system, a name field takes Command+A, Command+Z, Option+Left and Option+Backspace",
    userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      await probe.pressUntil("Tab", "Project name");
      let state = await probe.press("Meta+a", "Command+A");
      probe.check(state.keys.find((key) => key.key === "a")?.reached, "Command+A did not reach the field");
      state = await probe.type("One two");
      probe.check(plain(state.current).endsWith("text field, One two"), `select all and type left ${state.current}`);
      state = await probe.press("Alt+ArrowLeft", "Option+Left");
      state = await probe.type("X");
      probe.check(plain(state.current).endsWith("text field, One Xtwo"), `Option+Left did not move by a word: ${state.current}`);
      state = await probe.press("Alt+Backspace", "Option+Backspace");
      probe.check(plain(state.current).endsWith("text field, One two"), `Option+Backspace did not delete a word: ${state.current}`);
      state = await probe.press("Meta+z", "Command+Z");
      probe.check(!plain(state.current).endsWith("text field, One two"), `Command+Z undid nothing: ${state.current}`);
    },
  },

  "pointer-guard": {
    title: "a drag on the canvas makes the page answer unsaved in the same task; a click is a frame of eframe's own at once",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      probe.check(!(await probe.pageAnswersUnsaved()), "a new project's page answers unsaved");
      const dragged = await probe.pageAnswersUnsaved(
        "const canvas = document.getElementById('midi-evil-canvas'); canvas.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, buttons: 1, clientX: 300, clientY: 400 })); canvas.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, buttons: 1, clientX: 340, clientY: 400 }));",
      );
      probe.check(dragged, "a drag and beforeunload in one task: the page did not answer unsaved");
      await probe.page.evaluate(() =>
        document.getElementById("midi-evil-canvas").dispatchEvent(new PointerEvent("pointerup", { bubbles: true, clientX: 340, clientY: 400 })),
      );
    },
  },

  "field-ended": {
    title: "after Enter or Escape in the tempo box, a typed digit changes nothing and Tab moves on",
    async run(probe) {
      for (const ending of ["Enter", "Escape"]) {
        await probe.open(probe.url);
        await newProjectFromTheMenu(probe);
        await probe.pressUntil("Tab", "Tempo in beats per minute");
        await probe.press("Control+a");
        await probe.type("130");
        let state = await probe.press(ending);
        const kept = plain(state.current);
        probe.check(state.activeId === "midi-evil-canvas", `after ${ending} the page's focus is on ${state.activeId}, an open edit`);
        state = await probe.type("5");
        probe.check(plain(state.current) === kept, `after ${ending} a digit changed the field: ${state.current}`);
        probe.expectWrites(state, [], `a digit after ${ending}`);
        state = await probe.press("Tab", `Tab after ${ending}`);
        probe.check(plain(state.current).startsWith("Meter"), `Tab after ${ending} reached ${state.current}`);
        await probe.expectFocus("Meter, beats per bar over 4", "Meter, beats per bar over 4");
      }
    },
  },

  "markup-as-text": {
    title: "a track named with <, & and a direction override reaches the page as text",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      await probe.pressUntil("Tab", "Track name");
      await probe.press("Control+a");
      const name = "<b>&amp;‮dir";
      await probe.type(name);
      const state = await probe.press("Enter");
      probe.check(plain(state.current).includes(name), `the current element does not hold the name as typed: ${JSON.stringify(state.current)}`);
      probe.check(!state.currentHasElements && !state.liveHasElements, "the name became markup");
      probe.expectWrites(state, [`Track name, ${name}`], "the name committed");
    },
  },
};
