// Drives the W0-web harness in a headless browser with the site's Playwright and prints, after
// every step, what a screen reader could read: where the page's focus is and what that element is
// called and described as, the accessibility snapshot of the canvas's neighbourhood, the text of
// the current-description element and of the live region, and every write the live region
// received during the step. Expectations fail the run, as atspi-probe.py's do. The bundle is
// served by the caller (web-gate.sh); this script only drives.
//
//   node web-probe.mjs --url http://127.0.0.1:PORT/ --browser chromium \
//       --key Tab --key Tab --expect "Piano roll" --key ArrowRight --expect "C 5"
//
// Settings, anywhere on the command line:
//   --url URL  --browser chromium|firefox  --fixture empty|full  --query NAME=VALUE (repeatable:
//   edge=value puts a value box at egui's first stop, agent=eframe leaves eframe's text entry as
//   eframe made it)
//
// Steps run in command-line order. Keys use Playwright's names: Tab, Shift+Tab, ArrowRight,
// Control+ArrowLeft, Shift+ArrowUp, Enter, Delete, Home, End, Escape, F6, Shift+F6.
//   --heading TEXT        a line in the transcript
//   --key CHORD           one key press, then three animation frames to let egui answer
//   --type TEXT           types TEXT into whatever has the page's focus, then three frames
//   --hold KEY N          a held key: N keydowns dispatched from the page, one per animation
//                         frame, the first plain and the rest repeat-flagged; at each next frame
//                         the cursor must have moved one note (back for ArrowLeft, on for any
//                         other key) and the last announcement must be the cursor's note
//   --burst KEY N         N keydowns dispatched in one task, so they arrive in one frame
//   --reload              a fresh page at the same address
//   --remember NAME       keeps the current description under NAME
//   --key-table           the browser-key table (synthetic), printed as Markdown rows
//
// Expectations check what the step before them left; all of one step's are checked together.
//   --expect TEXT             the current description contains TEXT, and so does the accessible
//                             description of the element that has the page's focus, which must
//                             also have a name. That element must be the canvas, unless the same
//                             step names it with --expect-focus: an expectation on what can be
//                             read never passes while focus is somewhere it cannot be read.
//   --expect-focus WHERE      the page's focus is on the element with id WHERE, or, written
//                             role:NAME, on an element whose accessible role is NAME
//   --expect-name TEXT        the accessible name of the element with the page's focus is TEXT
//   --expect-same             the current description did not change during the step
//   --expect-remembered NAME  the current description is the one kept under NAME
//   --expect-announcement TEXT   a live-region write during the step contains TEXT
//   --expect-announcements N  the live region was written exactly N times during the step
//   --expect-egui FOCUS       egui has focused: roll, readout, other or none
//   --expect-place N          the cursor is on the Nth note in walk order (0 is the first)
//   --expect-valid            the project still validates
//   --expect-changed ATTRIBUTE   the canvas's data attribute changed during the step
//
// Every page load first checks the semantics the rest depends on: the canvas's role and name, and
// the live region's aria-live and aria-atomic. Every browser call has a deadline, and the whole
// run has one: a page that stops answering is a failed step, the browser is closed, and the exit
// status is not 0. Exit status 0 means every expectation held and the page raised no error.

import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repository = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const siteRoot =
  process.env.PLAYWRIGHT_FROM ?? resolve(repository, "../insiculous_web/package.json");
const require = createRequire(siteRoot);
const { chromium, firefox } = require("playwright");
const { expect } = require("playwright/test");

const SETTLE_FRAMES = 3;
const ASSERTION_TIMEOUT_MILLISECONDS = 2000;
const STEP_DEADLINE_MILLISECONDS = 15000;
const LOAD_DEADLINE_MILLISECONDS = 30000;
const RUN_DEADLINE_MILLISECONDS = Number(process.env.WEB_PROBE_DEADLINE_SECONDS ?? 240) * 1000;
if (!(RUN_DEADLINE_MILLISECONDS > 0)) {
  console.error("web-probe: WEB_PROBE_DEADLINE_SECONDS must be a positive number");
  process.exit(2);
}
const NO_BREAK_SPACE = /\u00a0/g;
const CANVAS_ID = "midi-evil-canvas";
const CANVAS_NAME = "MIDI Evil editor";

