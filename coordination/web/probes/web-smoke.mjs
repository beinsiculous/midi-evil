// Opens the browser build's page once in a headless browser with the site's Playwright and judges
// whether the workstation started: within the draw deadline, a screenshot of the canvas element
// itself — never the page, whose visible text would make any screenshot pass — must hold more
// than one colour, and over the whole visit the page must raise no error and no panic, log
// nothing as an error, have no request fail, never call navigator.requestMIDIAccess — a page
// that asks for MIDI on load puts a permission prompt in front of a visitor who pressed nothing —
// and construct no AudioContext, which the page makes only in a visitor's first key or press.
// There is no list of tolerated errors: the page asks for nothing but its own files. Polling stops
// at the first error. The bundle is served by the caller (web-smoke.sh); this script only looks.
//
//   PLAYWRIGHT_FROM=…/insiculous_web/package.json \
//   node web-smoke.mjs --url http://127.0.0.1:PORT/bundle/ --browser chromium --screenshot out.png
//
// It deliberately clicks nothing, presses nothing and moves no pointer: anything reached by a
// gesture is not part of starting. The canvas screenshot it judged last is written to
// --screenshot whether the run passes or not.
//
// Exit status: 0, the workstation started; 1, a check failed — a page that does not finish loading
// among them — and each failure is printed on a line beginning "FAILED:"; 2, the tooling failed —
// Playwright would not load or the browser would not launch — and nothing was judged.

import { writeFile } from "node:fs/promises";
import { createRequire } from "node:module";

// How long the canvas has to show more than one colour after the page has loaded, and how long
// the page is then watched for an error from the first frames. A run stopped by an error captures
// the canvas once more a moment later, so the transcript says whether the page drew all the same.
const DRAW_DEADLINE_MILLISECONDS = 60000;
const WATCH_AFTER_DRAWING_MILLISECONDS = 5000;
const CAPTURE_AFTER_ERROR_MILLISECONDS = 2000;
const POLL_MILLISECONDS = 500;
const LOAD_DEADLINE_MILLISECONDS = 30000;
const VIEWPORT = { width: 1280, height: 900 };

// Thrown for whatever keeps the run from judging the page at all; it ends the run with status 2
// once the browser is closed.
class ToolingFailure extends Error {}

function toolingFailure(message) {
  throw new ToolingFailure(message);
}

function parseArguments(argv) {
  const settings = { url: null, browser: null, screenshot: null };
  for (let index = 0; index < argv.length; index += 2) {
    const [flag, value] = [argv[index], argv[index + 1]];
    if (value === undefined) toolingFailure(`${flag} needs a value`);
    if (flag === "--url") settings.url = value;
    else if (flag === "--browser") settings.browser = value;
    else if (flag === "--screenshot") settings.screenshot = value;
    else toolingFailure(`unknown argument ${flag}`);
  }
  if (!settings.url || !settings.screenshot || !["chromium", "firefox"].includes(settings.browser)) {
    toolingFailure("usage: --url URL --browser chromium|firefox --screenshot PATH");
  }
  return settings;
}

// PLAYWRIGHT_FROM names the package.json whose node_modules holds Playwright; web-smoke.sh finds
// the working set's.
function loadPlaywright() {
  const packageJson = process.env.PLAYWRIGHT_FROM;
  if (!packageJson) toolingFailure("PLAYWRIGHT_FROM names no package.json to load Playwright beside");
  try {
    return createRequire(packageJson)("playwright");
  } catch (error) {
    return toolingFailure(`Playwright could not be loaded beside ${packageJson}: ${error.message}`);
  }
}

// The number of distinct colours in a PNG, decoded by a blank page of the same browser so no
// image library is needed; it stops counting at two, since one is all a blank canvas has.
async function countColours(decoder, png) {
  return decoder.evaluate(async (base64) => {
    const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
    const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
    const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
    const drawing = surface.getContext("2d");
    drawing.drawImage(bitmap, 0, 0);
    const { data } = drawing.getImageData(0, 0, bitmap.width, bitmap.height);
    const pixels = new Uint32Array(data.buffer, data.byteOffset, data.byteLength / 4);
    const colours = new Set();
    for (const pixel of pixels) {
      colours.add(pixel);
      if (colours.size > 1) break;
    }
    return colours.size;
  }, png.toString("base64"));
}

