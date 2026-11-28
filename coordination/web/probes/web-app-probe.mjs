// Drives the browser build of the whole application — the real bundle, from its own default
// project — through the rows of the web gate, in one headless browser, and judges each row. A row
// passes only when everything it names held, and every check of where the page's focus is reads
// the element that has it: it must be named, and described by what the row expects. In Chromium
// that is the browser's real accessibility tree, read over the DevTools protocol and asserted —
// role, name, description; Firefox has no such protocol, so there the check is Playwright's own
// computation of the role and name with the description read from aria-describedby, and every
// Firefox row says so: its real tree is the sitting's.
//
//   PLAYWRIGHT_FROM=…/insiculous_web/package.json \
//   node web-app-probe.mjs --url http://127.0.0.1:PORT/bundle/index.html \
//       --gate-url http://127.0.0.1:PORT/gate/index.html --browser chromium \
//       [--group matrix] [--rows nine-steps,held-key] [--shows-desktop] [--marker TEXT]
//
// The rows are in web-app-rows.mjs, the matrix's in web-app-matrix-rows.mjs and the sound's in
// web-app-playing-rows.mjs, every one with the audio tap of web-app-audio-tap.mjs installed
// before the page's own scripts, so whatever a row plays is heard by the gate and by nobody
// else. Each runs in a
// fresh page, on the bundle it names: the visitor's (--url) or the gate's (--gate-url), which
// opens the row's fixture by `?fixture=`. --shows-desktop adds `?shows=desktop` to every page
// of the gate's bundle, as the absence walk's negative control asks; --marker is the text only
// the gate's bundle compiles in. Keys are Playwright's, delivered to
// the page past the browser's own shortcut handling, so what a browser itself does with a chord —
// open a dialog, go back, print — is never shown here: the rows that hand a chord to the browser
// show that the application never received it, did nothing and wrote nothing, and that the
// chord was not cancelled, and they say the rest is the sitting's.
//
// Exit status: 0, every row held; 1, a row failed, each failure on a line beginning "FAILED:";
// 2, the tooling failed — Playwright would not load, the browser would not launch, the tap could
// not hear what it must, or the browser has nowhere to play to, which refuses the playing group
// rather than failing it — and nothing was judged; a row that outlives its deadline fails, so a
// hang never stands in for a result.

import { createRequire } from "node:module";

import { installAudioTap } from "./web-app-audio-tap.mjs";
import { MATRIX_ROWS } from "./web-app-matrix-rows.mjs";
import { PAGE_ROWS } from "./web-app-page-rows.mjs";
import { PLAYING_ROWS, preflightTheTap } from "./web-app-playing-rows.mjs";
import { ROWS as EARLIER_ROWS, plain } from "./web-app-rows.mjs";

// Every row, each in the group it runs in. The groups run apart, each under its own deadline.
const ROWS = Object.fromEntries(
  [
    ...Object.entries(PAGE_ROWS),
    ...Object.entries(EARLIER_ROWS).map(([name, row]) => [name, { group: "keys-and-bridge", ...row }]),
    ...Object.entries(MATRIX_ROWS).map(([name, row]) => [name, { group: "matrix", bundle: "gate", ...row }]),
    ...Object.entries(PLAYING_ROWS).map(([name, row]) => [name, { group: "playing", bundle: "gate", ...row }]),
  ],
);

// A browser's tree collapses runs of white space in a description; so does every comparison.
function collapsed(text) {
  return plain(text).replace(/\s+/g, " ");
}

const LOAD_DEADLINE_MILLISECONDS = 60000;
const ROW_DEADLINE_MILLISECONDS = 120000;
const VIEWPORT = { width: 1440, height: 900 };

class ToolingFailure extends Error {}

