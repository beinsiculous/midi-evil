# Probes

Mechanical evidence for the screen-reader gates (plan D4): transcripts an agent can produce and
re-run. They are half of each gate; the other half is a person listening with Orca.

## `atspi-probe.py` and `run-probe.sh` — the native application over AT-SPI

`run-probe.sh` starts MIDI Evil, runs `atspi-probe.py` against it with the script's own
arguments, stops everything it started, and exits with the probe's status. `probe_support.py`
beside them holds what the probe stands on: its isolation check, its event pump, and the sender
that delivers keys to the application's own X window; `probe_tree.py` reads the tree (one line
per node, the walk, where focus is); `probe_expectations.py` holds the checks. The probe finds the
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

## `fixtures/` — the gates' projects

One set of tracked `.evil` projects that both gates start from. The native gate opens each by
launch argument; the browser gate's own bundle compiles them in and opens one by `?fixture=`. The
native gate saves into `saved.evil`, so neither gate opens the set in place: `native-gate.sh`
copies it into a private temporary directory at every run, and a run leaves the tracked files
unchanged. `bach.evil` is the tracked Bach invention imported from MIDI, kept as a project because
a browser cannot import a file; `the_tracked_bach_project_is_the_midi_file_imported_note_for_note`
holds it to the import, and W0's main sequence goes on importing the MIDI file itself. The rest:

- `twins` — one track, two identical notes; `bass` — four bass notes; `two-tracks` — one note
  each; `three-notes` — three, the second overlapping the first; `same-pitch-overlap` — two
  overlapping notes at one pitch; `long` — sixty-four one-beat notes, half a minute at 120 beats
  a minute; `tempo` — one note at 90 beats a minute; `lane-point` — two notes and one pitch-bend
  point; `clean` and `saved` — one note each, nothing unsaved; `arrangement` — two tracks, the
  first playing one shared pattern in bars 1 and 3, the second a unique timeline of one note;
  `bach-at-the-limit` — the Bach invention played again from its start after its end and cut at
  two and a half minutes, five track-minutes, the most the browser renders
  (`the_bach_invention_at_the_limit_is_the_tracked_project_played_again_and_cut_there` holds it
  to `bach.evil`); `chords-at-the-limit` — four tracks of four-note chords, one a beat, thirty
  bars at 96 beats a minute, the same five track-minutes; `chords-over-the-limit` — the same a
  bar longer, which the browser refuses to play.

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
- `--set-value NAME:VALUE` sets the value box named NAME (`Velocity`, `Tempo in beats per
  minute`) through AT-SPI's Value interface, which AccessKit turns into the widget's SetValue
  action, as a screen reader's value change would be. Focus does not move.
- `--open-in-running FILE` launches the application a second time with FILE, as opening a project
  from a file manager or a terminal does; the second launch hands the file to the running
  instance through the single-instance socket in the private runtime directory and exits. A
  project opened at launch says so before any listener is on the bus, so this is how a row hears
  a project load; the step waits, up to three seconds, until that load is heard. The probe
  refuses unless that socket is there, and the second launch runs with no display at all — no
  `DISPLAY`, no `WAYLAND_DISPLAY` — and the empty ALSA configuration, so should the hand-over fail
  it can open no window and no device.
- `--pause SECONDS` waits, printing the application's events; `--key-until "CHORD|TEXT"` sends
  CHORD every half second until the focused name contains TEXT (bounded by `--wait`) — Enter on
  the transport readout until a render has finished and it says playing.
- `--screen-reader-after-launch` (not a step: it applies to the whole run) has `run-probe.sh`
  leave `ScreenReaderEnabled` off; the probe shows the application absent from the accessibility
  bus for three seconds, turns the flag on — on the private bus it has just verified — and only
  then looks for the application, as a screen reader started after the application would.
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
- `--expect-quiet SECONDS` waits and fails on any announcement, or on any change to the focused
  node's name, description or value, raised while it waited — the readout's five silent seconds.
  `--expect-changed-name` fails unless the focused name differs from before the step;
  `--expect-unfocused-name-change PREFIX` unless, during the step, a node without focus changed
  its name to one beginning PREFIX. `--expect-all-named` fails if any focusable node or any
  scroll bar has no name, and lists them. `--expect-node-description "NAME|TEXT"` fails unless
  the one node named NAME has a description containing TEXT, or, with TEXT empty, has none —
  which is how a disabled button's reason is checked, since its disabled state is not exposed.
  Like `--expect`, it waits up to three seconds for what it expects to arrive.
  `--expect-event TYPE` fails unless an event of exactly TYPE arrived during the last step or
  since. `--pause` and `--expect-quiet` set aside, printed apart, whatever arrived before they
  opened, so each counts only its own window; `--key-until` that runs out of time fails.
  `probe_self_check.py` checks without an application that what `--expect-quiet` counts is what
  the probe listens for (`object:property-change` covers the description's and the value's
  changes), and that a description change on the focused node fails a quiet window.
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