async function main() {
  const settings = parseArguments(process.argv.slice(2));
  const { chromium, firefox } = loadPlaywright();
  const engine = settings.browser === "firefox" ? firefox : chromium;
  let browser;
  try {
    // The launch settings web-probe.mjs drew its canvas with: headless, WebGL from the software
    // rasteriser.
    browser = await engine.launch(
      settings.browser === "firefox"
        ? { headless: true, firefoxUserPrefs: { "webgl.force-enabled": true } }
        : { headless: true, args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] },
    );
  } catch (error) {
    toolingFailure(`${settings.browser} did not launch: ${error.message.split("\n")[0]}`);
  }
  const failures = [];
  try {
    const context = await browser.newContext({ viewport: VIEWPORT });
    const decoder = await context.newPage();
    const page = await context.newPage();
    // Installed before any of the page's own scripts run; the request still goes through, so the
    // page behaves as it would unwatched.
    await page.addInitScript(() => {
      window.midiEvilSmokeMidiRequests = 0;
      const requestMidiAccess = navigator.requestMIDIAccess?.bind(navigator);
      if (requestMidiAccess) {
        navigator.requestMIDIAccess = (...options) => {
          window.midiEvilSmokeMidiRequests += 1;
          return requestMidiAccess(...options);
        };
      }
      window.midiEvilSmokeAudioContexts = 0;
      const NativeContext = window.AudioContext;
      if (NativeContext) {
        window.AudioContext = class extends NativeContext {
          constructor(...options) {
            super(...options);
            window.midiEvilSmokeAudioContexts += 1;
          }
        };
      }
    });
    const errors = [];
    page.on("pageerror", (error) => errors.push(`the page raised: ${error.message}`));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(`the console logged an error: ${message.text()}`);
      else if (/panicked/i.test(message.text())) errors.push(`the console logged a panic: ${message.text()}`);
    });
    page.on("requestfailed", (request) =>
      errors.push(`a request failed: ${request.url()} (${request.failure()?.errorText})`),
    );
    page.on("response", (response) => {
      if (response.status() >= 400) errors.push(`a request answered ${response.status()}: ${response.url()}`);
    });
    console.log(`# ${settings.browser} ${browser.version()} — ${settings.url}`);
    // The server has already answered with this bundle, so a page that does not finish loading is
    // the page's failure, judged like any other.
    let loaded = true;
    try {
      await page.goto(settings.url, { waitUntil: "load", timeout: LOAD_DEADLINE_MILLISECONDS });
    } catch (error) {
      loaded = false;
      failures.push(`the page did not finish loading: ${error.message.split("\n")[0]}`);
    }
    if (loaded) failures.push(...(await judgePage(page, decoder, errors, settings.screenshot)));
    failures.push(...errors);
  } finally {
    await browser.close();
  }
  for (const failure of failures) console.log(`FAILED: ${failure}`);
  if (failures.length === 0) console.log("PASSED: the workstation started, and the page raised nothing");
  return failures.length > 0 ? 1 : 0;
}

// What a loaded page shows: one canvas that draws, no request for MIDI access, and no audio.
async function judgePage(page, decoder, errors, screenshotPath) {
  const failures = [];
  const canvases = page.locator("canvas");
  const canvasCount = await canvases.count();
  if (canvasCount !== 1) failures.push(`the page holds ${canvasCount} canvases, not one`);
  const loadedAt = Date.now();
  let screenshot = null;
  let colours = 0;
  if (canvasCount === 1) {
    try {
      ({ screenshot, colours } = await watchCanvas(page, canvases, decoder, errors, loadedAt));
    } catch (error) {
      failures.push(`the canvas could not be captured: ${error.message.split("\n")[0]}`);
    }
    if (screenshot) {
      await writeFile(screenshotPath, screenshot);
      console.log(`canvas screenshot: ${screenshotPath}`);
    }
    if (colours <= 1) {
      failures.push(`the canvas holds one colour, ${screenshot ? "as last captured" : "or none was captured"}`);
    }
  }
  try {
    const midiRequests = await page.evaluate(() => window.midiEvilSmokeMidiRequests);
    if (midiRequests !== 0) failures.push(`the page asked for MIDI access: navigator.requestMIDIAccess was called ${midiRequests} time(s)`);
  } catch (error) {
    failures.push(`whether the page asked for MIDI access could not be read: ${error.message.split("\n")[0]}`);
  }
  try {
    const contexts = await page.evaluate(() => window.midiEvilSmokeAudioContexts);
    if (contexts !== 0) failures.push(`the page made audio: ${contexts} AudioContext(s) were constructed`);
  } catch (error) {
    failures.push(`whether the page made audio could not be read: ${error.message.split("\n")[0]}`);
  }
  return failures;
}

// Captures the canvas until it holds more than one colour, the page records an error, or the draw
// deadline passes. If it drew, the first frames are watched and the canvas captured once more; if
// an error stopped it, the canvas is captured once more a moment later. The last capture is the
// one judged.
async function watchCanvas(page, canvases, decoder, errors, loadedAt) {
  const capture = async () => {
    const screenshot = await canvases.screenshot({ timeout: LOAD_DEADLINE_MILLISECONDS });
    return { screenshot, colours: await countColours(decoder, screenshot) };
  };
  let screenshot = null;
  let colours = 0;
  while (Date.now() - loadedAt < DRAW_DEADLINE_MILLISECONDS && errors.length === 0) {
    ({ screenshot, colours } = await capture());
    if (colours > 1) break;
    await page.waitForTimeout(POLL_MILLISECONDS);
  }
  if (errors.length > 0) {
    console.log(`an error was recorded ${Date.now() - loadedAt} ms after the page loaded; polling stopped`);
    await page.waitForTimeout(CAPTURE_AFTER_ERROR_MILLISECONDS);
    ({ screenshot, colours } = await capture());
    console.log(`the canvas held ${colours > 1 ? "more than one colour" : "one colour"} ${CAPTURE_AFTER_ERROR_MILLISECONDS} ms later`);
  } else if (colours > 1) {
    console.log(`the canvas held more than one colour ${Date.now() - loadedAt} ms after the page loaded`);
    await page.waitForTimeout(WATCH_AFTER_DRAWING_MILLISECONDS);
    ({ screenshot, colours } = await capture());
  } else {
    console.log(`the canvas held one colour until the ${DRAW_DEADLINE_MILLISECONDS} ms deadline`);
  }
  return { screenshot, colours };
}

// The status is set rather than exited with, so every line already written reaches a pipe before
// node ends; the caller's negative controls read those lines.
main().then(
  (status) => {
    process.exitCode = status;
  },
  (error) => {
    const reason = error instanceof ToolingFailure ? error.message : `the run stopped: ${error.message.split("\n")[0]}`;
    console.error(`web-smoke: the tooling failed: ${reason}`);
    process.exitCode = 2;
  },
);