const failures = [];
const remembered = new Map();
let runExpired = false;
let stepExpired = false;

function fail(message) {
  failures.push(message);
  console.log(`  FAILED: ${message}`);
  console.error(`web-probe: ${message}`);
}

class DeadlineExceeded extends Error {}

// Every call into the browser goes through this, so a wedged page ends the step instead of the
// run waiting forever.
function bounded(promise, label, milliseconds = STEP_DEADLINE_MILLISECONDS) {
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(
      () => reject(new DeadlineExceeded(`${label} did not finish within ${milliseconds} ms`)),
      milliseconds,
    );
  });
  return Promise.race([promise, deadline]).finally(() => clearTimeout(timer));
}

function normalise(text) {
  return (text ?? "").replace(NO_BREAK_SPACE, " ").trim();
}

function escapeForRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function parseArguments(argv) {
  const settings = { browser: "chromium", url: null, query: [] };
  // Each action is followed by the expectations that check it; expectations before the first
  // action check the page as it loaded.
  const groups = [{ action: { flag: "--loaded" }, expectations: [] }];
  const takesValue = new Set([
    "--url", "--browser", "--fixture", "--query", "--heading", "--key", "--type", "--remember",
    "--expect", "--expect-focus", "--expect-name", "--expect-remembered", "--expect-announcement",
    "--expect-announcements", "--expect-egui", "--expect-place", "--expect-changed",
  ]);
  const takesTwo = new Set(["--hold", "--burst"]);
  const takesNone = new Set(["--reload", "--expect-same", "--expect-valid", "--key-table"]);
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    let step;
    if (takesTwo.has(flag)) {
      step = { flag, key: argv[index + 1], count: Number(argv[index + 2]) };
      if (!step.key || !Number.isInteger(step.count)) throw new Error(`${flag} needs KEY N`);
      index += 2;
    } else if (takesValue.has(flag)) {
      const value = argv[index + 1];
      if (value === undefined) throw new Error(`${flag} needs a value`);
      index += 1;
      if (flag === "--url") settings.url = value;
      else if (flag === "--browser") settings.browser = value;
      else if (flag === "--fixture") settings.query.push(["fixture", value]);
      else if (flag === "--query") settings.query.push(value.split("=", 2));
      else step = { flag, value };
    } else if (takesNone.has(flag)) {
      step = { flag };
    } else {
      throw new Error(`unknown argument ${flag}`);
    }
    if (!step) continue;
    if (step.flag.startsWith("--expect")) groups.at(-1).expectations.push(step);
    else groups.push({ action: step, expectations: [] });
  }
  if (!settings.url) throw new Error("--url is required");
  if (!["chromium", "firefox"].includes(settings.browser)) {
    throw new Error(`--browser must be chromium or firefox, not ${settings.browser}`);
  }
  return { settings, groups };
}

// Runs in the page before its scripts. Records each live-region mutation record — not one read
// per observer callback — so a write replaced within one task is still seen, and a write per key
// in a burst is counted per key; and every keydown as the page received it.
function instrumentPage() {
  window.__announcements = [];
  window.__keydowns = [];
  window.addEventListener(
    "keydown",
    (event) => window.__keydowns.push({ event, key: event.key, repeat: event.repeat }),
    true,
  );
  document.addEventListener("DOMContentLoaded", () => {
    const live = document.getElementById("midi-evil-live");
    new MutationObserver((records) => {
      for (const record of records) {
        if (record.type === "characterData") {
          window.__announcements.push(record.target.data);
        } else if (record.addedNodes.length > 0) {
          window.__announcements.push([...record.addedNodes].map((node) => node.textContent).join(""));
        } else if (record.removedNodes.length > 0) {
          window.__announcements.push("");
        }
      }
    }).observe(live, { childList: true, characterData: true, subtree: true });
  });
}

async function settle(page, frames = SETTLE_FRAMES) {
  await bounded(
    page.evaluate(
      (count) =>
        new Promise((done) => {
          let seen = 0;
          const tick = () => (++seen >= count ? done() : requestAnimationFrame(tick));
          requestAnimationFrame(tick);
        }),
      frames,
    ),
    "waiting for animation frames",
  );
}

