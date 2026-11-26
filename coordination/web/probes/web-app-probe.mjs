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
//   node web-app-probe.mjs --url http://127.0.0.1:PORT/bundle/index.html --browser chromium \
//       [--rows nine-steps,held-key]
//
// The rows are in web-app-rows.mjs. Each runs in a fresh page. Keys are Playwright's, delivered to
// the page past the browser's own shortcut handling, so what a browser itself does with a chord —
// open a dialog, go back, print — is never shown here: the rows that hand a chord to the browser
// show that the application never received it, did nothing and wrote nothing, and that the
// chord was not cancelled, and they say the rest is the sitting's.
//
// Exit status: 0, every row held; 1, a row failed, each failure on a line beginning "FAILED:";
// 2, the tooling failed — Playwright would not load, the browser would not launch — and nothing
// was judged; a row that outlives its deadline fails, so a hang never stands in for a result.

import { createRequire } from "node:module";

import { ROWS, plain } from "./web-app-rows.mjs";

// A browser's tree collapses runs of white space in a description; so does every comparison.
function collapsed(text) {
  return plain(text).replace(/\s+/g, " ");
}

const LOAD_DEADLINE_MILLISECONDS = 60000;
const ROW_DEADLINE_MILLISECONDS = 120000;
const VIEWPORT = { width: 1440, height: 900 };

class ToolingFailure extends Error {}

function parseArguments(argv) {
  const settings = { url: null, browser: null, rows: null };
  for (let index = 0; index < argv.length; index += 2) {
    const [flag, value] = [argv[index], argv[index + 1]];
    if (value === undefined) throw new ToolingFailure(`${flag} needs a value`);
    if (flag === "--url") settings.url = value;
    else if (flag === "--browser") settings.browser = value;
    else if (flag === "--rows") settings.rows = value.split(",");
    else throw new ToolingFailure(`unknown argument ${flag}`);
  }
  if (!settings.url || !["chromium", "firefox"].includes(settings.browser)) {
    throw new ToolingFailure("usage: --url URL --browser chromium|firefox [--rows A,B]");
  }
  for (const row of settings.rows ?? []) {
    if (!ROWS[row]) throw new ToolingFailure(`no row named ${row}`);
  }
  return settings;
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
  // navigation no person started.)
  async open(url) {
    this.url = url;
    // Closing a context runs no leave-page handler.
    if (this.page) await this.page.context().close();
    const context = await this.browser.newContext({
      viewport: VIEWPORT,
      ...(this.userAgent ? { userAgent: this.userAgent } : {}),
    });
    this.page = await context.newPage();
    await this.page.addInitScript(instrumentPage);
    this.page.on("pageerror", (error) => this.errors.push(`the page raised: ${error.message}`));
    this.page.on("console", (message) => {
      if (message.type() === "error") this.errors.push(`the console logged an error: ${message.text().slice(0, 300)}`);
    });
    this.cdp = null;
    await this.page.goto(url, { waitUntil: "load", timeout: LOAD_DEADLINE_MILLISECONDS });
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

  async press(chord, label = chord) {
    await this.page.keyboard.press(chord);
    await this.settle();
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

  // Presses `chord` until the current element begins with `text`.
  async pressUntil(chord, text, most = 60) {
    for (let count = 0; count < most; count += 1) {
      const state = await this.press(chord);
      if (plain(state.current).startsWith(text)) return state;
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
    let timer;
    const deadline = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`the row outlived its ${ROW_DEADLINE_MILLISECONDS / 1000} s deadline`)), ROW_DEADLINE_MILLISECONDS);
    });
    const row = (async () => {
      await probe.open(settings.url);
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
  const names = settings.rows ?? Object.keys(ROWS);
  let failed = 0;
  for (const name of names) {
    console.log(`\n## ${settings.browser}: ${name} — ${ROWS[name].title}`);
    if (settings.browser === "firefox") {
      console.log("   (Firefox: Playwright's computation of the focused element; the real tree is the sitting's)");
    }
    if (ROWS[name].sitting) console.log(`   (the sitting's: ${ROWS[name].sitting})`);
    if (ROWS[name].userAgent) console.log(`   (as a Mac: ${ROWS[name].userAgent})`);
    const failures = await runRow(engine, settings, name, (line) => console.log(line));
    if (failures.length === 0) console.log(`PASSED: ${name}`);
    else failed += 1;
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