function parseArguments(argv) {
  const settings = { url: null, gateUrl: null, browser: null, rows: null, group: null, showsDesktop: false, marker: null, groupSeconds: null };
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    if (flag === "--shows-desktop") {
      settings.showsDesktop = true;
      index -= 1;
      continue;
    }
    const value = argv[index + 1];
    if (value === undefined) throw new ToolingFailure(`${flag} needs a value`);
    if (flag === "--url") settings.url = value;
    else if (flag === "--gate-url") settings.gateUrl = value;
    else if (flag === "--browser") settings.browser = value;
    else if (flag === "--rows") settings.rows = value.split(",");
    else if (flag === "--group") settings.group = value;
    else if (flag === "--marker") settings.marker = value;
    else if (flag === "--group-seconds") settings.groupSeconds = Number(value);
    else throw new ToolingFailure(`unknown argument ${flag}`);
  }
  if (!settings.url || !["chromium", "firefox"].includes(settings.browser)) {
    throw new ToolingFailure("usage: --url URL [--gate-url URL] --browser chromium|firefox [--group G] [--rows A,B] [--shows-desktop] [--marker TEXT]");
  }
  for (const row of settings.rows ?? []) {
    if (!ROWS[row]) throw new ToolingFailure(`no row named ${row}`);
  }
  if (settings.group && !Object.values(ROWS).some((row) => row.group === settings.group)) {
    throw new ToolingFailure(`no group named ${settings.group}`);
  }
  // A row that may take most of its group's time would leave the group's own deadline nothing to
  // stand for. Only the rows this run will run are held to the deadline it runs under.
  for (const name of selectedRows(settings)) {
    const seconds = ROWS[name].deadlineSeconds ?? ROW_DEADLINE_MILLISECONDS / 1000;
    if (settings.groupSeconds && seconds > settings.groupSeconds / 2) {
      throw new ToolingFailure(`the row ${name} may take ${seconds} s, more than half its group's ${settings.groupSeconds} s`);
    }
  }
  return settings;
}

// The rows named, or else every row of the group named, or else every row.
function selectedRows(settings) {
  return settings.rows ?? Object.keys(ROWS).filter((name) => !settings.group || ROWS[name].group === settings.group);
}

// The page a row starts on: its bundle's, with its fixture and, for the negative control, the
// desktop's set of controls.
function rowUrl(settings, row) {
  if (row.bundle !== "gate" && !settings.showsDesktop) return settings.url;
  if (!settings.gateUrl) throw new ToolingFailure("a row on the gate's bundle needs --gate-url");
  const query = [row.fixture ? `fixture=${row.fixture}` : null, settings.showsDesktop ? "shows=desktop" : null].filter(Boolean);
  return `${settings.gateUrl}${query.length ? `?${query.join("&")}` : ""}`;
}

function loadPlaywright() {
  const packageJson = process.env.PLAYWRIGHT_FROM;
  if (!packageJson) throw new ToolingFailure("PLAYWRIGHT_FROM names no package.json to load Playwright beside");
  try {
    return createRequire(packageJson)("playwright");
  } catch (error) {
    throw new ToolingFailure(`Playwright could not be loaded beside ${packageJson}: ${error.message}`);
  }
}

// Installed before the page's own scripts: every write the live region receives, as mutation
// records, so a write replaced within one task is still counted; and, for every keydown, whether
// it reached the canvas or the text entry — eframe's listeners are there — and whether its default
// was cancelled once the whole dispatch had run.
function instrumentPage() {
  window.midiEvilGate = { writes: [], keys: [] };
  const gate = window.midiEvilGate;
  window.addEventListener(
    "keydown",
    (event) => {
      const record = { key: event.key, reached: false, cancelled: false };
      gate.keys.push(record);
      // Watched from the first key on; a listener added before the event reaches its target
      // still hears it.
      for (const id of ["midi-evil-canvas", "midi-evil-text-entry"]) {
        const element = document.getElementById(id);
        if (element && !element.midiEvilGateWatched) {
          element.midiEvilGateWatched = true;
          element.addEventListener("keydown", (seen) => {
            const pending = gate.keys.findLast((key) => key.key === seen.key);
            if (pending) pending.reached = true;
          });
        }
      }
      setTimeout(() => {
        record.cancelled = event.defaultPrevented;
      }, 0);
    },
    true,
  );
  document.addEventListener("DOMContentLoaded", () => {
    const live = document.getElementById("midi-evil-live");
    new MutationObserver((records) => {
      for (let index = 0; index < records.length; index += 1) gate.writes.push(live.textContent);
    }).observe(live, { childList: true, characterData: true, subtree: true });
  });
}

// What the gate reads of the page, one call: where the page's focus is, the current element, the
// live region's writes since the last read, the canvas's data attributes and the scroll position.
function readPage() {
  const gate = window.midiEvilGate;
  const active = document.activeElement;
  const current = document.getElementById("midi-evil-current");
  const live = document.getElementById("midi-evil-live");
  const canvas = document.getElementById("midi-evil-canvas");
  return {
    activeId: active?.id ?? "",
    activeTag: active?.tagName.toLowerCase() ?? "",
    current: current.textContent,
    currentHasElements: current.children.length > 0,
    liveHasElements: live.children.length > 0,
    writes: gate.writes.splice(0),
    keys: gate.keys.splice(0),
    edge: canvas.dataset.focusEdge ?? "",
    unsaved: canvas.dataset.unsaved === "true",
    scroll: [window.scrollX, window.scrollY, document.scrollingElement.scrollTop],
  };
}

