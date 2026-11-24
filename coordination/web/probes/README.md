# Probes

Mechanical evidence for the screen-reader gates (plan D4): transcripts an agent can produce and
re-run. They are half of each gate; the other half is a person listening with Orca.

## `atspi-probe.py` and `run-probe.sh` — the native application over AT-SPI

`run-probe.sh` starts MIDI Evil, runs `atspi-probe.py` against it with the script's own
arguments, stops everything it started, and exits with the probe's status. The probe finds the
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
- `--focus NAME` gives that node focus through AT-SPI (AccessKit's Focus action). A gate does not
  use it to reach the roll: the roll is reached with F6, as a person would.
- `--key CHORD` delivers a key: an X keysym name (`Tab`, `Left`, `Delete`, `Home`, `F6`, `a`) or
  one of `Enter`, `Space`, `+`, with any of `shift+`, `ctrl+`/`control+`, `alt+` in front
  (`shift+Tab`, `ctrl+Left`, `shift+F6`, `shift++`). Every chord is checked before the first is
  sent; a bad one stops the probe with one line on stderr. A keysym that sits on a key's shifted
  level (`+`, `A`) is sent with Shift added.
- The expectations check what the last `--press`, `--focus` or `--key` left; several may follow
  one step. `--expect TEXT` fails the run unless the focused node's name contains TEXT;
  `--expect-not TEXT` fails if it does, **or if nothing is focused** — it exists to show focus went
  somewhere else. `--expect-same-name` fails unless a node is focused and its name is what it was
  before that step. `--expect-node TEXT` fails unless some node, focused or not, has a name
  containing TEXT. `--expect-announcement TEXT` fails unless an `object:announcement` raised
  during that step contains TEXT. A failed expectation is one line on stderr and exit status 1.
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

Runs the plan's native gate (§ W0-native, *Gate*) through `run-probe.sh` and prints the
transcript: the main sequence, then each edge, each in a fresh application. Every step the gate
names carries an `--expect`, so exit status 0 means each key did what the gate says. The
sequences assume the Bach fixture and the debug build's keyboard model; the other half of the gate
is a person performing the same keys with Orca.

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

**It cannot touch a live MIDI Evil.** A second instance hands its file to the first and exits —
but only through the socket in its runtime directory, and this run's is private, so it can neither
join an instance the person has open nor be joined by one they start. On exit it stops the
application and the registry it started and removes the directory. When the probe fails, or the
application dies during the run, the application's log and the session's are printed before they
are removed — a panic is the first thing a gate needs to see.
