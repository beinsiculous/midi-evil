// The rows about the page itself and about what the browser build leaves out, and the one row the
// keys-and-bridge group was owed. All of them run on the visitor's bundle: what a visitor gets is
// what they judge. web-app-probe.mjs runs them.

import { enterTheEditor, newProjectFromTheMenu, plain, walkEveryStop } from "./web-app-rows.mjs";

// What the browser build leaves out, as the desktop names each on its control, and the stops only
// a pointer could drag. A current element beginning with any of these is a removed thing reached.
const LEFT_OUT = [
  "MIXER", "SYNTH", "KEYBOARD", "PADS", "Add a Maxwell drum-kit track", "Open Evil Synth",
  "Open Pads", "Samples", "Modulation", "Mixer & FX", "Automation", "MIDI input port",
  "Refresh MIDI inputs", "Import MIDI…", "Recover rhythm from WAV…", "Relink MIDI…",
  "Export MIDI…", "Render WAV…", "Render section WAV…", "Track stems…", "Arrangement parts…",
  "Export", "Standard MIDI…", "Stereo WAV…", "Section WAV…", "Load files…",
];
const DRAGGED_ONLY = ["title bar", "Side panel width"];

// What the page's desktop-only list must name: one line for each thing the browser leaves out.
const DESKTOP_ONLY = [
  "The Mixer", "The Synth view", "The Keyboard view", "The Pads view", "automation lanes",
  "MIDI input from a device", "Importing a MIDI file", "Recovering a rhythm from a WAV file",
  "Relinking the MIDI file", "Exporting MIDI", "Rendering WAV files, track stems and arrangement parts",
  "The export row", "Loading sound files", "Moving a window by its title bar", "Resizing the side panel",
];
// Every chord of the browser's table. None is refused: Space and B play in the browser too.
const BROWSER_CHORDS = [
  "Control+S: ", "Control+Z: ", "Control+Shift+Z: ", "Control+Y: ", "Space: ", "Alt+Page Down: ", "Alt+Page Up: ",
  "Left: ", "Right: ", "Up: ", "Down: ", "Home: ", "End: ", "Control+Left: ", "Control+Right: ",
  "Shift+Up: ", "Shift+Down: ", "Shift+Left: ", "Shift+Right: ", "Enter: ", "Delete: ", "Escape: ", "B: ", "I: ",
];
const REFUSAL_SUFFIX = "not in the browser build yet";
const KEPT_BY_A_BROWSER = ["F6", "Alt+Left", "Alt+Right", "Control+N", "Control+O"];

// A stop's name is the current element's text up to its first comma: a window's title bar is
// "Sound library, title bar, …", and the Keyboard notes' own text, which names the title bar as
// desktop-only, is not one.
function leftOutIn(current) {
  const text = plain(current);
  if (/^[^,]+, title bar,/.test(text)) return "title bar";
  return [...LEFT_OUT, ...DRAGGED_ONLY].find((name) => text === name || text.startsWith(`${name},`));
}

// Every stop of the editor as it now is, failing on any removed thing and recording each stop's
// text.
function walk(probe, where, seen) {
  return walkEveryStop(probe, where, (state) => {
    seen.push(plain(state.current));
    const found = leftOutIn(state.current);
    if (found) probe.fail(`the walk reached ${found}, which the browser leaves out (${where})`);
  });
}