async function readState(page) {
  return bounded(
    page.evaluate(() => {
      const canvas = document.getElementById("midi-evil-canvas");
      const active = document.activeElement;
      return {
        active: active?.id || active?.tagName.toLowerCase(),
        activeLabel: active?.getAttribute("aria-label") ?? null,
        activeHidden: active?.closest("[aria-hidden=true]") !== null,
        current: document.getElementById("midi-evil-current").textContent,
        live: document.getElementById("midi-evil-live").textContent,
        announcementCount: window.__announcements.length,
        data: { ...canvas.dataset },
      };
    }),
    "reading the page",
  );
}

// The browser's own accessibility tree, which only Chromium offers a protocol for: the canvas's
// node, and the focused node when it is another.
async function chromiumNodes(page) {
  const session = await bounded(page.context().newCDPSession(page), "opening the protocol");
  try {
    const { nodes } = await bounded(session.send("Accessibility.getFullAXTree"), "reading the tree");
    const describe = (node) => {
      const focused = node.properties?.find((property) => property.name === "focused")?.value?.value;
      return `${node.role?.value} '${node.name?.value ?? ""}' description='${node.description?.value ?? ""}'${focused ? " focused" : ""}`;
    };
    const lines = [];
    const application = nodes.find((node) => node.role?.value === "application");
    lines.push(application ? describe(application) : "no application node");
    const focused = nodes.find(
      (node) =>
        node !== application &&
        node.role?.value !== "RootWebArea" &&
        node.properties?.some((property) => property.name === "focused" && property.value?.value),
    );
    if (focused) lines.push(`focused: ${describe(focused)}`);
    return lines;
  } finally {
    await session.detach().catch(() => {});
  }
}

async function report(page, browserName, label, before) {
  const after = await readState(page);
  const announcements = await bounded(
    page.evaluate((from) => window.__announcements.slice(from), before.announcementCount),
    "reading the announcements",
  );
  const focusLabel = after.activeLabel ? ` '${after.activeLabel}'` : "";
  const hidden = after.activeHidden ? " (aria-hidden)" : "";
  console.log(`after ${label}: focus=${after.active}${focusLabel}${hidden} egui=${after.data.eguiFocus} place=${after.data.cursorPlace}`);
  console.log(`  current: '${after.current}'`);
  for (const announcement of announcements) {
    const marked = announcement.includes("\u00a0") ? " (repeat-marked)" : "";
    console.log(`  announced: '${normalise(announcement)}'${marked}`);
  }
  const snapshot = await bounded(
    page.locator("#editor-neighbourhood").ariaSnapshot(),
    "taking the accessibility snapshot",
  );
  for (const line of snapshot.split("\n")) console.log(`  | ${line}`);
  if (browserName === "chromium") {
    for (const line of await chromiumNodes(page)) console.log(`  chromium: ${line}`);
  }
  return { after, announcements };
}

// What the page must carry for anything else to mean what it says.
async function checkSemantics(page, browserName) {
  const canvas = page.locator(`#${CANVAS_ID}`);
  const checks = [
    ["the canvas's role is application", () => expect(canvas).toHaveRole("application", { timeout: ASSERTION_TIMEOUT_MILLISECONDS })],
    [`the canvas is named '${CANVAS_NAME}'`, () => expect(canvas).toHaveAccessibleName(CANVAS_NAME, { timeout: ASSERTION_TIMEOUT_MILLISECONDS })],
    ["the canvas is described by #midi-evil-current", () => expect(canvas).toHaveAttribute("aria-describedby", "midi-evil-current", { timeout: ASSERTION_TIMEOUT_MILLISECONDS })],
    ["the live region is polite", () => expect(page.locator("#midi-evil-live")).toHaveAttribute("aria-live", "polite", { timeout: ASSERTION_TIMEOUT_MILLISECONDS })],
    ["the live region is atomic", () => expect(page.locator("#midi-evil-live")).toHaveAttribute("aria-atomic", "true", { timeout: ASSERTION_TIMEOUT_MILLISECONDS })],
  ];
  const held = [];
  for (const [what, check] of checks) {
    try {
      await bounded(check(), `checking that ${what}`);
      held.push(what);
    } catch (error) {
      fail(`semantics: ${what} does not hold (${String(error.message).split("\n")[0]})`);
    }
  }
  console.log(`semantics: ${held.length} of ${checks.length} hold — ${held.join("; ")}`);
  if (browserName === "chromium") {
    const session = await bounded(page.context().newCDPSession(page), "opening the protocol");
    try {
      const { nodes } = await bounded(session.send("Accessibility.getFullAXTree"), "reading the tree");
      const live = nodes.find((node) =>
        node.properties?.some((property) => property.name === "live" && property.value?.value === "polite") &&
        node.properties?.some((property) => property.name === "atomic" && property.value?.value === true),
      );
      if (live) console.log("  chromium: a node is live=polite, atomic=true");
      else fail("semantics: Chromium's tree has no node that is live=polite and atomic");
    } finally {
      await session.detach().catch(() => {});
    }
  }
}

