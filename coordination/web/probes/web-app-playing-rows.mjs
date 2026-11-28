// The playing group: Play heard in the browser, each row from a fixture of the gate's bundle,
// judged by what the audio tap of web-app-audio-tap.mjs heard and by what the transport says.
// web-app-probe.mjs runs them, after `preflightTheTap` has shown that the tap hears an
// oscillator and nothing without one and that the browser has somewhere to play to.
//
// The transport is read in the arranger, where Space plays and stops and I writes the
// transport into the live region. A row about a page nobody has touched is marked `untouched`:
// the probe reads it through its console alone until the row's own key or click is past, since
// page.evaluate is itself a gesture to both browsers.

import { installAudioTap, onsets } from "./web-app-audio-tap.mjs";
import { expectFixture } from "./web-app-matrix-rows.mjs";
import { RECORDING_REFUSED, enterTheEditor, plain } from "./web-app-rows.mjs";

// Louder than this is sound; a tap that hears nothing reads exactly zero.
const HEARD = 0.01;
const SILENT = 1e-4;
// Where Play is, at the gate's 1440 by 900 window: the toolbar's first block after the project
// buttons, the same on every fixture. A row that clicks it passes only on what is then heard, so
// a click that misses fails.
const PLAY_AT = { x: 368, y: 103 };
// The notes of the long fixture, one a beat at 120 beats a minute.
const NOTE_SPACING_SECONDS = 0.5;
const ONSET_TOLERANCE_SECONDS = 0.025;
const RENDER_DEADLINE_MILLISECONDS = 30000;
const STOPPED_SENTENCE = "The browser is not playing the sound; press Play again.";
const TOO_MUCH = "Too much to play in the browser build yet.";

const wait = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

function pageNow(probe) {
  return probe.page.evaluate(() => performance.now());
}

function tap(probe, read, ...values) {
  return probe.page.evaluate(read, ...values);
}

// "Transport, playing, tempo 120 beats per minute, 4 4 time, bar 3 beat 2, position 0 minutes,
// 5 seconds", as numbers: where it is, in seconds from the song's start by its bar and beat.
function transportOf(text) {
  const found = /^Transport, (\w+), tempo ([\d.]+) beats per minute, (\d+) (\d+) time, bar (\d+) beat (\d+)/.exec(plain(text));
  if (!found) return null;
  const [, playing, tempo, beatsPerBar, , bar, beat] = found;
  const beats = (Number(bar) - 1) * Number(beatsPerBar) + Number(beat) - 1;
  return { playing, bar: Number(bar), beat: Number(beat), seconds: (beats * 60) / Number(tempo), text: plain(text) };
}

// I in the arranger: the one write it makes, read as the transport.
async function readTransport(probe, label = "I, the transport") {
  const state = await probe.press("i", label);
  const write = state.writes.map(plain).find((text) => text.startsWith("Transport, "));
  probe.check(Boolean(write), `${label}: I wrote no transport: ${JSON.stringify(state.writes)}`);
  return transportOf(write ?? "") ?? { playing: "unread", bar: 0, beat: 0, seconds: 0, text: write ?? "" };
}

// I until the transport says `word`, within `milliseconds`.
async function readUntil(probe, word, milliseconds = RENDER_DEADLINE_MILLISECONDS) {
  const until = Date.now() + milliseconds;
  let transport;
  do {
    transport = await readTransport(probe, `I, until ${word}`);
    if (transport.playing === word) return transport;
    await wait(100);
  } while (Date.now() < until);
  probe.fail(`the transport never read ${word} within ${milliseconds / 1000} s: ${transport.text}`);
  return transport;
}

// The fixture, then the arranger, where Space is the transport's.
function arranger(probe, project, tracks) {
  return expectFixture(probe, project, tracks);
}

// Polls the tap until it hears something from `since`, as a render may hold the page first.
async function waitToHear(probe, since, label) {
  const until = Date.now() + RENDER_DEADLINE_MILLISECONDS;
  let peak = 0;
  while (peak <= HEARD && Date.now() < until) {
    await wait(100);
    peak = await tap(probe, (from) => window.midiEvilAudio.peakBetween(from), since);
  }
  probe.log(`    the tap's loudest since ${label}: ${peak.toFixed(4)}`);
  return probe.check(peak > HEARD, `${label}: the tap heard nothing within ${RENDER_DEADLINE_MILLISECONDS / 1000} s`);
}