export const PAGE_ROWS = {
  "click-from-outside": {
    group: "keys-and-bridge",
    title: "a click on a control from outside the editor, pressed and released with no frame between, is announced",
    async run(probe) {
      await probe.page.evaluate(() => document.getElementById("midi-evil-heading").focus());
      const box = await probe.page.locator("#midi-evil-canvas").boundingBox();
      // The toolbar's first control, the studio's own mark, which opens the About window.
      await probe.page.evaluate(
        ([x, y]) => {
          const canvas = document.getElementById("midi-evil-canvas");
          const at = { bubbles: true, cancelable: true, clientX: x, clientY: y, pointerId: 1, pointerType: "mouse", isPrimary: true };
          canvas.dispatchEvent(new PointerEvent("pointerdown", { ...at, button: 0, buttons: 1 }));
          canvas.dispatchEvent(new PointerEvent("pointerup", { ...at, button: 0, buttons: 0 }));
        },
        [box.x + 32, box.y + 28],
      );
      await probe.settle(200);
      const state = await probe.read();
      probe.describe("a click on the About button", state);
      probe.expectWrites(state, ["The About window is open; Tab reaches it."], "the click");
    },
  },

  "absence-walk": {
    group: "absence",
    deadlineSeconds: 150,
    title: "a walk by Tab through every stop of the Song and Notes views, each menu and each window never reaches a removed thing",
    async run(probe) {
      const seen = [];
      await enterTheEditor(probe);
      await walk(probe, "the Song view, with the About window", seen);
      // Project › New closes the About window, which keeps the region chord to itself.
      await probe.pressUntil("Tab", "Project, button");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "New, button");
      await probe.press("Enter");
      await probe.pressUntil("Alt+PageDown", "Song,");
      await probe.press("Enter");
      await probe.press("Enter", "Enter, a note");
      await walk(probe, "the Notes view, a note under the cursor", seen);
      for (const menu of ["Project", "+ Track", "Track actions"]) {
        await probe.pressUntil("Tab", `${menu}, button`, 150);
        await probe.press("Enter");
        await walk(probe, `the ${menu} menu`, seen);
        await probe.press("Escape");
      }
      await probe.pressUntil("Tab", "Project, button", 150);
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Keyboard notes…");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Sound: ", 150);
      await probe.press("Enter");
      await walk(probe, "the Keyboard notes and the Sound library windows", seen);
      for (const present of ["Add a synth track", "Track actions", "Undo", "Project", "+ Track", "Next sound", "Keyboard notes…", "Track name", "Piano roll", "Start composing"]) {
        probe.check(seen.some((text) => text.startsWith(present)), `the walk never reached ${present}, which must be there`);
      }
    },
  },

  "absence-no-way-in": {
    group: "absence",
    title: "no menu path, chord or button changes the view to a hidden one: adding and selecting a drum track leaves the view where it was",
    async run(probe) {
      await newProjectFromTheMenu(probe);
      for (const view of ["Song,", "Piano roll"]) {
        await probe.pressUntil("Tab", "+ Track, button", 150);
        await probe.press("Enter");
        await probe.pressUntil("Tab", "Drum track");
        const added = await probe.press("Enter");
        probe.check(added.writes.some((write) => plain(write).includes("a drum track with the Maxwell kit sound")), `the drum track was not added: ${JSON.stringify(added.writes)}`);
        const editor = await probe.pressUntil("Alt+PageDown", view === "Song," ? "Song," : "Piano roll");
        probe.check(plain(editor.current).startsWith(view), `after adding a drum track the editor is ${editor.current}`);
        await probe.pressUntil("Alt+PageUp", "0");
        await probe.press("Enter", "Enter on the selected track");
        const after = await probe.pressUntil("Alt+PageDown", view);
        probe.check(plain(after.current).startsWith(view), `selecting the drum track changed the editor to ${after.current}`);
        if (view === "Song,") await probe.press("Enter", "Enter, into the roll");
      }
    },
  },

  "no-window-moves": {
    group: "absence",
    title: "no window moves and the side panel does not resize: no stop is named for either",
    async run(probe) {
      const seen = [];
      await enterTheEditor(probe);
      await probe.pressUntil("Tab", "Project, button");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Keyboard notes…");
      await probe.press("Enter");
      await probe.pressUntil("Tab", "Sound: ", 150);
      await probe.press("Enter");
      await walk(probe, "three windows open", seen);
      probe.check(seen.length > 20, `the walk passed only ${seen.length} stops`);
    },
  },

  "notes-before-the-wasm": {
    group: "page",
    title: "the page's notes and desktop-only list are in the page before the wasm has started, and come before the canvas",
    opens: false,
    async run(probe) {
      // The wasm is held for the whole row, which never needs it, and the route is let go
      // however the row ends; the page closes with its request still waiting.
      const wasm = "**/evil_web_bg.wasm";
      let page;
      try {
        await probe.open(probe.settings.url, {
          waitForEditor: false,
          before: (fresh) => fresh.route(wasm, () => new Promise(() => {})),
        });
        page = await probe.page.evaluate(() => {
          const notes = document.getElementById("midi-evil-notes");
          const canvas = document.getElementById("midi-evil-canvas");
          const toggle = document.getElementById("midi-evil-notes-toggle");
          const heading = document.getElementById("midi-evil-heading");
          const before = (first, second) => Boolean(first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING);
          return {
            text: notes.textContent.replace(/\s+/g, " "),
            hidden: notes.hidden,
            current: document.getElementById("midi-evil-current").textContent,
            order: before(heading, toggle) && before(toggle, notes) && before(notes, document.getElementById("midi-evil-enter-first")) && before(notes, canvas),
            expanded: toggle.getAttribute("aria-expanded"),
          };
        });
      } finally {
        await probe.page?.unrouteAll({ behavior: "ignoreErrors" });
      }
      probe.check(page.current === "Loading the editor", `the application had started: ${page.current}`);
      probe.check(page.order, "the heading, the notes' button and the notes do not come before the canvas");
      probe.check(page.hidden && page.expanded === "false", "the notes are not closed when the page loads");
      for (const chord of BROWSER_CHORDS) probe.check(page.text.includes(chord), `the notes lack ${JSON.stringify(chord)}`);
      probe.check(!page.text.includes(REFUSAL_SUFFIX), `the notes still say a chord is ${REFUSAL_SUFFIX}`);
      for (const kept of KEPT_BY_A_BROWSER) probe.check(!page.text.includes(kept), `the notes name ${kept}, which a browser keeps`);
      for (const thing of DESKTOP_ONLY) probe.check(page.text.includes(thing), `the desktop-only list lacks ${JSON.stringify(thing)}`);
      probe.check(page.text.includes("Chromium and Firefox"), "the notes do not say which browsers the page was tested in");
      probe.check(page.text.includes("a Mac's own shortcuts are not designed yet"), "the notes do not say a Mac's shortcuts are not designed");
    },
  },

  "page-order": {
    group: "page",
    title: "the page's order: the heading, the notes' button, the notes when open, then the editor; the notes scroll within themselves and the document never does",
    async run(probe) {
      let state = await probe.press("Tab", "Tab from the page");
      probe.check(state.activeId === "midi-evil-notes-toggle", `the first stop is ${state.activeId}, not the notes' button`);
      const button = await probe.page.locator(":focus").ariaSnapshot();
      probe.check(/button "Keyboard notes, and what is desktop-only"/.test(button), `the notes' button is ${button}`);
      state = await probe.press("Enter", "Enter on the notes' button");
      const opened = await probe.page.evaluate(() => ({
        hidden: document.getElementById("midi-evil-notes").hidden,
        expanded: document.getElementById("midi-evil-notes-toggle").getAttribute("aria-expanded"),
      }));
      probe.check(!opened.hidden && opened.expanded === "true", `the notes did not open: ${JSON.stringify(opened)}`);
      state = await probe.press("Tab", "Tab to the notes");
      probe.check(state.activeId === "midi-evil-notes", `Tab from the button reached ${state.activeId}, not the notes`);
      const region = await probe.page.locator(":focus").ariaSnapshot();
      probe.check(/region "Keyboard notes, and what is desktop-only"/.test(region), `the notes are ${region.slice(0, 80)}`);
      const scrolled = await probe.page.evaluate(() => document.getElementById("midi-evil-notes").scrollTop);
      for (let count = 0; count < 5; count += 1) await probe.press("ArrowDown", "Down in the notes");
      state = await probe.read();
      const after = await probe.page.evaluate(() => document.getElementById("midi-evil-notes").scrollTop);
      probe.check(after > scrolled, `Down did not scroll the notes: ${scrolled} then ${after}`);
      probe.check(state.scroll.every((offset) => offset === 0), `the document scrolled to ${state.scroll}`);
      state = await probe.press("Tab", "Tab into the editor");
      probe.check(state.activeId === "midi-evil-canvas" && state.edge === "first", `Tab from the notes reached ${state.activeId}, edge ${state.edge}`);
      await probe.expectFocus("About MIDI Evil, button");
      state = await probe.press("Shift+Tab", "Shift+Tab at the first stop");
      if (probe.browserName === "chromium") {
        probe.check(state.activeId === "midi-evil-notes", `Shift+Tab from the editor reached ${state.activeId}, not the notes`);
      } else {
        probe.log(`    Firefox left the focus on ${state.activeTag}#${state.activeId}: the real browser's next stop is the sitting's`);
      }
    },
  },

  "layout": {
    group: "page",
    title: "the layout at 1366 by 768, at the page's text size and at twice it, and at 683 by 384, a 1366 by 768 window at twice the zoom; the notes closed and open, the current element two lines long: the current element and Leave the editor stay in the window, Leave the editor is reached by Tab, and the canvas keeps 240 pixels wherever the window has them",
    opens: false,
    async run(probe) {
      const floors = {};
      for (const [width, height, textSize] of [[1366, 768, "100%"], [1366, 768, "200%"], [683, 384, "100%"]]) {
        for (const notesOpen of [false, true]) {
          const where = `${width} by ${height}, text at ${textSize}, notes ${notesOpen ? "open" : "closed"}`;
          await probe.open(probe.settings.url, { viewport: { width, height } });
          await probe.page.evaluate(
            ([size, open]) => {
              document.documentElement.style.fontSize = size;
              if (open) document.getElementById("midi-evil-notes-toggle").click();
            },
            [textSize, notesOpen],
          );
          // The arranger, whose description fills the current element's two lines.
          await newProjectFromTheMenu(probe);
          await probe.pressUntil("Alt+PageDown", "Song,");
          let state = probe.last;
          for (let count = 0; count < 160 && state.activeId !== "midi-evil-leave"; count += 1) {
            state = await probe.press("Tab", "Tab", 20);
          }
          const layout = await probe.page.evaluate(() => {
            const rect = (id) => document.getElementById(id).getBoundingClientRect();
            const current = document.getElementById("midi-evil-current");
            return {
              canvas: rect("midi-evil-canvas").height,
              currentLines: Math.round(rect("midi-evil-current").height / parseFloat(getComputedStyle(current).lineHeight)),
              currentBottom: rect("midi-evil-current").bottom,
              leaveTop: rect("midi-evil-leave").top,
              leaveBottom: rect("midi-evil-leave").bottom,
              height: window.innerHeight,
              scroll: [window.scrollX, window.scrollY, document.scrollingElement.scrollTop],
              notes: rect("midi-evil-notes").height,
              floor: getComputedStyle(document.getElementById("midi-evil-canvas")).minHeight,
            };
          });
          probe.log(`    ${where}: ${JSON.stringify(layout)}`);
          probe.check(state.activeId === "midi-evil-leave", `${where}: Tab from the arranger never reached Leave the editor`);
          probe.check(layout.currentLines >= 2, `${where}: the current element holds ${layout.currentLines} lines, not two`);
          probe.check(layout.currentBottom <= layout.height, `${where}: the current element ends at ${layout.currentBottom} of a ${layout.height}-pixel window`);
          probe.check(layout.leaveTop >= 0 && layout.leaveBottom <= layout.height, `${where}: Leave the editor spans ${layout.leaveTop} to ${layout.leaveBottom} of a ${layout.height}-pixel window`);
          probe.check(layout.scroll.every((offset) => offset === 0), `${where}: the document scrolled to ${layout.scroll}`);
          probe.check(!notesOpen || layout.notes > 0, `${where}: the open notes have no height`);
          probe.check(layout.canvas > 0, `${where}: the canvas has no height`);
          // A window of 768 pixels has room for the floor; a short one gives the floor up
          // before it loses a control.
          if (height >= 768) probe.check(layout.canvas >= 240, `${where}: the canvas is ${layout.canvas} pixels high, under its 240`);
          // The floor is in pixels, not in text: one that grew with the text would take the
          // room the notes and Leave the editor need at twice the size.
          floors[textSize] ??= layout.floor;
          probe.check(layout.floor === floors["100%"] || width !== 1366, `${where}: the canvas's least height is ${layout.floor} at this text size and ${floors["100%"]} at the page's`);
        }
      }
    },
  },

  "visitor-ignores-the-gate": {
    group: "page",
    title: "the visitor's bundle ignores ?fixture= and ?shows=, and only the gate's wasm carries the gate's marker",
    opens: false,
    async run(probe) {
      await probe.open(`${probe.settings.url}?fixture=twins&shows=desktop`);
      const canvasMarked = await probe.page.evaluate(() => document.getElementById("midi-evil-canvas").dataset.gateBundle ?? "");
      probe.check(canvasMarked === "", `the visitor's canvas carries ${canvasMarked}`);
      await enterTheEditor(probe);
      const seen = [];
      for (let count = 0; count < 12; count += 1) {
        const state = await probe.press("Tab");
        seen.push(plain(state.current));
        const found = leftOutIn(state.current);
        if (found) probe.fail(`?shows=desktop drew ${found} in the visitor's bundle`);
      }
      const named = await probe.pressUntil("Tab", "Project name");
      probe.check(plain(named.current).endsWith("text field, Untitled"), `?fixture=twins opened a project in the visitor's bundle: ${named.current}`);
      const marker = probe.settings.marker;
      probe.check(Boolean(marker), "the probe was given no marker");
      const carried = await probe.page.evaluate(
        async ([visitor, gate, text]) => {
          const holds = async (url) => {
            const bytes = new Uint8Array(await (await fetch(url)).arrayBuffer());
            const wanted = new TextEncoder().encode(text);
            outer: for (let index = 0; index + wanted.length <= bytes.length; index += 1) {
              for (let offset = 0; offset < wanted.length; offset += 1) {
                if (bytes[index + offset] !== wanted[offset]) continue outer;
              }
              return true;
            }
            return false;
          };
          return { visitor: await holds(visitor), gate: await holds(gate) };
        },
        [new URL("evil_web_bg.wasm", probe.settings.url).href, new URL("evil_web_bg.wasm", probe.settings.gateUrl).href, marker],
      );
      probe.check(carried.gate, "the gate's wasm does not carry the marker");
      probe.check(!carried.visitor, "the visitor's wasm carries the gate's marker");
    },
  },
};