async function focusMatches(page, where, after) {
  if (!where.startsWith("role:")) return after.active === where;
  try {
    await bounded(
      expect(page.locator(":focus")).toHaveRole(where.slice("role:".length), { timeout: ASSERTION_TIMEOUT_MILLISECONDS }),
      `checking the focused element's role`,
    );
    return true;
  } catch {
    return false;
  }
}

async function checkReadable(page, text, after, namedFocus) {
  if (!normalise(after.current).includes(text)) {
    fail(`expected '${text}' in the current description, got '${after.current}'`);
    return;
  }
  if (!namedFocus && after.active !== CANVAS_ID) {
    fail(`'${text}' is in the current description, but the page's focus is on ${after.active}, which this step does not name with --expect-focus: what is read there is not checked`);
    return;
  }
  const focused = page.locator(":focus");
  try {
    await bounded(
      expect(focused).toHaveAccessibleName(/\S/, { timeout: ASSERTION_TIMEOUT_MILLISECONDS }),
      "checking the focused element's name",
    );
  } catch {
    fail(`the element with the page's focus (${after.active}) has no accessible name`);
  }
  try {
    await bounded(
      expect(focused).toHaveAccessibleDescription(new RegExp(escapeForRegExp(text)), {
        timeout: ASSERTION_TIMEOUT_MILLISECONDS,
      }),
      "checking the focused element's description",
    );
  } catch {
    fail(`the accessible description of the element with the page's focus (${after.active}) does not contain '${text}'`);
  }
}

async function checkGroup(page, expectations, outcome) {
  const { before, after, announcements } = outcome;
  const focusStep = expectations.find((step) => step.flag === "--expect-focus");
  let namedFocus = false;
  if (focusStep) {
    namedFocus = await focusMatches(page, focusStep.value, after);
    if (!namedFocus) fail(`expected page focus on ${focusStep.value}, got ${after.active}`);
  }
  for (const step of expectations) {
    switch (step.flag) {
      case "--expect-focus":
        break;
      case "--expect":
        await checkReadable(page, step.value, after, namedFocus);
        break;
      case "--expect-name":
        try {
          await bounded(
            expect(page.locator(":focus")).toHaveAccessibleName(step.value, {
              timeout: ASSERTION_TIMEOUT_MILLISECONDS,
            }),
            "checking the focused element's name",
          );
        } catch {
          fail(`the accessible name of the element with the page's focus (${after.active}) is not '${step.value}'`);
        }
        break;
      case "--expect-same":
        if (after.current !== before.current) {
          fail(`expected the description to stay '${before.current}', got '${after.current}'`);
        }
        break;
      case "--expect-remembered":
        if (after.current !== remembered.get(step.value)) {
          fail(`expected '${remembered.get(step.value)}' (kept as ${step.value}), got '${after.current}'`);
        }
        break;
      case "--expect-announcement":
        if (!announcements.some((text) => normalise(text).includes(normalise(step.value)))) {
          fail(`expected an announcement containing '${step.value}', got ${JSON.stringify(announcements.map(normalise))}`);
        }
        break;
      case "--expect-announcements":
        if (announcements.length !== Number(step.value)) {
          fail(`expected ${step.value} announcement(s), got ${announcements.length}`);
        }
        break;
      case "--expect-egui":
        if (after.data.eguiFocus !== step.value) {
          fail(`expected egui focus ${step.value}, got ${after.data.eguiFocus}`);
        }
        break;
      case "--expect-place":
        if (after.data.cursorPlace !== step.value) {
          fail(`expected the cursor on note ${step.value}, got ${after.data.cursorPlace}`);
        }
        break;
      case "--expect-valid":
        if (after.data.projectValid !== "true") fail("the project no longer validates");
        break;
      case "--expect-changed": {
        const name = step.value.replace(/^data-/, "").replace(/-(\w)/g, (_, letter) => letter.toUpperCase());
        if (before.data[name] === after.data[name]) {
          fail(`expected ${step.value} to change, it stayed ${after.data[name]}`);
        }
        break;
      }
      default:
        throw new Error(`not an expectation: ${step.flag}`);
    }
  }
}