class Probe {
  constructor(browser, browserName, userAgent, log) {
    this.browser = browser;
    this.browserName = browserName;
    this.userAgent = userAgent;
    this.log = log;
    this.failures = [];
    this.errors = [];
  }

  // A fresh page in a context of its own: a page left with unsaved work asks before it is left,
  // and that prompt is never part of a row. (Playwright's handling of it differs: with no dialog
  // listener, Chromium's prompt is accepted for the script; headless Firefox raises none for a
  // navigation no person started.) `before` runs on the new page before it loads, and with
  // `waitForEditor` false the row reads the page before the application has started. An
  // `untouched` page is read through its console alone, from the moment it opens: the tap and the
  // bridge report there, and nothing the probe does to read it counts as a gesture.
  async open(url, { viewport = VIEWPORT, before = null, waitForEditor = true, untouched = false } = {}) {
    this.url = url;
    // Closing a context runs no leave-page handler.
    if (this.page) await this.page.context().close();
    const context = await this.browser.newContext({
      viewport,
      ...(this.userAgent ? { userAgent: this.userAgent } : {}),
    });
    this.page = await context.newPage();
    await this.page.addInitScript(installAudioTap, { reportThroughConsole: untouched });
    await this.page.addInitScript(instrumentPage);
    this.console = [];
    this.untouchedCurrent = "";
    this.page.on("pageerror", (error) => this.errors.push(`the page raised: ${error.message}`));
    this.page.on("console", (message) => {
      if (message.type() === "error") this.errors.push(`the console logged an error: ${message.text().slice(0, 300)}`);
      const line = { at: Date.now(), type: message.type(), text: message.text() };
      this.console.push(line);
      if (line.text.startsWith("midi-evil-current: ")) this.untouchedCurrent = plain(line.text.slice(19));
    });
    this.cdp = null;
    if (before) await before(this.page);
    await this.page.goto(url, { waitUntil: "load", timeout: LOAD_DEADLINE_MILLISECONDS });
    if (untouched) {
      const started = await this.waitForConsole(() => this.untouchedCurrent && this.untouchedCurrent !== "Loading the editor", LOAD_DEADLINE_MILLISECONDS);
      if (!started) throw new Error("the editor never said it had started, through the console");
      return;
    }
    if (!waitForEditor) return;
    await this.page.waitForFunction(
      () => document.getElementById("midi-evil-current")?.textContent !== "Loading the editor",
      null,
      { timeout: LOAD_DEADLINE_MILLISECONDS },
    );
    if (this.browserName === "chromium") {
      this.cdp = await this.page.context().newCDPSession(this.page);
      await this.cdp.send("Accessibility.enable");
    }
    await this.settle();
    await this.read();
  }

