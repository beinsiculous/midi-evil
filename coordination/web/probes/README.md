# Probes

Mechanical evidence for the screen-reader gates (plan D4): transcripts an agent can produce and
re-run. They are half of each gate; the other half is a person listening with Orca.

## `atspi-probe.py` and `run-probe.sh` — the native application over AT-SPI

`run-probe.sh` starts MIDI Evil, runs `atspi-probe.py` against it with the script's own
arguments, stops everything it started, and exits with the probe's status. `probe_support.py`
beside them holds what the probe stands on: its isolation check, its event pump, and the sender
that delivers keys to the application's own X window. The probe finds the
application on the AT-SPI bus, then runs its steps in the order given: it performs actions on
named nodes, delivers keys, checks what each step left, and prints the focused node and the
application's accessibility events in the half second after each key. Run from the repository
root, with a debug build in `target/debug/`:

```sh
coordination/web/probes/run-probe.sh --dump
coordination/web/probes/run-probe.sh --press "PIANO ROLL" --key F6 --key F6 --key F6 \
    --expect "Piano roll" --key Right --key ctrl+Right --key shift+Left
coordination/web/probes/run-probe.sh --press "PIANO ROLL" --focus "Zoom" --key Right --listen 0.5
```

The application opens the tracked fixture
`examples/public-domain/bach-invention-02/bach-invention-02.mid`; `MIDI_EVIL_FILE` and
`MIDI_EVIL_BINARY` choose another file or build.

Steps, run in command-line order:

- `--press NAME` performs the first action (click) of the node with that name — this is how the
  probe switches views and presses toolbar buttons without a pointer.
- `--press-in "REGION|NAME"` presses the node named NAME inside the node named REGION — a region
  such as `Side panel` — for names two controls share. `--press` itself refuses a name that
  several pressable nodes carry, so it can never silently press the other one.