// A held key, as a real one arrives: one keydown per animation frame, dispatched from the page.
// Each frame first records what the previous keydown left, then sends the next; the canvas is
// where eframe listens, and the events are untrusted, which eframe does not distinguish.
async function hold(page, key, count) {
  const frames = await bounded(
    page.evaluate(
      ({ key, count }) =>
        new Promise((done) => {
          const canvas = document.getElementById("midi-evil-canvas");
          const current = document.getElementById("midi-evil-current");
          const frames = [];
          let press = 0;
          let announcementsBefore = window.__announcements.length;
          const tick = (time) => {
            if (press > 0) {
              const announcements = window.__announcements.slice(announcementsBefore);
              announcementsBefore = window.__announcements.length;
              frames.push({ press, time, place: canvas.dataset.cursorPlace, current: current.textContent, announcements });
            }
            if (press === count) {
              canvas.dispatchEvent(new KeyboardEvent("keyup", { key, code: key, bubbles: true, cancelable: true }));
              done(frames);
              return;
            }
            canvas.dispatchEvent(
              new KeyboardEvent("keydown", { key, code: key, repeat: press > 0, bubbles: true, cancelable: true }),
            );
            press += 1;
            requestAnimationFrame(tick);
          };
          requestAnimationFrame(tick);
        }),
      { key, count },
    ),
    `holding ${key} for ${count} frames`,
  );
  return frames;
}

async function burst(page, key, count) {
  await bounded(
    page.evaluate(
      ({ key, count }) => {
        const canvas = document.getElementById("midi-evil-canvas");
        for (let press = 0; press < count; press += 1) {
          canvas.dispatchEvent(
            new KeyboardEvent("keydown", { key, code: key, repeat: press > 0, bubbles: true, cancelable: true }),
          );
        }
        canvas.dispatchEvent(new KeyboardEvent("keyup", { key, code: key, bubbles: true, cancelable: true }));
      },
      { key, count },
    ),
    `bursting ${count} × ${key}`,
  );
  await settle(page);
}

// Each chord of the roll's keyboard model and each native global shortcut. `edge` rows are the
// ones a browser may keep for itself: a synthetic key reaches the page past the browser's own
// handling, so their answer is a person's, in a real browser.
const KEY_TABLE = [
  ["ArrowLeft", "roll: previous note"], ["ArrowRight", "roll: next note"],
  ["Home", "roll: first note"], ["End", "roll: last note"],
  ["ArrowUp", "roll: pitch up"], ["ArrowDown", "roll: pitch down"],
  ["Shift+ArrowUp", "roll: octave up"], ["Shift+ArrowDown", "roll: octave down"],
  ["Control+ArrowLeft", "roll: earlier", "edge"], ["Control+ArrowRight", "roll: later", "edge"],
  ["Shift+ArrowLeft", "roll: shorter"], ["Shift+ArrowRight", "roll: longer"],
  ["Enter", "roll: add a note"], ["Delete", "roll: delete the note"],
  ["Shift+Tab", "egui: previous control (from the roll: the Copy button)"],
  ["Tab", "egui: next control; from egui's last stop (the roll) released to the page"],
  ["F6", "regions: next", "edge"], ["Shift+F6", "regions: previous", "edge"],
  ["Control+n", "native: new project", "edge"], ["Control+o", "native: open project", "edge"],
  ["Control+s", "native: save", "edge"], ["Control+z", "native: undo"],
  ["Control+y", "native: redo"], ["Control+Shift+Z", "native: redo"],
  ["Control+w", "browser: close tab (not the application's)", "edge"],
  ["Control+t", "browser: new tab (not the application's)", "edge"],
  ["Alt+ArrowLeft", "native: previous sound", "edge"], ["Alt+ArrowRight", "native: next sound", "edge"],
  ["Space", "native: play/stop"],
  ["Escape", "native: keyboard/pads stop; egui drops focus, the harness gives it back"],
  ...["a", "w", "s", "e", "d", "f", "t", "g", "y", "h", "u", "j", "k", "o", "l", "p", ";", "'", "z", "x"]
    .map((letter) => [letter, "native: computer-keyboard piano"]),
  ...["c", "v", "q", "r", "1", "2", "3", "4"].map((letter) => [letter, "native: pads"]),
];