  // Waits, reading nothing of the page but its console, until `predicate` holds; false if it never
  // did within `milliseconds`.
  async waitForConsole(predicate, milliseconds = 10000) {
    const until = Date.now() + milliseconds;
    while (Date.now() < until) {
      if (predicate()) return true;
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    return predicate();
  }

  // A key on an untouched page, then a wait, read through the console alone.
  async pressUntouched(chord, milliseconds = 150) {
    await this.page.keyboard.press(chord);
    await new Promise((resolve) => setTimeout(resolve, milliseconds));
    this.log(`after ${chord}, read through the console: current: ${this.untouchedCurrent.slice(0, 150)}`);
    return this.untouchedCurrent;
  }

  fail(message) {
    this.failures.push(message);
    this.log(`FAILED: ${message}`);
  }

  check(condition, message) {
    if (!condition) this.fail(message);
    return condition;
  }

  // Lets the frames a key asks for run: eframe answers on the next animation frame, and a name the
  // announcer holds back asks for one more.
  async settle(extraMilliseconds = 80) {
    await this.page.evaluate(
      (wait) =>
        new Promise((resolve) => {
          let frames = 0;
          const next = () => (++frames >= 4 ? setTimeout(resolve, wait) : requestAnimationFrame(next));
          requestAnimationFrame(next);
        }),
      extraMilliseconds,
    );
  }

  async read() {
    const state = await this.page.evaluate(readPage);
    this.last = state;
    return state;
  }

  describe(label, state) {
    const writes = state.writes.map((write) => JSON.stringify(write)).join(", ");
    this.log(
      `after ${label}: focus ${state.activeTag}#${state.activeId}, edge ${state.edge}` +
        `${state.unsaved ? ", unsaved" : ""}\n    current: ${state.current.slice(0, 150)}` +
        `\n    writes: ${writes || "none"}`,
    );
  }

  // `extraMilliseconds` past the frames a key asks for; a walk over every stop waits less.
  async press(chord, label = chord, extraMilliseconds = 80) {
    await this.page.keyboard.press(chord);
    await this.settle(extraMilliseconds);
    const state = await this.read();
    this.describe(label, state);
    return state;
  }

  async type(text) {
    await this.page.keyboard.type(text);
    await this.settle();
    const state = await this.read();
    this.describe(`typing ${JSON.stringify(text)}`, state);
    return state;
  }

  // Presses `chord` until the current element begins with `text`. A Tab that leaves the editor
  // comes round again into it, as the next Tab would in a browser with a window around the page
  // (headless Firefox has none, and leaves the focus where it went): by the sentinel on the side
  // it would enter from, whatever stands between it and the top or the end of the page.
  async pressUntil(chord, text, most = 60) {
    for (let count = 0; count < most; count += 1) {
      let state = await this.press(chord);
      if (plain(state.current).startsWith(text)) return state;
      const left = state.activeId !== "midi-evil-canvas" && state.activeId !== "midi-evil-text-entry";
      if (left && (chord === "Tab" || chord === "Shift+Tab")) {
        const sentinel = chord === "Tab" ? "midi-evil-enter-first" : "midi-evil-enter-last";
        await this.page.evaluate((id) => document.getElementById(id).focus(), sentinel);
        await this.settle();
        state = await this.read();
        this.describe(`${chord}, coming round into the editor`, state);
        if (plain(state.current).startsWith(text)) return state;
      }
    }
    this.fail(`${chord} never reached a control beginning ${JSON.stringify(text)}`);
    return this.last;
  }

  // The element with the page's focus, as the browser's real tree has it (Chromium) or as
  // Playwright computes it (Firefox).
  async focusedNode() {
    if (this.cdp) {
      const { result } = await this.cdp.send("Runtime.evaluate", { expression: "document.activeElement" });
      const { nodes } = await this.cdp.send("Accessibility.getPartialAXTree", {
        objectId: result.objectId,
        fetchRelatives: false,
      });
      const node = nodes.find((candidate) => !candidate.ignored) ?? nodes[0];
      return {
        judge: "Chromium's tree over CDP",
        role: node?.role?.value ?? "",
        name: node?.name?.value ?? "",
        description: node?.description?.value ?? "",
      };
    }
    const snapshot = await this.page.locator(":focus").ariaSnapshot({ timeout: 5000 }).catch(() => "");
    const match = /^- (\w+)(?: "((?:[^"\\]|\\.)*)")?/.exec(snapshot.trim());
    const description = await this.page.evaluate(() => {
      const ids = document.activeElement?.getAttribute("aria-describedby") ?? "";
      return ids
        .split(/\s+/)
        .map((id) => document.getElementById(id)?.textContent ?? "")
        .join(" ")
        .trim();
    });
    return {
      judge: "Playwright's computation",
      role: match?.[1] ?? "",
      name: (match?.[2] ?? "").replaceAll('\\"', '"'),
      description,
    };
  }

  // The element with the page's focus is the canvas, or, with `entry` named, the adopted text
  // entry carrying that name; it is named, and its description holds `described`.
  async expectFocus(described, entry = null) {
    const node = await this.focusedNode();
    const wantedRole = entry ? "textbox" : "application";
    const wantedName = entry ?? "MIDI Evil editor";
    const ok =
      this.check(node.role === wantedRole, `the focused element's role is ${JSON.stringify(node.role)}, not ${wantedRole} (${node.judge})`) &
      this.check(node.name === wantedName, `the focused element is named ${JSON.stringify(node.name)}, not ${JSON.stringify(wantedName)} (${node.judge})`) &
      this.check(
        collapsed(node.description).includes(collapsed(described)),
        `the focused element's description ${JSON.stringify(node.description.slice(0, 120))} does not hold ${JSON.stringify(described)} (${node.judge})`,
      );
    this.log(`    focused: ${node.role} ${JSON.stringify(node.name)} — judged by ${node.judge}`);
    return Boolean(ok);
  }

  expectWrites(state, wanted, label) {
    const writes = state.writes.map(plain);
    if (!this.check(writes.length === wanted.length, `${label}: ${writes.length} writes, not ${wanted.length}: ${JSON.stringify(writes)}`)) return;
    wanted.forEach((text, index) =>
      this.check(writes[index].includes(text), `${label}: write ${JSON.stringify(writes[index])} does not hold ${JSON.stringify(text)}`),
    );
  }

  async pageAnswersUnsaved(firstScript = null) {
    return this.page.evaluate((script) => {
      if (script) new Function(script)();
      const event = new Event("beforeunload", { cancelable: true });
      window.dispatchEvent(event);
      return event.defaultPrevented;
    }, firstScript);
  }
}