async function expectHeard(probe, since, label) {
  const peak = await tap(probe, (from) => window.midiEvilAudio.peakBetween(from), since);
  probe.log(`    the tap's loudest since ${label}: ${peak.toFixed(4)}`);
  return probe.check(peak > HEARD, `${label}: the tap heard nothing (loudest ${peak.toFixed(5)})`);
}

// Nothing heard from `since`, by a tap that was listening the whole time.
async function expectSilence(probe, since, label, seconds = 0.4) {
  const { peak, listened } = await tap(
    probe,
    (from) => ({ peak: window.midiEvilAudio.peakBetween(from), listened: window.midiEvilAudio.listenedSince(from) }),
    since,
  );
  probe.log(`    the tap's loudest since ${label}: ${peak.toFixed(5)}, over ${listened.toFixed(2)} s it listened`);
  probe.check(listened >= seconds, `${label}: the tap listened for only ${listened.toFixed(2)} s`);
  return probe.check(peak <= SILENT, `${label}: the tap heard ${peak.toFixed(5)}, not silence`);
}

// Space, then the transport read until playing and the sound heard.
async function playAndHear(probe, label) {
  const pressedAt = await pageNow(probe);
  await probe.press("Space", label);
  await readUntil(probe, "playing");
  await wait(300);
  await expectHeard(probe, pressedAt, label);
  return pressedAt;
}

async function stopAndHearSilence(probe, label) {
  await probe.press("Space", label);
  const stoppedAt = await pageNow(probe);
  const stopped = await readTransport(probe, `I, after ${label}`);
  probe.check(stopped.playing === "stopped", `after ${label} the transport reads ${stopped.text}`);
  await wait(700);
  await expectSilence(probe, stoppedAt + 250, label);
}

// The long fixture's first sixteen notes, heard from `since`: each within 25 ms of half a second
// after the last.
async function expectInTime(probe, since, label) {
  const recording = await tap(probe, (from) => window.midiEvilAudio.recording(from), since);
  const found = onsets(recording);
  const first = found.slice(0, 16);
  const gaps = first.slice(1).map((time, index) => time - first[index]);
  const worst = gaps.reduce((most, gap) => Math.max(most, Math.abs(gap - NOTE_SPACING_SECONDS)), 0);
  probe.log(`    ${label}: ${found.length} onsets; the first sixteen's gaps ${gaps.map((gap) => gap.toFixed(4)).join(" ")}; worst ${(worst * 1000).toFixed(1)} ms off`);
  probe.check(Boolean(recording?.contiguous), `${label}: the tap's recording has a gap of its own`);
  probe.check(first.length === 16, `${label}: ${first.length} onsets heard, not sixteen`);
  probe.check(worst <= ONSET_TOLERANCE_SECONDS, `${label}: a note came ${(worst * 1000).toFixed(1)} ms off half a second after the last`);
}

// A key while the page is held, and the longest the page then answered nothing.
async function longestFrame(probe, since, until = Infinity) {
  return tap(probe, ([from, to]) => window.midiEvilAudio.longestFrameBetween(from, to), [since, until]);
}

// Firefox and Chromium both: the page's last say through the console, past `since`.
function consoleSince(probe, since) {
  return probe.console.filter((line) => line.at >= since);
}

// Into the editor on an untouched page and Tab until Play, reading only the console.
async function tabToPlayUntouched(probe) {
  for (let count = 0; count < 40; count += 1) {
    const current = await probe.pressUntouched("Tab");
    if (current.startsWith("PLAY, button")) return true;
  }
  probe.fail("Tab never reached PLAY on the untouched page");
  return false;
}

async function expectHeardThroughTheConsole(probe, since, label) {
  const heard = await probe.waitForConsole(
    () => consoleSince(probe, since).some((line) => line.text.startsWith("midi-evil-audio: sound")),
    RENDER_DEADLINE_MILLISECONDS,
  );
  return probe.check(heard, `${label}: the tap never said it heard sound`);
}

// A row's errors and warnings since `since`: the page's own logger writes a warning or worse for
// anything that goes wrong, and the tap's lines are its own.
function logged(probe, since) {
  return consoleSince(probe, since).filter((line) => line.type === "error" || line.type === "warning" || line.type === "warn");
}