// Puts the page's focus on the canvas and egui's on the roll, and says whether it got there.
async function focusRoll(page) {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    let state = await readState(page);
    if (state.active === CANVAS_ID && state.data.eguiFocus === "roll") return true;
    if (state.active !== CANVAS_ID) {
      await bounded(page.locator(`#${CANVAS_ID}`).focus(), "focusing the canvas");
      await settle(page);
      state = await readState(page);
      if (state.data.eguiFocus === "roll") continue;
    }
    // From the toolbar F6 goes to the roll; from an editor control, to the toolbar first.
    await bounded(page.keyboard.press("F6"), "pressing F6");
    await settle(page);
  }
  const state = await readState(page);
  return state.active === CANVAS_ID && state.data.eguiFocus === "roll";
}

async function keyTable(open, browserName) {
  console.log(`\n| chord | ${browserName}: egui received it | default prevented by eframe | what it is | how obtained |`);
  console.log("|---|---|---|---|---|");
  let page = null;
  const personRows = [];
  for (const [chord, meaning, edge] of KEY_TABLE) {
    if (!page || page.isClosed()) page = await open();
    let received;
    let prevented = "—";
    if (!(await focusRoll(page))) {
      received = "not pressed: focus could not be put on the roll";
      fail(`key table: focus could not be put on the roll before ${chord}`);
    } else {
      const before = Number((await readState(page)).data.keysReceived);
      try {
        await bounded(page.keyboard.press(chord), `pressing ${chord}`);
        await settle(page);
        const after = await readState(page);
        received = Number(after.data.keysReceived) > before ? `yes (${after.data.lastKey})` : "no";
        prevented = (await bounded(
          page.evaluate(() => window.__keydowns.at(-1)?.event.defaultPrevented),
          "reading defaultPrevented",
        )) ? "yes" : "no";
      } catch (error) {
        if (error instanceof DeadlineExceeded) throw error;
        received = `page closed (${String(error.message).split("\n")[0]})`;
      }
    }
    const how = edge ? "synthetic — needs a person" : "synthetic";
    if (edge) personRows.push(chord);
    console.log(`| ${chord} | ${received} | ${prevented} | ${meaning} | ${how} |`);
  }
  console.log(`\nneeds a person in a real ${browserName}: ${personRows.join(", ")}`);
  if (page && !page.isClosed()) await page.close();
}