- `--focus NAME` gives that node focus through AT-SPI (AccessKit's Focus action). A gate does not
  use it to reach the roll: the roll is reached with F6, as a person would.
- `--key CHORD` delivers a key: an X keysym name (`Tab`, `Left`, `Delete`, `Home`, `F6`, `a`) or
  one of `Enter`, `Space`, `+`, with any of `shift+`, `ctrl+`/`control+`, `alt+` in front
  (`shift+Tab`, `ctrl+Left`, `shift+F6`, `shift++`). Every chord is checked before the first is
  sent; a bad one stops the probe with one line on stderr. A keysym that sits on a key's shifted
  level (`+`, `A`) is sent with Shift added.
- `--burst "CHORD CHORD …"` delivers several chords back to back with nothing pumped between.
  Nothing outside the application can show they were read in one frame, so a burst row shows the
  net result only; the one-frame ordering is proven by the crate's tests, which feed one frame by
  construction: `two_keys_in_one_frame_act_in_the_order_they_arrived` and
  `an_arrow_in_the_frame_after_f6_moves_the_cursor_and_focus_stays` (`app/keyboard_model_tests.rs`),
  and `keys_after_a_tab_in_the_same_frame_never_edit_the_note_being_left`
  (`app/key_dispatch_tests.rs`).
- `--hold CHORD:REPEATS` presses a chord, sends REPEATS further presses of its key 40 ms apart
  with no release between — winit reports each as a repeat — waits, then releases it. It records
  the focused node's name changes while the key repeated and after the release, apart.
- `--set-value LABEL:VALUE` sets the value box that belongs to the label LABEL — the first node
  with a value among the label's following siblings, before the next caption — through AT-SPI's
  Value interface, which AccessKit turns into the widget's SetValue action: the
  note inspector's boxes are reached this way while they have no names of their own.
- The expectations check what the last `--press`, `--focus` or `--key` left; several may follow
  one step. `--expect TEXT` fails the run unless the focused node's name contains TEXT;
  `--expect-not TEXT` fails if it does, **or if nothing is focused** — it exists to show focus went
  somewhere else. `--expect-same-name` fails unless a node is focused and its name is what it was
  before that step. `--expect-node TEXT` fails unless some node, focused or not, has a name
  containing TEXT. `--expect-announcement TEXT` fails unless an `object:announcement` raised
  during that step contains TEXT, and `--expect-announcement-count TEXT:COUNT` unless exactly
  COUNT of them do. `--expect-region NAME` fails unless the focused node is inside a node named
  NAME (the regions are named groups: Toolbar, Side panel, Editor); `--expect-description TEXT`
  unless its description contains TEXT. `--expect-enabled NAME` and `--expect-disabled NAME` read
  the enabled and sensitive states of the one node named NAME. AccessKit's AT-SPI adapter reports
  a disabled state only for roles that can be read-only — text inputs, checkboxes, sliders, combo
  boxes and the like — so a disabled *button* always reads enabled, and these steps cannot show it. After a `--hold`, `--expect-hold-name-changes COUNT` and
  `--expect-release-name-changes COUNT` count the focused name's changes while the key repeated
  and after its release; a step that was not a `--hold` leaves no counts, so these fail after
  one. A failed expectation is one line on stderr and exit status 1. The new steps' arguments
  (`--hold Up:5x`, a count that is not a number) are checked with the chords, before anything
  runs, and a call the accessibility bus refuses is one line on stderr, not a traceback.
- `--heading TEXT` prints a line into the transcript; `--dump` prints each node as role, name,
  description, value, text, focus states, actions and extents in window coordinates.

**Names that carry state.** The piano roll's name is "Piano roll, " followed by the cursor's note,
so it changes with every key. `--press` and `--focus` therefore take a usable node with exactly
that name first and, when there is none, the usable nodes whose name *begins* with it
(`--focus "Piano roll"`); `--expect` matches by *contains*. `--focus` uses the first of several;
`--press` refuses a prefix that several pressable nodes share — the status line repeats the roll's
name and is itself pressable, so a guess would press the wrong one and say it had not.

- `--application` is the AT-SPI application name, which AccessKit takes from the executable's
  file name (`midi-evil`; a renamed copy of the binary is listed under its new name).
- `--wait` bounds every wait: for the application, its window, and its keyboard focus.
- `--listen` keeps printing events for that many seconds after the last step.

## `native-gate.sh` — the native gate's mechanical half

Runs the native gate of W-A1a (plan § W-A1a, *Gate rows*) and prints the transcript: first the
refusal rows — the inner entry with the desktop's own environment; one row per clause of its
check, each with every other clause satisfied so only that clause can refuse; a control with all
of them satisfied, which must get past the check; and the probe's own check called without the
launcher — each requiring the desktop's screen-reader flag and settings files to be unchanged;
then W0's sequence and its five edges, less the transport; then the rows found in W0's review,
R-1 to R-10 and R-14 to R-20, each in a fresh application. Every step a row names carries an
expectation, so exit status 0 means each key did what the row says. Rows that need a project of
their own — two identical notes, a bass part, two tracks — write it as a small `.evil` file into a
private temporary directory, removed on exit. What keys cannot show from outside (which of two
identical notes a Delete took, a lane point clicked with a pointer) is proven by the crate's
tests, and those rows say so in their titles. Each row is headed with the configuration it ran in;
the other half of the gate is a person performing the same keys with Orca, Wayland-native.

```sh
MIDI_EVIL_BINARY=/path/to/target/debug/midi-evil coordination/web/probes/native-gate.sh
```

Needs only `python3` and `gi` (`Atspi` 2.0), `libX11`, and `dbus-run-session`, which every
desktop has.

### What the session needs, and why the script does what it does

**The application runs under XWayland.** Keys are sent with `XSendEvent` to the application's own
X window, so they can reach that window or nothing. winit ignores keys while its window is not the
X input focus, so the window must keep focus while the probe runs; before every key the probe
waits up to `--wait` for it and stops, without sending, if focus does not come back.

**Modifier chords arrive modified.** For `shift+Tab` the probe sends Shift's own press, then Tab
with Shift in the event's state, then the releases — all to the application's window. winit 0.30
takes its modifiers from each key event's state field. Proven for Shift and Control: from a
focused button, `Tab` then `shift+Tab` returns to that button; in the roll, `ctrl+Right` moves the
note a snap later where a bare Right would move the cursor to the next note, and `shift+Right` /
`shift+Left` lengthen and shorten it (`native-gate.sh`'s main sequence). Alt uses the same path
and is not yet proven.

**No focus-wide injector, ever.** AT-SPI's own key synthesis (`Atspi.generate_keyboard_event`) is
not offered: it reached nothing in a Wayland-native window, and under XWayland it is XTest, which
on this desktop (Xwayland started with `-enable-ei-portal`) was seen to leave the application and
reach another client. `ydotool`, `wtype` and uinput are never used.

**The accessibility bus must say a screen reader is on.** AccessKit builds no tree until
`org.a11y.Status.ScreenReaderEnabled` is true on the session bus — `IsEnabled` is not enough. The
desktop's own setting is never flipped (the desktop may react by starting a screen reader).
Instead the script runs the application and the probe inside a private session bus:

- `GSETTINGS_BACKEND=memory` is set *outside* `dbus-run-session`. Setting ScreenReaderEnabled
  makes the private bus launcher store it in `org.gnome.desktop.a11y.applications`, and the
  launcher inherits the private bus daemon's environment, not an inner shell's; with the real
  backend the desktop would read `screen-reader-enabled=true` at the next login. Check with
  `gsettings get org.gnome.desktop.a11y.applications screen-reader-enabled`.
- The registry daemon is started by hand: the private bus cannot activate it (the launcher asks
  systemd, which a private session bus does not have).
- The whole run has its own `XDG_RUNTIME_DIR`, a short directory under `/tmp`: the private
  accessibility bus's socket, the application's single-instance socket, its log and its saved
  window state all live there and are removed with it. The path is short on purpose — a long
  runtime directory overflows the socket address and the application refuses to start.
- The private bus runs with `DISPLAY=:99`, a display that does not exist: given the real one, its
  launcher would write its bus address onto the desktop's X root window. The application and the
  probe use the desktop's real `DISPLAY`.

**It refuses to run outside that isolation, twice.** `run-probe.sh` has set the screen-reader
flag, started a registry and launched the application before Python runs, so a check only in the
probe would refuse after the damage. The launcher's inner entry therefore checks, before its first
bus call or child process, and exits 2 with one line on stderr when any fails:

- the settings backend is the in-memory one (`GSETTINGS_BACKEND=memory`);
- its parent process is `dbus-run-session`;
- its session bus is set, is not the desktop's (the address the outer half recorded), and is not
  a per-user bus under `/run/user/`;
- its runtime directory is a `/tmp/midi-evil-probe.*` directory, the one the outer half made,
  owned by the person running it, holding the random token the outer half wrote.

The probe checks again, before it sends anything: the settings backend; the runtime directory's
name, that it is the outer half's, and its token; the session bus, as above; and — a clause only
the probe has — that the accessibility bus is set and not under `/run/user/`. It does not repeat
the launcher's parent check (its own parent is the inner shell) or the ownership check.

**It makes no sound.** The application runs with `ALSA_CONFIG_PATH` naming an empty file in the
private runtime directory: ALSA then defines no device, so a row that starts playback or a take
cannot sound on the desktop, and no MIDI port of the person's is opened.

**It cannot touch a live MIDI Evil.** A second instance hands its file to the first and exits —
but only through the socket in its runtime directory, and this run's is private, so it can neither
join an instance the person has open nor be joined by one they start. On exit it stops the
application and the registry it started and removes the directory. When the probe fails, or the
application dies during the run, the application's log and the session's are printed before they
are removed — a panic is the first thing a gate needs to see.

## `web-probe.mjs` and `web-gate.sh` — the web harness in Chromium and Firefox

The web half of W0 (plan § W0-web) runs the spike's piano roll in a browser: a scratch crate,
`spike/roll-web/` on the `spike/w0-accessibility` branch, that includes the spike's
`piano_roll.rs`, `theme.rs`, `description.rs` and `transport_readout.rs` by `#[path]` and runs them
on `eframe::WebRunner`. Its page carries:

- the canvas: `role="application"`, named "MIDI Evil editor", described by the next element;
- `#midi-evil-current`: the description of whatever egui has focused, in the strings the native
  accessibility nodes carry;
- `#midi-evil-live`: a polite, atomic live region that receives each change;
- `#midi-evil-text-agent`: eframe's own hidden text entry, adopted. While egui edits text — a value
  box reached by Tab is enough — eframe puts the page's focus there, not on the canvas, so the
  entry is named for the control being edited and described by `#midi-evil-current`, as the canvas
  is. It is in the accessibility tree only while egui is editing text and out of the Tab order
  always, and it refuses any focus that arrives while it is hidden (eframe's load-time autofocus
  among them).

Build it in the spike worktree:

```sh
spike/roll-web/build.sh      # release wasm + this repository's pinned wasm-bindgen, into dist/
```

`build.sh` uses the CLI under the main checkout's `target/tools/bin/` (found through
`git worktree list`) and refuses one whose version is not the lock's, or a lock with no
wasm-bindgen entry; never the `wasm-bindgen` on `PATH`, which is the engine's pin. It builds with
an explicit `--target-dir`, binds into a staging directory, and swaps that with `dist/` in one
`renameat2(RENAME_EXCHANGE)` only when everything succeeded, so each *request* a person serving `dist/` makes
gets the old bundle or the new one; a page load is several requests, so reload after a rebuild.
`web-gate.sh` serves its own copy, taken when it starts. The old bundle is kept as
`dist.previous/`.

The page takes these in its query string: `?fixture=empty` (a one-bar pattern with no notes, used
by one part), `?fixture=full` (the same bar with a note on each beat, used by two), `?edge=value`
(a value box as egui's first stop, so leaving the canvas backwards from the text entry can be
tested), `?agent=eframe` (eframe's text entry left as eframe made it — unnamed, a Tab stop at the
end of the page, and autofocused at load — to show what the adoption fixes). A failed start is
written into `#midi-evil-current`.

`web-gate.sh` builds nothing: it serves the spike worktree's `spike/roll-web/dist/`
(`MIDI_EVIL_WEB_BUNDLE` names another) on `127.0.0.1`, runs every sequence of the web gate through
`web-probe.mjs` in headless Chromium and then headless Firefox, each in a fresh browser, then two
negative controls per browser, then the browser-key table, stops the server however it exits, and
exits 0 only if every sequence held and every negative control failed. Run from the repository
root:

```sh
coordination/web/probes/web-gate.sh > review/web/w0/web-gate-transcript.txt
node coordination/web/probes/web-probe.mjs --url http://127.0.0.1:PORT/ --browser firefox \
    --key Tab --key Tab --expect "Piano roll" --key ArrowRight --expect "C 5"
```

Playwright is the site's: the probe resolves it from the working set's
`insiculous_web/node_modules` (`PLAYWRIGHT_FROM` names another `package.json`), with Playwright's
own Chromium and Firefox under `~/.cache/ms-playwright`. It needs Node 24; `web-gate.sh` finds one
under `~/.nvm` when the `node` on `PATH` is not (`NODE` overrides). Both browsers run headless,
always; Chromium's WebGL comes from SwiftShader (`--use-angle=swiftshader
--enable-unsafe-swiftshader`), Firefox's from its software path (`webgl.force-enabled`).

After every step the probe prints where the page's focus is (with that element's `aria-label`,
and whether it is inside `aria-hidden`), egui's focus (the canvas's `data-egui-focus`), the
cursor's place in walk order, the current description, every write the live region received during
the step, Playwright's `ariaSnapshot` of `#editor-neighbourhood` — the link before the canvas, the
canvas, the text entry while it is exposed, the current element, the live region, the link after —
and, in Chromium, the canvas's node and the focused node as the browser computes them
(`Accessibility.getFullAXTree`: role, name, description, focus). Firefox has no such protocol;
there, as in Chromium, the expectations use Playwright's own computation. The steps and
expectations are listed at the top of `web-probe.mjs`.

**What the probe holds, and why.**

- **`--expect` cannot pass where nothing can be read.** It checks the current element's text and
  then the element that has the page's focus: that element must have an accessible name and its
  accessible description must contain the text. The element must be the canvas unless the same
  step names it with `--expect-focus` (an id, or `role:NAME`). The gate's first transcript passed
  with the page's focus on eframe's hidden, unnamed text entry because the old `--expect` only
  checked the canvas when the canvas happened to be focused; two negative controls per browser now
  show the probe failing on that case — `--expect` at the value box with no `--expect-focus`, and
  `?agent=eframe`'s unnamed entry named by its role. A control passes only when the probe
  exits 1 *and* prints the message it exists to show; a step that outlives its deadline exits 3,
  so a hang never stands in for a failed expectation. `--expect-name` asserts the focused
  element's accessible name itself — `--expect` only requires that there be one.
- **Semantics first.** Every page load checks the canvas's role (application) and name, its
  `aria-describedby`, and the live region's `aria-live="polite"` and `aria-atomic="true"`; in
  Chromium also that the browser's tree has a node that is live polite and atomic.
- **Announcements are mutation records.** A `MutationObserver` on the live region records every
  record it is handed, not one read of the text per callback, so a write replaced within one task
  is still counted and a write per key in a burst is counted per key. No-break spaces (the bridge's
  mark on a repeated announcement) are normalised before matching and shown as "repeat-marked".
- **Keys are Playwright's, delivered to the page.** They pass the browser's own shortcut handling,
  so a chord the browser keeps (Ctrl+N, T, W, Alt+Left, F6 among them) is received here and may
  not be in a real browser; those rows of the key table say *needs a person*. The key table puts
  focus on the roll before each chord and says so in the row if it could not.
- **A held key, and its cadence.** `--hold` runs in the page: one `requestAnimationFrame` loop that
  dispatches one `keydown` per frame on the canvas — the first plain, the rest `repeat: true` —
  and, at the start of each next frame, records the cursor's place, the current description and
  the live-region writes since the last frame. The run then requires the cursor to have moved one
  note per frame and the last write of each frame to be the cursor's note. At the 60 Hz the
  headless browsers run, that is one keydown every 16.7 ms — faster than a typical desktop key
  repeat of 25 to 33 a second — and eframe answers each by the next frame. `--burst` dispatches the keydowns in one
  task, so they land in one egui frame. Both are untrusted DOM events, which eframe's listener does
  not distinguish.
- **Deadlines.** Every call into the browser has one (fifteen seconds; thirty for a load), and the
  run has one (`WEB_PROBE_DEADLINE_SECONDS`, default 240): a page that stops answering is a failed
  step, the browser is closed and the exit status is not 0. `web-gate.sh` runs each probe under
  `timeout` (`WEB_PROBE_TIMEOUT_SECONDS`, default 300; SIGTERM, which Playwright answers by closing
  its browser, then SIGKILL fifteen seconds later).
- **The server.** `python3`'s `http.server`, bound by the server itself on a free port of
  `127.0.0.1` (no port is chosen and released first), with `.wasm` served as `application/wasm`
  whatever the machine's MIME table says. The gate starts nothing until the server is alive and
  answers with this bundle — the page's `midi-evil-harness` sentinel and the wasm's content type —
  and stops at once if it does not.
- **The page opens with nothing focused, and Tab leaves the canvas at egui's ends.** eframe
  prevents the default of every Tab; the harness stops a Tab at the window, before eframe sees it,
  when egui's focus is on its last stop (Shift+Tab: its first), so the browser moves focus on —
  from the text entry it first puts focus on the canvas, so the browser leaves from the canvas's
  place in either direction. egui keeps its own focus while the page's is elsewhere, and the first
  stop the canvas gives is the roll. egui drops focus on Escape and on Enter in a text edit; the
  harness gives it back to the same control in the next pass, which is what announces a committed
  value.