export const PLAYING_ROWS = {
  "play-by-space": {
    title: "R-11 in the browser: read stopped, Space, read playing, the position a second on is a second later, the tap heard it; Space, read stopped, the tap falls silent",
    fixture: "long",
    async run(probe) {
      await arranger(probe, "long", 1);
      const stopped = await readTransport(probe, "I, before Play");
      probe.check(stopped.playing === "stopped", `before Play the transport reads ${stopped.text}`);
      await playAndHear(probe, "Space, to play");
      const first = await readTransport(probe, "I, once playing");
      await wait(1000);
      const second = await readTransport(probe, "I, a second later");
      const advanced = second.seconds - first.seconds;
      probe.log(`    the position advanced ${advanced.toFixed(2)} s in a second: ${first.text} then ${second.text}`);
      probe.check(advanced >= 0.5 && advanced <= 1.5, `the position advanced ${advanced.toFixed(2)} s in a second`);
      probe.check(second.playing === "playing", `a second on the transport reads ${second.text}`);
      await stopAndHearSilence(probe, "Space, to stop");
    },
  },

  "play-by-a-click": {
    title: "the same by the browser's own pointer, moved to PLAY and pressed, and to STOP: heard, then silent",
    fixture: "long",
    async run(probe) {
      await arranger(probe, "long", 1);
      const clickedAt = await pageNow(probe);
      await probe.page.mouse.click(PLAY_AT.x, PLAY_AT.y);
      await waitToHear(probe, clickedAt, "a click on PLAY");
      // A click leaves nothing of egui's focused; the region chord comes back to the arranger.
      await probe.pressUntil("Alt+PageDown", "Song,");
      const playing = await readTransport(probe, "I, after a click on PLAY");
      probe.check(playing.playing === "playing", `after a click on PLAY the transport reads ${playing.text}`);
      await probe.page.mouse.click(PLAY_AT.x, PLAY_AT.y);
      await probe.settle();
      const stoppedAt = await pageNow(probe);
      await probe.pressUntil("Alt+PageDown", "Song,");
      const stopped = await readTransport(probe, "I, after a click on STOP");
      probe.check(stopped.playing === "stopped", `after a click on STOP the transport reads ${stopped.text}`);
      await wait(700);
      await expectSilence(probe, stoppedAt + 250, "a click on STOP");
    },
  },

  "in-time": {
    title: "in time: the long fixture's first sixteen notes each within 25 ms of half a second after the last, on a first Play, a second, and with the page held 200 ms every second",
    fixture: "long",
    deadlineSeconds: 150,
    async run(probe) {
      await arranger(probe, "long", 1);
      for (const label of ["the first Play", "a second Play"]) {
        const playedAt = await playAndHear(probe, `Space, ${label}`);
        await wait(8500);
        await expectInTime(probe, playedAt, label);
        await stopAndHearSilence(probe, `Space, after ${label}`);
      }
      const playedAt = await playAndHear(probe, "Space, a Play the page is held during");
      await tap(probe, () => {
        window.midiEvilGateHold = setInterval(() => window.midiEvilAudio.block(200), 1000);
      });
      await wait(9000);
      await tap(probe, () => clearInterval(window.midiEvilGateHold));
      const held = await longestFrame(probe, playedAt);
      probe.log(`    the page was held for up to ${Math.round(held)} ms at a time`);
      probe.check(held >= 150, `the page was never held: its longest frame was ${Math.round(held)} ms`);
      await expectInTime(probe, playedAt, "a Play the page is held during");
      await stopAndHearSilence(probe, "Space, after the held Play");
    },
  },

  "keys-alone": {
    title: "keys alone: on a fresh page that has had no pointer event, Tab to PLAY and Space; the tap hears it",
    fixture: "long",
    untouched: true,
    async run(probe) {
      if (!(await tabToPlayUntouched(probe))) return;
      const pressedAt = Date.now();
      await probe.pressUntouched("Space");
      await expectHeardThroughTheConsole(probe, pressedAt, "Tab to PLAY and Space");
    },
  },

  "click-alone": {
    title: "a click alone: on a fresh page, one click on PLAY; the tap hears it",
    fixture: "long",
    untouched: true,
    async run(probe) {
      const clickedAt = Date.now();
      await probe.page.mouse.click(PLAY_AT.x, PLAY_AT.y);
      await expectHeardThroughTheConsole(probe, clickedAt, "one click on PLAY");
    },
  },

  "twenty-plays": {
    title: "twenty Plays and Stops: one context was constructed, and the twentieth is heard",
    fixture: "long",
    deadlineSeconds: 140,
    async run(probe) {
      await arranger(probe, "long", 1);
      let lastPlayedAt = 0;
      for (let round = 1; round <= 20; round += 1) {
        lastPlayedAt = await pageNow(probe);
        await probe.press("Space", `Space, Play ${round}`);
        await readUntil(probe, "playing");
        await wait(200);
        if (round < 20) {
          await probe.press("Space", `Space, Stop ${round}`);
          await readUntil(probe, "stopped", 2000);
        }
      }
      await wait(300);
      await expectHeard(probe, lastPlayedAt, "the twentieth Play");
      const constructed = await tap(probe, () => window.midiEvilAudio.constructed);
      probe.check(constructed === 1, `${constructed} contexts were constructed, not one`);
      await stopAndHearSilence(probe, "Space, the twentieth Stop");
    },
  },

  "space-twice-across-the-render": {
    title: "Space twice with the render between: the second Space, sent while the page renders, stops it; the tap hears nothing and the transport reads stopped",
    // Chords that sound from the song's first frame: a preview started and then stopped within
    // one frame has no silence of the song's own to hide behind.
    fixture: "chords-at-the-limit",
    async run(probe) {
      await arranger(probe, "chords-at-the-limit", 4);
      const pressedAt = await pageNow(probe);
      await probe.page.keyboard.press("Space");
      // The render runs in the next frame and holds the page for half a second or more; a key
      // sent now waits in the browser until the render is done.
      await wait(60);
      await probe.page.keyboard.press("Space");
      await probe.settle(500);
      const inputs = await tap(probe, (from) => window.midiEvilAudio.inputs.filter((input) => input.heardAt >= from && input.name === "keydown"), pressedAt);
      probe.check(inputs.length === 2, `${inputs.length} keydowns reached the page, not two`);
      // The page was held between the two, and the second was sent 60 ms after the first: it
      // was sent while the render held the page.
      const held = inputs.length === 2 ? await longestFrame(probe, inputs[0].heardAt, inputs[1].heardAt) : 0;
      probe.log(`    between the two Spaces the page was held ${Math.round(held)} ms`);
      probe.check(held >= 300, `the page was not rendering between the two Spaces: its longest frame was ${Math.round(held)} ms`);
      const stopped = await readTransport(probe, "I, after both");
      probe.check(stopped.playing === "stopped", `after both the transport reads ${stopped.text}`);
      await wait(700);
      await expectSilence(probe, pressedAt, "Space twice", 1);
    },
  },

  "the-end-and-a-repeat": {
    title: "the end of the song: the arrangement plays to its end and then reads stopped; a repeat plays past its end and the position read has gone back",
    fixture: "arrangement",
    deadlineSeconds: 60,
    async run(probe) {
      await arranger(probe, "Arrangement", 2);
      await playAndHear(probe, "Space, to play to the end");
      await wait(7500);
      const ended = await readTransport(probe, "I, after the end");
      probe.check(ended.playing === "stopped", `after the song's end the transport reads ${ended.text}`);
      // Repeat section, bars 1 to 2: the first bar again and again.
      await probe.pressUntil("Shift+Tab", "Repeat section");
      const checked = await probe.press("Space", "Space on Repeat section");
      probe.check(plain(checked.current).startsWith("Repeat section, check box, checked"), `the check box reads ${checked.current}`);
      await probe.pressUntil("Alt+PageDown", "Song,");
      // The loop is the start bar and the one after it, four seconds at this tempo.
      await playAndHear(probe, "Space, a repeat");
      const early = await readUntilPast(probe, 3);
      await wait(1500);
      const later = await readTransport(probe, "I, past the repeat's end");
      probe.log(`    the repeat read ${early.text}, then ${later.text}`);
      probe.check(later.playing === "playing", `past its end the repeat reads ${later.text}`);
      probe.check(later.seconds < early.seconds, `the position did not go back: ${early.seconds} s then ${later.seconds} s`);
      await stopAndHearSilence(probe, "Space, to stop the repeat");
    },
  },

  "b-then-space": {
    title: "B, then Space: playback starts at the part's bar, and the position read says so",
    fixture: "arrangement",
    async run(probe) {
      await arranger(probe, "Arrangement", 2);
      await probe.press("End", "End, to the last part");
      const moved = await probe.press("b", "B");
      probe.expectWrites(moved, ["Playback starts at bar 3."], "B");
      await playAndHear(probe, "Space, from bar 3");
      const playing = await readTransport(probe, "I, once playing");
      probe.check(playing.bar === 3, `playing from bar 3 the transport reads ${playing.text}`);
      // From bar 3 there is one note to play, and the song then ends by itself.
      await readUntil(probe, "stopped", 5000);
    },
  },

  "too-much": {
    title: "too much: the chords at the limit play; a bar longer is refused with its sentence on Play of the whole and of two bars at its end, nothing rendered and the frame no longer than any, while its first bars are heard",
    fixture: "chords-at-the-limit",
    deadlineSeconds: 120,
    async run(probe) {
      await arranger(probe, "chords-at-the-limit", 4);
      await playAndHear(probe, "Space, the chords at the limit");
      await stopAndHearSilence(probe, "Space, to stop");

      await probe.open(probe.url.replace(/fixture=[^&]*/, "fixture=chords-over-the-limit"));
      await arranger(probe, "chords-over-the-limit", 4);
      const refuse = async (label) => {
        const pressedAt = await pageNow(probe);
        const refused = await probe.press("Space", label);
        probe.expectWrites(refused, [TOO_MUCH], label);
        await wait(600);
        const frame = await longestFrame(probe, pressedAt);
        probe.log(`    ${label}: the longest frame after it was ${Math.round(frame)} ms`);
        probe.check(frame <= 100, `${label}: the frame that refused took ${Math.round(frame)} ms`);
        const transport = await readTransport(probe, `I, after ${label}`);
        probe.check(transport.playing === "stopped", `after ${label} the transport reads ${transport.text}`);
        await expectSilence(probe, pressedAt, label);
      };
      await refuse("Space, the whole of the chords a bar too long");
      // Start bar 30 of 31, playing through to the end: the renderer still works from the start.
      await setTransportBox(probe, "Start bar", "30");
      await probe.pressUntil("Alt+PageDown", "Song,");
      await refuse("Space, its last two bars");
      // Bars 1 to 2, repeated: well within the limit.
      await setTransportBox(probe, "Start bar", "1");
      await setTransportBox(probe, "Loop end bar", "3");
      await probe.pressUntil("Tab", "Repeat section");
      await probe.press("Space", "Space on Repeat section");
      await probe.pressUntil("Alt+PageDown", "Song,");
      await playAndHear(probe, "Space, its first bars");
      await stopAndHearSilence(probe, "Space, to stop its first bars");
    },
  },

  "record-says-its-reason": {
    title: "Record keeps its stop and says its own reason",
    fixture: "long",
    async run(probe) {
      await enterTheEditor(probe);
      await probe.pressUntil("Tab", "Record, toggle button");
      await probe.expectFocus(`Record, toggle button, not pressed, unavailable. ${RECORDING_REFUSED}`);
      const pressed = await probe.press("Enter", "Enter on Record");
      probe.expectWrites(pressed, [RECORDING_REFUSED], "Record activated");
      probe.check(plain(pressed.current).startsWith("Record,"), "focus left Record");
    },
  },

  "a-device-that-will-not-open": {
    title: "a device that will not open: the constructor throws; Play says the failure and the transport reads stopped; with it restored, Play is heard",
    fixture: "long",
    async run(probe) {
      await tap(probe, () => {
        window.midiEvilAudio.constructorThrows = true;
      });
      await arranger(probe, "long", 1);
      const failed = await probe.press("Space", "Space, with no context to be had");
      await probe.settle(300);
      const said = [...failed.writes, ...(await probe.read()).writes].map(plain);
      probe.log(`    written: ${JSON.stringify(said)}`);
      probe.check(
        said.some((text) => text.startsWith("Could not start audio preview: the browser made no audio context: ")),
        `Play did not say the failure: ${JSON.stringify(said)}`,
      );
      const stopped = await readTransport(probe, "I, after the failure");
      probe.check(stopped.playing === "stopped", `after the failure the transport reads ${stopped.text}`);
      await tap(probe, () => {
        window.midiEvilAudio.constructorThrows = false;
      });
      await playAndHear(probe, "Space, with the constructor restored");
      await stopAndHearSilence(probe, "Space, to stop");
    },
  },

  "a-context-that-never-starts": {
    title: "a context that never starts: within four seconds of Play the sentence is heard and the transport reads stopped; released, the next Play is heard, on the one context",
    fixture: "long",
    async run(probe) {
      await tap(probe, () => window.midiEvilAudio.holdSuspended());
      await arranger(probe, "long", 1);
      await expectGivenUp(probe, "Space, on a context held suspended");
      await tap(probe, () => window.midiEvilAudio.release());
      await playAndHear(probe, "Space, released");
      const constructed = await tap(probe, () => window.midiEvilAudio.constructed);
      probe.check(constructed === 1, `${constructed} contexts were constructed, not one`);
      await stopAndHearSilence(probe, "Space, to stop");
    },
  },

  "a-context-that-stops": {
    title: "a context that stops: suspended ten seconds into a song, within four seconds the sentence is heard and the transport reads stopped; released, the next Play is heard, on the one context",
    fixture: "long",
    deadlineSeconds: 60,
    async run(probe) {
      await arranger(probe, "long", 1);
      await playAndHear(probe, "Space, to play");
      await wait(10000);
      await expectGivenUp(probe, "the context suspended ten seconds in", () => tap(probe, () => window.midiEvilAudio.holdSuspended()));
      await tap(probe, () => window.midiEvilAudio.release());
      await playAndHear(probe, "Space, released");
      const constructed = await tap(probe, () => window.midiEvilAudio.constructed);
      probe.check(constructed === 1, `${constructed} contexts were constructed, not one`);
      await stopAndHearSilence(probe, "Space, to stop");
    },
  },

  "stop-then-wait": {
    title: "Stop, then wait, and the song's end, then wait: five seconds on, the page answers a key and has logged nothing",
    fixture: "arrangement",
    deadlineSeconds: 60,
    async run(probe) {
      await arranger(probe, "Arrangement", 2);
      const since = Date.now();
      await playAndHear(probe, "Space, to play");
      await stopAndHearSilence(probe, "Space, to stop");
      await wait(5000);
      const answered = await readTransport(probe, "I, five seconds after Stop");
      probe.check(answered.playing === "stopped", `five seconds after Stop the transport reads ${answered.text}`);
      await playAndHear(probe, "Space, to play to the end");
      await wait(7000 + 5000);
      const atRest = await readTransport(probe, "I, five seconds after the song's end");
      probe.check(atRest.playing === "stopped", `five seconds after the end the transport reads ${atRest.text}`);
      const lines = logged(probe, since);
      probe.check(lines.length === 0, `the page logged ${lines.length} lines: ${JSON.stringify(lines.map((line) => line.text.slice(0, 200)))}`);
    },
  },

  "never-played-is-silent": {
    title: "the negative control's page: keys pressed but never Play, and the tap, listening, hears nothing",
    fixture: "long",
    async run(probe) {
      await arranger(probe, "long", 1);
      const since = await pageNow(probe);
      await probe.press("ArrowRight", "Right, which plays nothing");
      await wait(2500);
      await expectSilence(probe, since, "keys but never Play", 2);
      const constructed = await tap(probe, () => window.midiEvilAudio.constructed);
      probe.check(constructed === 1, `the page's keys made ${constructed} contexts, not one`);
    },
  },

  "frame-that-handles-play": {
    title: "measured: the frame that handles Play, for the Bach invention, the invention at the limit and the four tracks of chords at the limit; none holds the page more than five seconds",
    fixture: "bach",
    deadlineSeconds: 120,
    async run(probe) {
      for (const [fixture, project, tracks] of [
        ["bach", "bach-invention-02", 2],
        ["bach-at-the-limit", "bach-at-the-limit", 2],
        ["chords-at-the-limit", "chords-at-the-limit", 4],
      ]) {
        if (fixture !== "bach") await probe.open(probe.url.replace(/fixture=[^&]*/, `fixture=${fixture}`));
        await arranger(probe, project, tracks);
        const playedAt = await playAndHear(probe, `Space, ${fixture}`);
        const frame = await longestFrame(probe, playedAt);
        probe.log(`    MEASURED: the frame that handles Play for ${fixture}: ${Math.round(frame)} ms`);
        probe.check(frame <= 5000, `the frame that handles Play for ${fixture} held the page ${Math.round(frame)} ms, over five seconds`);
        await stopAndHearSilence(probe, `Space, to stop ${fixture}`);
      }
    },
  },
};