async function main() {
  const { settings, groups } = parseArguments(process.argv.slice(2));
  const url = new URL(settings.url);
  for (const [name, value] of settings.query) url.searchParams.set(name, value ?? "");
  const engine = settings.browser === "firefox" ? firefox : chromium;
  // Headless only; WebGL comes from the software rasteriser.
  const browser = await bounded(
    engine.launch(
      settings.browser === "firefox"
        ? { headless: true, firefoxUserPrefs: { "webgl.force-enabled": true } }
        : { headless: true, args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] },
    ),
    "launching the browser",
    LOAD_DEADLINE_MILLISECONDS,
  );
  // The last resort: a run that outlives its deadline closes what it can and exits.
  const watchdog = setTimeout(() => {
    runExpired = true;
    console.error(`web-probe: the run did not finish within ${RUN_DEADLINE_MILLISECONDS} ms`);
    browser.close().finally(() => process.exit(3));
    setTimeout(() => process.exit(3), 10000).unref();
  }, RUN_DEADLINE_MILLISECONDS);
  watchdog.unref();
  const pageErrors = [];
  try {
    const context = await bounded(browser.newContext({ viewport: { width: 1280, height: 900 } }), "opening a context");
    const open = async () => {
      const page = await bounded(context.newPage(), "opening a page");
      page.on("pageerror", (error) => pageErrors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") pageErrors.push(message.text());
      });
      await bounded(page.addInitScript(instrumentPage), "instrumenting the page");
      await bounded(page.goto(url.toString()), "loading the page", LOAD_DEADLINE_MILLISECONDS);
      await bounded(
        page.waitForFunction(() => document.getElementById("midi-evil-canvas")?.dataset.eguiFocus),
        "waiting for the editor to start",
        LOAD_DEADLINE_MILLISECONDS,
      );
      await settle(page);
      await checkSemantics(page, settings.browser);
      return page;
    };
    console.log(`# ${settings.browser} ${browser.version()} — ${url}`);
    let page = await open();
    let outcome = { before: await readState(page), after: await readState(page), announcements: [] };
    for (const { action, expectations } of groups) {
      const before = await readState(page);
      switch (action.flag) {
        case "--loaded":
          break;
        case "--heading":
          console.log(`\n## ${action.value}`);
          break;
        case "--reload":
          await bounded(page.close(), "closing the page");
          page = await open();
          console.log("after reload: a fresh page");
          outcome = { before: await readState(page), after: await readState(page), announcements: [] };
          break;
        case "--remember":
          remembered.set(action.value, outcome.after.current);
          break;
        case "--key-table":
          await keyTable(open, settings.browser);
          break;
        case "--key":
          await bounded(page.keyboard.press(action.value), `pressing ${action.value}`);
          await settle(page);
          outcome = { before, ...(await report(page, settings.browser, action.value, before)) };
          break;
        case "--type":
          await bounded(page.keyboard.type(action.value), `typing '${action.value}'`);
          await settle(page);
          outcome = { before, ...(await report(page, settings.browser, `typing '${action.value}'`, before)) };
          break;
        case "--hold": {
          const frames = await hold(page, action.key, action.count);
          // Left walks the cursor back a note per keydown; every other held key, forward.
          const step = action.key === "ArrowLeft" ? -1 : 1;
          let previousPlace = Number(before.data.cursorPlace);
          for (const frame of frames) {
            const announced = frame.announcements.map(normalise);
            console.log(`  frame ${frame.press} (${frame.time.toFixed(1)} ms): place=${frame.place} current='${frame.current}' announced=${JSON.stringify(announced)}`);
            if (Number(frame.place) !== previousPlace + step) {
              fail(`keydown ${frame.press} left the cursor on ${frame.place} at the next frame, not ${previousPlace + step}`);
            }
            previousPlace = Number(frame.place);
            if (announced.at(-1) !== normalise(frame.current)) {
              fail(`keydown ${frame.press}: the last announcement ${JSON.stringify(announced.at(-1))} is not the cursor's note '${frame.current}'`);
            }
          }
          await settle(page);
          outcome = { before, ...(await report(page, settings.browser, `${action.key} held for ${action.count} frames`, before)) };
          break;
        }
        case "--burst":
          await burst(page, action.key, action.count);
          outcome = { before, ...(await report(page, settings.browser, `${action.count} × ${action.key} in one frame`, before)) };
          break;
        default:
          throw new Error(`unhandled step ${action.flag}`);
      }
      await checkGroup(page, expectations, outcome);
    }
  } catch (error) {
    for (const pageError of pageErrors) console.error(`web-probe: the page raised: ${pageError}`);
    if (!(error instanceof DeadlineExceeded)) throw error;
    // Not an expectation that failed: a negative control must not be satisfied by a hang.
    console.error(`web-probe: a step did not finish: ${error.message}`);
    stepExpired = true;
  } finally {
    clearTimeout(watchdog);
    await bounded(browser.close(), "closing the browser").catch(() => {
      console.error("web-probe: the browser did not close; exiting kills it");
    });
  }
  for (const error of pageErrors) fail(`the page raised: ${error}`);
  if (stepExpired) process.exit(3);
  process.exit(failures.length > 0 ? 1 : 0);
}

// 3: the run or one of its steps outlived its deadline; 2: the probe itself failed (a bad
// argument, a page that would not load); 1: an expectation did not hold, and nothing else.
main().catch((error) => {
  console.error(`web-probe: ${runExpired ? "stopped at the run's deadline: " : ""}${error.message.split("\n")[0]}`);
  process.exit(runExpired ? 3 : 2);
});
