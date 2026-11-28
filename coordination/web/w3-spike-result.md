# W3's spike — what a browser does with MIDI Evil's sound (2026-11-27)

A throwaway build on `spike/w3-audio`, cut from `jesse` at `87fa107`, never merged; built and
measured by Claude Opus 5.5 from `review/web-w3/spike-handoff.md`. Its scripts are on that branch
under `coordination/web/probes/spike-w3/`. Headless Chromium 151 and Firefox 153 through
Playwright 1.62.1 on bob-ross, release bundles. A script installed before the page's own wrapped
`AudioContext`, counted constructions, and listened to everything connected into a destination —
with an analyser and with a worklet that records each sample against the context's clock — and
passed it on at no volume, so the desktop stayed silent. Its onset detector is good to about
12 ms. Not answered: any headed run (no Xvfb here, and the desktop is a person's).

W3's section of the plan is written from these.

1. **A context made in egui's frame starts.** With rodio's `wasm-bindgen` feature the context
   is made in the frame after the in-place render, not in the event's handler — 130 ms after
   the Space in Chromium, 480 ms in Firefox — and sounds in both, for keys alone and for one
   click on a fresh page. With a 6.4 s render between the key and the context, long past the
   moment of the gesture, Firefox still started it: a page that has been interacted with is
   enough. Chromium makes a context running; **Firefox makes every context suspended and
   starts it itself 35 to 90 ms later**, so a context's state read when it is made says
   nothing.
2. **Under a context that never runs, the position stops at 80 ms.** cpal schedules two
   buffers and asks for the next when one ends, which never happens. The transport reads
   "playing, bar 1 beat 1" for as long as anyone waits.
3. **A sink for each Play, as the desktop has, is in time and runs out of nothing.** The
   `long` fixture's onsets, 0.5 s apart: mean 0.5007 s, worst 12.5 ms off, no dropouts, no
   stutter at the start — on the first Play, on a second, and after the page was blocked for
   one second and for three. Sixty Plays and Stops made sixty contexts and closed sixty; none
   failed.
4. **A sink kept for the page's life is worse.** After any block while it lives — the next
   render is one — cpal schedules the overdue buffers at once and the song plays at double
   speed for as long as the block lasted. In Firefox a second Play started that way every
   time.
5. **While the page stops during playback, the sound drops out.** A 200 ms block every second
   lost 117 to 160 ms of sound each time, in both browsers: cpal's stream is fed from the
   page's own thread, about 85 ms ahead.
6. **The other way has no dropouts.** One context resumed inside the web entry's own key and
   pointer listeners, and the preview played as one `AudioBuffer`: in time in every run, the
   200 ms blocks included, running when the buffer starts in both browsers. It costs a seam and
   a module in the web entry; the wasm is 20 KB larger than today against the feature's 41 KB.
7. **How long the page stops for a render**, in milliseconds, Chromium and Firefox: `long`
   (32 s of sound) 93 and 470; `bach` (101 s) 580 and 3340; the Bach invention repeated to
   five minutes 1220 and 6100. Firefox runs the render five to six times slower.
8. **A Space pressed during the render arrives in the frame that takes the result**, after it:
   the preview starts and is stopped in one frame, a context is made and closed within 7 ms,
   and nothing sounds — the tap's peak was zero in all four runs.
9. **Headless Firefox needs the machine's audio server**; without one its context never
   starts. Chromium falls back to an output of its own. Headless Firefox plays through the
   real server — a gate that does not mute its tap plays the fixture out of the speakers.
10. **`page.evaluate` counts as a gesture** in both browsers. A row about a page nobody has
    touched must read the page some other way until its own input is past.
11. **The wasm** is 6.02 MiB today and 6.06 with the feature, against 7.