// I until the position is at least `seconds` into the song.
async function readUntilPast(probe, seconds) {
  const until = Date.now() + 5000;
  let transport;
  do {
    transport = await readTransport(probe, `I, until ${seconds} s in`);
    if (transport.seconds >= seconds) return transport;
    await wait(100);
  } while (Date.now() < until);
  probe.fail(`the position never passed ${seconds} s: ${transport.text}`);
  return transport;
}

// A box of the transport's row, given a value typed in and committed with Enter.
async function setTransportBox(probe, name, value) {
  await probe.pressUntil("Shift+Tab", name);
  await probe.press("Control+a");
  await probe.type(value);
  const committed = await probe.press("Enter", `Enter on ${name}`);
  probe.check(plain(committed.current).includes(`${name}`), `the box committed is not ${name}: ${committed.current}`);
}

// A preview whose device does not run: within four seconds of `start` — Space, unless given —
// the sentence is written and the transport then reads stopped.
async function expectGivenUp(probe, label, start = () => probe.page.keyboard.press("Space")) {
  const startedAt = Date.now();
  await start();
  let heard = false;
  while (!heard && Date.now() - startedAt < 5000) {
    await probe.settle(50);
    heard = (await probe.read()).writes.map(plain).some((text) => text.includes(STOPPED_SENTENCE));
  }
  const seconds = (Date.now() - startedAt) / 1000;
  probe.log(`    ${label}: the sentence ${heard ? `was written ${seconds.toFixed(1)} s on` : "was never written"}`);
  probe.check(heard && seconds <= 4, `${label}: the sentence was not written within four seconds`);
  const stopped = await readTransport(probe, `I, after ${label}`);
  probe.check(stopped.playing === "stopped", `after ${label} the transport reads ${stopped.text}`);
}