Runs the native gate of W-A1 (plan § W-A1a, § W-A1b and § W-A1c) and prints the
transcript: first the
refusal rows — the inner entry with the desktop's own environment; one row per clause of its
check, each with every other clause satisfied so only that clause can refuse; a control with all
of them satisfied, which must get past the check; and the probe's own check called without the
launcher — each requiring the desktop's screen-reader flag and settings files to be unchanged;
then W0's sequence, the transport's step included, and its five edges; then the rows found in
W0's review, R-1 to R-22, and the instrument list's; then, from
`native-gate-arranger-and-files.sh`, which it sources, the record's nine steps, the Song, track
panel and file sequences, and a row for each announced command that needs no dialog — each in a
fresh application. The ninth step, save, is two rows of its own: an edit saved with Control+S to
the file the project was opened from, and a relaunch on that file that finds the edit, both in
the gate's private fixture directory, removed on exit. Open from the menu and Save As on a new
project open the desktop's own dialog, which keys sent to the application's window cannot drive;
those two are the sitting's alone, under Jesse's written waiver. Every step a row names carries an
expectation, so exit status 0 means each key did what the row says. Every row opens a project of
`fixtures/` — the Bach project unless it names another, two identical notes, a bass part, two
tracks — from a copy of the set in a private temporary directory, removed on exit. What keys cannot show from outside (which of two
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
cannot sound on the desktop, and no MIDI port of the person's is opened. With no device the song
could not play at all, and the transport could not be read playing; `MIDI_EVIL_SILENT_PLAYBACK`,
which `run-probe.sh` sets, has a **debug** build play on a silent clock instead — the same player,
fed to a thread that takes its samples at the rate a device would and drops them — so its
position advances in real time. A release build never reads the variable
(`preview.rs`, `silent_playback_requested`). ALSA's own `null` device was tried first and does not
pace: it took two seconds of audio in three milliseconds.

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

## `web-app-probe.mjs`, its rows and `web-app-gate.sh` — the application's own bundles

W0's harness above stays as that half's evidence; this is the gate for the real application, on
its two bundles: the visitor's, built by `scripts/build_wasm.sh` into `target/web/dist/`, and the
gate's, built by `scripts/build_wasm.sh --gate` into `target/web/dist-gate/`, which compiles
`fixtures/` in and reads `?fixture=` and `?shows=desktop`. A visitor never gets the gate's bundle;
the gate refuses to start without one whose wasm carries the marker only that build compiles in
(read from `crates/evil-web/src/gate.rs`), carries the digest of the fixture set it was compiled
from — the gate computes the same over `fixtures/`, so the two gates can never open different
projects under one row — and whose page is the source tree's. Both bundles also carry the identity
of the tree they were built from (`scripts/build_identity.sh`, over everything under `crates/`, the
workspace's manifest and lock file and the fixtures), and the gate refuses to start unless the
visitor's, the gate's and the tree's are one, so neither bundle can be left from an older tree.

`web-app-gate.sh` serves copies of both on `127.0.0.1` and runs every row through
`web-app-probe.mjs` in headless Chromium and then headless Firefox — each row in a fresh page — in
six groups, each under a deadline of its own (`WEB_APP_GROUP_TIMEOUT_SECONDS`, default 300, in
each browser, and `WEB_APP_PLAYING_TIMEOUT_SECONDS`, default 600, for the playing group; the
probe refuses to start a row whose own deadline is more than half the deadline it runs under): **keys-and-bridge**
(`web-app-rows.mjs`, and the owed click row in `web-app-page-rows.mjs`: the keys, the bridge, the
focus boundary and the text entry, from the default project, on the visitor's bundle), **matrix**
(`web-app-matrix-rows.mjs`: the native gate's W0 and R rows, each from the fixture its native row
opens and first asserting it by name and track count, on the gate's bundle), **surfaces** (the
same file's sequences for the instrument list, the Song view, the roll's toolbar, the track panel
and the windows, the same way), **absence** and **page**
(`web-app-page-rows.mjs`, on the visitor's bundle), and **playing** (`web-app-playing-rows.mjs`,
Play heard, on the gate's bundle; below). Then five negative controls in each browser,
each run on the row that shows it and each of which must fail for its own reason:
`live-never-written` (the live region ignores every write), `current-stale` (the current element
keeps its first text), `binds-not-bound` (the page turns Control+O into the application's Save) —
each a copy of the visitor's bundle with one line put into its page — and `shows-desktop` (the
gate's bundle drawing what the desktop draws, on which the absence walk must reach the Mixer's
tab), and `play-never-pressed` (a copy of the gate's bundle whose page keeps Space from the
application, on which `play-by-space` must hear nothing). It exits 0 only if every row held in
both browsers and all ten controls failed as they must; it stops the server however it ends.

```sh
coordination/web/probes/web-app-gate.sh > web-app-gate-transcript.txt
node coordination/web/probes/web-app-probe.mjs --url http://127.0.0.1:PORT/bundle/index.html \
    --gate-url http://127.0.0.1:PORT/gate/index.html --browser firefox --group matrix
```

**The page's order.** The heading; the notes' button, a stop a visitor meets before the editor,
whose notes — the keyboard notes, what is desktop-only and the browsers it was tested in — are
static text in `crates/evil-web/index.html`, closed when the page loads; the notes when open, a
named region that scrolls within itself; the sentinel; the canvas; the current element; the live
region; the sentinel; "Leave the editor". Every row that enters from the page passes the notes'
button first.

**What a row holds.** Every check of where the page's focus is reads the element that has it: the
canvas, or eframe's text entry while a field is edited, which must be named and described by what
the row expects. In Chromium that is the browser's real tree over CDP
(`Accessibility.getPartialAXTree` on `document.activeElement`), asserted for role, name and
description; in Firefox it is Playwright's `ariaSnapshot` for the role and name, with the
description read from `aria-describedby`, and every Firefox row says so — its real tree is the
sitting's. The live region's writes are mutation records, counted per step; a no-break space at
the end of one is the page's repeat mark and is ignored when matching. For every key the probe
records whether it reached the canvas or the text entry, where eframe listens, and whether its
default was cancelled once the whole dispatch had run.

**What a row cannot show.** Keys are Playwright's, delivered to the page past the browser's own
shortcut handling, so what a browser does with a chord it keeps — its file dialog for Control+O,
going back for Alt+Left, printing for Control+P — never happens here. The rows that hand a chord
to the browser show that eframe never received it, that it was not cancelled, and that the
application did nothing and wrote nothing; the rest is the sitting's, and the row says so. The
page's answer to `beforeunload` is read by dispatching the event and reading whether it was
cancelled, in the same task as an edit where the row says so.

**Fresh pages, and a Mac's system.** A row that needs a clean document opens a fresh page in a
context of its own rather than navigating one with unsaved work: the page's own leave-page prompt
would otherwise decide the step (with no dialog listener Playwright accepts Chromium's prompt for
the script; headless Firefox raises none for a navigation no person started). A row may run as a
Mac by its user agent, from which eframe tells egui the system; `mac-text-editing` does, so the
text-entry chords egui takes on a Mac — Command+A, Command+Z, Option+Left, Option+Backspace — are
shown reaching the field. A row stopped by its deadline is a failed row, and the probe sets its
exit status rather than exiting, so no line a control looks for is lost.

## `web-app-audio-tap.mjs` and `web-app-playing-rows.mjs` — what the gate hears

`installAudioTap` is installed before the page's own scripts in every row of `web-app-probe.mjs`,
whatever its group, since a row that plays anything must not be heard: headless Firefox plays
through the machine's real audio server. It wraps `AudioContext` to count constructions, sends
everything connected **into** a context's destination to a collector instead, and passes that on
at gain zero; an `AudioWorklet` beside the collector records every sample against the context's
own clock, and the page keeps the last forty seconds. The probe steers it through
`window.midiEvilAudio`: `constructorThrows` makes the constructor throw, as a browser with no audio
does; `holdSuspended()` suspends every context and keeps it suspended, answering the page's
`resume()` and ignoring it, until `release()`; `block(milliseconds)` holds the page's thread, as a
long render does; and its readers give the loudest sample heard between two moments, how long the
worklet listened, the samples themselves, and the longest wait between two animation frames —
how long the page answered nothing. The onset detector, `onsets`, is in the same file, in
JavaScript beside the rows rather than Python called from the gate: the peak of every 2.5 ms held
over 12.5 ms, and an onset wherever that rises above 1.15 times the median of the 200 ms before it
and above 0.02, at least 250 ms after the last. It is good to about 12 ms.

The **playing** group runs on the gate's bundle, each row from a fixture, and reads the transport
in the arranger, where Space plays and stops and I writes the transport. Before any row, the
probe shows on a bare page of the same origin that the tap hears an oscillator it connects and
nothing without one, and that the browser's context starts at all: headless Firefox needs the
machine's audio server, and without one its context never starts, which **refuses** the group —
the tooling's failure, exit status 2 — rather than failing it. A row marked `untouched`
(`keys-alone`, `click-alone`) reads the page through its console alone until its own key or click
is past, because `page.evaluate` is itself a gesture to both browsers: the tap and the bridge's
two elements report there. A real click is the browser's own pointer, moved to Play's stated place
at the gate's 1440 by 900 window and pressed; the row passes on what the tap then hears, so a click
that misses fails. `frame-that-handles-play` prints `MEASURED:` lines — the longest the page
answered nothing after Play, for `bach`, `bach-at-the-limit` and `chords-at-the-limit` — and fails
when any is over five seconds, the bound the browser's render limit is set by.