async function runRow(engine, settings, name, log) {
  const browser = await engine
    .launch(
      settings.browser === "firefox"
        ? { headless: true, firefoxUserPrefs: { "webgl.force-enabled": true } }
        : { headless: true, args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] },
    )
    .catch((error) => {
      throw new ToolingFailure(`${settings.browser} did not launch: ${error.message.split("\n")[0]}`);
    });
  try {
    const probe = new Probe(browser, settings.browser, ROWS[name].userAgent, log);
    probe.settings = settings;
    let timer;
    // A row that walks every stop several times says so by a deadline of its own.
    const rowDeadline = (ROWS[name].deadlineSeconds ?? ROW_DEADLINE_MILLISECONDS / 1000) * 1000;
    const deadline = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`the row outlived its ${rowDeadline / 1000} s deadline`)), rowDeadline);
    });
    const row = (async () => {
      if (ROWS[name].opens !== false) await probe.open(rowUrl(settings, ROWS[name]), { untouched: Boolean(ROWS[name].untouched) });
      await ROWS[name].run(probe);
    })();
    // A row stopped by its deadline goes on failing as the browser closes under it; that is not
    // a second result.
    row.catch(() => {});
    try {
      await Promise.race([row, deadline]);
    } catch (error) {
      probe.fail(`the row stopped: ${error.message.split("\n")[0]}`);
    } finally {
      clearTimeout(timer);
    }
    for (const error of probe.errors) probe.fail(error);
    return probe.failures;
  } finally {
    await browser.close();
  }
}

async function main() {
  const settings = parseArguments(process.argv.slice(2));
  const playwright = loadPlaywright();
  const engine = settings.browser === "firefox" ? playwright.firefox : playwright.chromium;
  const names = selectedRows(settings);
  // Before any row that plays: the tap hears an oscillator and nothing without one, and the
  // browser has an audio server to play to — without one, Firefox's context never starts, and
  // the rows are refused, not failed.
  if (names.some((name) => ROWS[name].group === "playing")) {
    await preflightTheTap(engine, settings, (line) => console.log(line), ToolingFailure);
  }
  let failed = 0;
  for (const name of names) {
    console.log(`\n## ${settings.browser}: ${name} — ${ROWS[name].title}`);
    if (settings.browser === "firefox") {
      console.log("   (Firefox: Playwright's computation of the focused element; the real tree is the sitting's)");
    }
    if (ROWS[name].sitting) console.log(`   (the sitting's: ${ROWS[name].sitting})`);
    if (ROWS[name].userAgent) console.log(`   (as a Mac: ${ROWS[name].userAgent})`);
    console.log(`   (on the ${ROWS[name].bundle === "gate" ? "gate's" : "visitor's"} bundle${ROWS[name].fixture ? `, from ${ROWS[name].fixture}.evil` : ""})`);
    const started = Date.now();
    const failures = await runRow(engine, settings, name, (line) => console.log(line));
    const seconds = Math.round((Date.now() - started) / 1000);
    if (failures.length === 0) console.log(`PASSED: ${name} (${seconds} s)`);
    else {
      console.log(`FAILED ROW: ${name} (${seconds} s)`);
      failed += 1;
    }
  }
  console.log(`\n${settings.browser}: ${names.length - failed} of ${names.length} rows held`);
  return failed > 0 ? 1 : 0;
}

// The exit status is set, not forced: output still being written when the process ends is the
// line a negative control looks for.
main().then(
  (status) => {
    process.exitCode = status;
  },
  (error) => {
    console.error(
      error instanceof ToolingFailure
        ? `web-app-probe: the tooling failed: ${error.message}`
        : `web-app-probe: ${error.stack}`,
    );
    process.exitCode = 2;
  },
);