// Before any row: on a bare page of the bundle's own origin, with the tap installed, a context
// the probe makes must start — headless Firefox needs the machine's audio server, and without one
// its context never runs, which refuses the group rather than failing it — and the tap must hear
// nothing while nothing is connected and an oscillator once one is.
export async function preflightTheTap(engine, settings, log, ToolingFailure) {
  const browser = await engine.launch(
    settings.browser === "firefox"
      ? { headless: true, firefoxUserPrefs: { "webgl.force-enabled": true } }
      : { headless: true, args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] },
  );
  try {
    const page = await (await browser.newContext()).newPage();
    const bare = new URL("audio-preflight.html", settings.gateUrl ?? settings.url).href;
    await page.route(bare, (route) => route.fulfill({ contentType: "text/html", body: "<!doctype html><title>preflight</title><button>button</button>" }));
    await page.addInitScript(installAudioTap, {});
    await page.goto(bare);
    await page.click("button");
    const result = await page.evaluate(async () => {
      const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
      const audio = window.midiEvilAudio;
      const context = new AudioContext();
      context.resume();
      const started = performance.now();
      while (context.state !== "running" && performance.now() - started < 5000) await sleep(50);
      if (context.state !== "running") return { running: false, state: context.state };
      await sleep(300);
      const quietFrom = performance.now();
      await sleep(1000);
      const quiet = { peak: audio.peakBetween(quietFrom), listened: audio.listenedSince(quietFrom) };
      const oscillator = context.createOscillator();
      oscillator.connect(context.destination);
      oscillator.start();
      const loudFrom = performance.now();
      await sleep(1000);
      const loud = { peak: audio.peakBetween(loudFrom), listened: audio.listenedSince(loudFrom) };
      oscillator.stop();
      return { running: true, quiet, loud, errors: audio.errors };
    });
    if (!result.running) {
      throw new ToolingFailure(
        `the playing group is refused, not failed: ${settings.browser}'s audio context never started (it stayed ${result.state}); headless Firefox needs the machine's audio server`,
      );
    }
    log(`## ${settings.browser}: the tap, before any row: nothing connected, loudest ${result.quiet.peak.toFixed(5)} over ${result.quiet.listened.toFixed(2)} s; an oscillator connected, loudest ${result.loud.peak.toFixed(3)} over ${result.loud.listened.toFixed(2)} s`);
    log("   (rows marked untouched read the page through its console alone until their own key or click is past: page.evaluate is a gesture)");
    const deaf = result.quiet.listened < 0.8 || result.loud.listened < 0.8;
    if (deaf || result.quiet.peak > SILENT || result.loud.peak < 0.1 || result.errors.length) {
      throw new ToolingFailure(`the tap does not hear what it must: ${JSON.stringify(result)}`);
    }
  } finally {
    await browser.close();
  }
}
