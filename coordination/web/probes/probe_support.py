"""What atspi-probe.py stands on: its error, its isolation check, its event pump, and the
sender that delivers keys to the application's own X window with XSendEvent and nowhere else.
"""

import ctypes
import ctypes.util
import math
import os
import re
import stat
import subprocess
import time

from gi.repository import GLib

POLL_SECONDS = 0.25
KEY_ALIASES = {"enter": "Return", "space": "space", "+": "plus"}
MODIFIER_ALIASES = {"shift": "shift", "control": "control", "ctrl": "control", "alt": "alt"}


class ProbeError(Exception):
    """A failure reported as one line on stderr and exit status 1."""


def verify_isolation():
    """The conditions run-probe.sh's inner entry checks, checked again before anything is sent.

    The probe runs inside the private session bus that run-probe.sh made, in the private runtime
    directory it marked, with the in-memory settings backend. Outside those, a key could reach
    the desktop and a setting could reach the person's own; the probe stops instead.
    """
    environment = os.environ
    runtime = environment.get("XDG_RUNTIME_DIR", "")
    token = environment.get("MIDI_EVIL_PROBE_TOKEN", "")
    try:
        with open(os.path.join(runtime, "isolation-token"), encoding="utf-8") as marker:
            marked_token = marker.read().strip()
    except OSError:
        marked_token = None
    session_bus = environment.get("DBUS_SESSION_BUS_ADDRESS", "")
    desktop_bus = environment.get("MIDI_EVIL_DESKTOP_BUS_ADDRESS")
    accessibility_bus = environment.get("AT_SPI_BUS_ADDRESS", "")
    if environment.get("GSETTINGS_BACKEND") != "memory":
        refusal = "the settings backend is not the in-memory one"
    elif (not runtime.startswith("/tmp/midi-evil-probe.")
          or runtime != environment.get("MIDI_EVIL_PRIVATE_RUNTIME")
          or not token or marked_token != token):
        refusal = "the runtime directory is not run-probe.sh's private one"
    elif (not session_bus or desktop_bus is None or session_bus == desktop_bus
          or "/run/user/" in session_bus):
        refusal = "the session bus is not run-probe.sh's private one"
    elif not accessibility_bus or "/run/user/" in accessibility_bus:
        refusal = "the accessibility bus is not the private session's"
    else:
        return
    raise ProbeError(f"refuses to run: {refusal}; nothing was sent")


def pump_events(seconds):
    # libatspi delivers events through the default GLib main context, so a listener only
    # fires while that context is iterated.
    context = GLib.MainContext.default()
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        while context.pending():
            context.iteration(False)
        time.sleep(0.01)


def poll(wait_seconds, attempt):
    """Call attempt until it returns something other than None or the wait runs out."""
    deadline = time.monotonic() + wait_seconds
    while True:
        result = attempt()
        if result is not None or time.monotonic() >= deadline:
            return result
        pump_events(POLL_SECONDS)


# How long a step waits for the application to catch up. The event window after a key is half
# a second; on a busy desktop a node can appear, or a name land, a few frames after that.
SETTLE_SECONDS = 3.0


def wait_for_node(find):
    """Call find until it stops reporting that the node does not exist yet, or the wait ends."""
    deadline = time.monotonic() + SETTLE_SECONDS
    while True:
        try:
            return find()
        except ProbeError as error:
            if not str(error).startswith("no node named") or time.monotonic() >= deadline:
                raise
        pump_events(POLL_SECONDS)


def ancestor_names(node):
    """The names of the node's ancestors, nearest first."""
    names = []
    try:
        parent = node.get_parent()
        while parent is not None:
            names.append(parent.get_name() or "")
            parent = parent.get_parent()
    except GLib.Error:
        pass
    return names


class KeyEvent(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("serial", ctypes.c_ulong),
        ("send_event", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("window", ctypes.c_ulong),
        ("root", ctypes.c_ulong),
        ("subwindow", ctypes.c_ulong),
        ("time", ctypes.c_ulong),
        ("x", ctypes.c_int),
        ("y", ctypes.c_int),
        ("x_root", ctypes.c_int),
        ("y_root", ctypes.c_int),
        ("state", ctypes.c_uint),
        ("keycode", ctypes.c_uint),
        ("same_screen", ctypes.c_int),
    ]


class XEvent(ctypes.Union):
    # Xlib's XEvent is a union padded to 24 longs, and XSendEvent copies that many bytes.
    _fields_ = [("key", KeyEvent), ("padding", ctypes.c_long * 24)]


class Chord:
    """A key with its modifiers, resolved to X keycodes before anything is sent."""

    def __init__(self, text, keycode, modifiers, modifier_keycodes):
        self.text = text
        self.keycode = keycode
        self.modifiers = modifiers
        self.modifier_keycodes = modifier_keycodes


class WindowKeySender:
    """Sends key presses and releases to one X window with XSendEvent.

    winit ignores a key that arrives while its window is not the X input focus, so the sender
    refuses to send when the window has lost focus rather than report a key that was dropped.
    """

    KEY_PRESS = 2
    KEY_RELEASE = 3
    KEY_PRESS_MASK = 1 << 0
    KEY_RELEASE_MASK = 1 << 1
    MODIFIER_MASKS = {"shift": 1 << 0, "control": 1 << 2, "alt": 1 << 3}
    MODIFIER_KEYSYMS = {"shift": "Shift_L", "control": "Control_L", "alt": "Alt_L"}

    def __init__(self):
        library_path = ctypes.util.find_library("X11")
        if library_path is None or not os.environ.get("DISPLAY"):
            raise ProbeError("keys need libX11 and DISPLAY; run the application under XWayland")
        self.xlib = ctypes.cdll.LoadLibrary(library_path)
        self.xlib.XOpenDisplay.restype = ctypes.c_void_p
        self.xlib.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        self.xlib.XDefaultRootWindow.restype = ctypes.c_ulong
        self.xlib.XStringToKeysym.restype = ctypes.c_ulong
        self.xlib.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        self.xlib.XkbKeycodeToKeysym.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ubyte,
            ctypes.c_int,
            ctypes.c_int,
        ]
        self.xlib.XkbKeycodeToKeysym.restype = ctypes.c_ulong
        self.xlib.XFetchName.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.POINTER(ctypes.c_char_p),
        ]
        self.xlib.XQueryTree.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)),
            ctypes.POINTER(ctypes.c_uint),
        ]
        self.xlib.XGetInputFocus.argtypes = [
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_int),
        ]
        self.xlib.XSendEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.c_int,
            ctypes.c_long,
            ctypes.c_void_p,
        ]
        self.xlib.XFlush.argtypes = [ctypes.c_void_p]
        self.display = self.xlib.XOpenDisplay(None)
        if not self.display:
            raise ProbeError("cannot open the X display")
        self.root = self.xlib.XDefaultRootWindow(self.display)
        self.window = None

    def keycode(self, keysym_name):
        keysym = self.xlib.XStringToKeysym(keysym_name.encode())
        if keysym == 0:
            return 0
        return self.xlib.XKeysymToKeycode(self.display, keysym)

    def needs_shift(self, keysym_name, keycode):
        """True when the keysym is on its key's shifted level, as + is on the = key."""
        keysym = self.xlib.XStringToKeysym(keysym_name.encode())
        unshifted = self.xlib.XkbKeycodeToKeysym(self.display, keycode, 0, 0)
        shifted = self.xlib.XkbKeycodeToKeysym(self.display, keycode, 0, 1)
        return unshifted != keysym and shifted == keysym

    def parse(self, text):
        """A chord such as Tab, shift+Tab, ctrl+Left, Enter or shift++; ProbeError if not."""
        if text == "+":
            modifier_names, key_name = [], "+"
        elif text.endswith("++"):
            modifier_names, key_name = text[:-2].split("+"), "+"
        else:
            *modifier_names, key_name = text.split("+")
        if not key_name:
            raise ProbeError(f"bad chord {text!r}: no key after the modifiers")
        modifiers = []
        for modifier_name in modifier_names:
            modifier = MODIFIER_ALIASES.get(modifier_name.lower())
            if modifier is None:
                raise ProbeError(f"bad chord {text!r}: unknown modifier {modifier_name!r}")
            modifiers.append(modifier)
        keysym_name = KEY_ALIASES.get(key_name.lower(), key_name)
        keycode = self.keycode(keysym_name)
        if keycode == 0:
            raise ProbeError(f"bad chord {text!r}: {key_name!r} is not an X keysym on this keyboard")
        if "shift" not in modifiers and self.needs_shift(keysym_name, keycode):
            modifiers.append("shift")
        modifier_keycodes = [self.keycode(self.MODIFIER_KEYSYMS[name]) for name in modifiers]
        return Chord(text, keycode, modifiers, modifier_keycodes)

    def query_tree(self, window):
        """The window's parent and children."""
        root_return = ctypes.c_ulong()
        parent_return = ctypes.c_ulong()
        child_list = ctypes.POINTER(ctypes.c_ulong)()
        child_count = ctypes.c_uint()
        if not self.xlib.XQueryTree(
            self.display,
            window,
            ctypes.byref(root_return),
            ctypes.byref(parent_return),
            ctypes.byref(child_list),
            ctypes.byref(child_count),
        ):
            return 0, []
        return parent_return.value, [child_list[index] for index in range(child_count.value)]

    def find_window(self, window, title):
        name = ctypes.c_char_p()
        if self.xlib.XFetchName(self.display, window, ctypes.byref(name)) and name.value == title:
            return window
        for child in self.query_tree(window)[1]:
            found = self.find_window(child, title)
            if found is not None:
                return found
        return None

    def attach(self, window_title, wait_seconds):
        self.window = poll(wait_seconds, lambda: self.find_window(self.root, window_title.encode()))
        if self.window is None:
            raise ProbeError(f"no X window titled {window_title!r}; is it under XWayland?")

    def focused_window(self):
        window = ctypes.c_ulong()
        revert_to = ctypes.c_int()
        self.xlib.XGetInputFocus(self.display, ctypes.byref(window), ctypes.byref(revert_to))
        return window.value

    def application_has_focus(self):
        window = self.focused_window()
        # 0 is None and 1 is PointerRoot: neither is inside the application's window.
        while window not in (0, 1, self.root):
            if window == self.window:
                return True
            window = self.query_tree(window)[0]
        return False

    def wait_for_focus(self, wait_seconds):
        if not poll(wait_seconds, lambda: True if self.application_has_focus() else None):
            raise ProbeError(
                f"the application's window does not have the X input focus "
                f"(0x{self.focused_window():x} does); nothing was sent"
            )

    def send_one(self, event_type, keycode, state):
        mask = self.KEY_PRESS_MASK if event_type == self.KEY_PRESS else self.KEY_RELEASE_MASK
        event = XEvent()
        event.key.type = event_type
        event.key.display = self.display
        event.key.window = self.window
        event.key.root = self.root
        event.key.state = state
        event.key.keycode = keycode
        event.key.same_screen = 1
        # No propagation: an event the window does not take must go nowhere, never to an ancestor.
        if not self.xlib.XSendEvent(self.display, self.window, 0, mask, ctypes.byref(event)):
            raise ProbeError(f"XSendEvent to window {self.window:#x} failed; nothing was sent")

    def press(self, chord):
        """The modifiers' presses, then the key's: the start of a held chord."""
        state = 0
        for name, keycode in zip(chord.modifiers, chord.modifier_keycodes):
            self.send_one(self.KEY_PRESS, keycode, state)
            state |= self.MODIFIER_MASKS[name]
        self.send_one(self.KEY_PRESS, chord.keycode, state)
        self.xlib.XFlush(self.display)

    def repeat(self, chord):
        """Another press of a key that was never released, which winit reports as a repeat."""
        self.send_one(self.KEY_PRESS, chord.keycode, self.chord_state(chord))
        self.xlib.XFlush(self.display)

    def release(self, chord):
        state = self.chord_state(chord)
        self.send_one(self.KEY_RELEASE, chord.keycode, state)
        for name, keycode in reversed(list(zip(chord.modifiers, chord.modifier_keycodes))):
            self.send_one(self.KEY_RELEASE, keycode, state)
            state &= ~self.MODIFIER_MASKS[name]
        self.xlib.XFlush(self.display)

    def chord_state(self, chord):
        state = 0
        for name in chord.modifiers:
            state |= self.MODIFIER_MASKS[name]
        return state

    def send(self, chord):
        # The modifier keys' own presses go first, as from a real keyboard, and every event
        # carries the state the chord holds at that moment: winit 0.30 takes its modifiers from
        # each core key event's state field (update_mods_from_core_event).
        state = 0
        for name, keycode in zip(chord.modifiers, chord.modifier_keycodes):
            self.send_one(self.KEY_PRESS, keycode, state)
            state |= self.MODIFIER_MASKS[name]
        self.send_one(self.KEY_PRESS, chord.keycode, state)
        self.send_one(self.KEY_RELEASE, chord.keycode, state)
        for name, keycode in reversed(list(zip(chord.modifiers, chord.modifier_keycodes))):
            self.send_one(self.KEY_RELEASE, keycode, state)
            state &= ~self.MODIFIER_MASKS[name]
        self.xlib.XFlush(self.display)


def whole_number(text, what):
    try:
        number = int(text)
    except ValueError:
        raise ProbeError(f"{what}: {text!r} is not a whole number") from None
    if number < 0:
        raise ProbeError(f"{what}: {text!r} is negative")
    return number


def check_step_arguments(steps):
    """Every new step's argument is checked before anything runs, as the chords are."""
    for kind, value in steps:
        if kind == "hold":
            chord, _, repeats = value.rpartition(":")
            if not chord:
                raise ProbeError(f"--hold {value!r}: give CHORD:REPEATS")
            whole_number(repeats, f"--hold {value!r}")
        elif kind == "press_in":
            region, separator, name = value.partition("|")
            if not (region and separator and name):
                raise ProbeError(f"--press-in {value!r}: give REGION|NAME")
        elif kind == "burst":
            if not value.split():
                raise ProbeError(f"--burst {value!r}: give at least one chord")
        elif kind == "set_value":
            name, _, number = value.rpartition(":")
            try:
                float(number)
            except ValueError:
                raise ProbeError(f"--set-value {value!r}: give NAME:NUMBER") from None
            if not name:
                raise ProbeError(f"--set-value {value!r}: give NAME:NUMBER")
        elif kind in ("pause", "expect_quiet"):
            try:
                seconds = float(value)
            except ValueError:
                seconds = math.nan
            if not math.isfinite(seconds) or seconds <= 0:
                raise ProbeError(f"--{kind.replace('_', '-')} {value!r}: give a finite, positive "
                                 f"number of seconds")
        elif kind in ("key_until", "expect_node_description"):
            first, separator, second = value.partition("|")
            # A description's text may be empty, to say it has none; a key's target may not.
            if not (first and separator) or (kind == "key_until" and not second):
                raise ProbeError(f"--{kind.replace('_', '-')} {value!r}: give FIRST|SECOND")
        elif kind == "open_in_running":
            if not os.path.isfile(value):
                raise ProbeError(f"--open-in-running {value!r}: no such file")
            for variable in ("MIDI_EVIL_BINARY", "XDG_RUNTIME_DIR"):
                if not os.environ.get(variable):
                    raise ProbeError(f"--open-in-running {value!r}: {variable} is not set; "
                                     f"run the probe through run-probe.sh")
        elif kind == "expect_unfocused_name_change":
            if not value:
                raise ProbeError("--expect-unfocused-name-change: give a non-empty prefix")
        elif kind in ("expect_hold_name_changes", "expect_release_name_changes"):
            whole_number(value, f"--{kind.replace('_', '-')} {value!r}")
        elif kind == "expect_announcement_count":
            text, _, count = value.rpartition(":")
            if not text:
                raise ProbeError(f"--expect-announcement-count {value!r}: give TEXT:COUNT")
            whole_number(count, f"--expect-announcement-count {value!r}")


def running_instance_socket():
    """The socket the application under test listens on for a second launch, in the private
    runtime directory: the path instance.rs makes, with the user's name kept to the characters
    it keeps."""
    user = re.sub(r"[^A-Za-z0-9_-]", "_", os.environ.get("USER", "default"))
    return os.path.join(os.environ.get("XDG_RUNTIME_DIR", ""), f"midi-evil-{user}.sock")


def second_launch(path, environment):
    """The second launch's command and environment: no display at all, X or Wayland, no audio
    device, and the file's absolute path, since the running instance resolves it from its own
    working directory."""
    launch_environment = dict(environment, ALSA_CONFIG_PATH=os.path.join(
        environment["XDG_RUNTIME_DIR"], "silent-asound.conf"))
    for display in ("DISPLAY", "WAYLAND_DISPLAY"):
        launch_environment.pop(display, None)
    return [launch_environment["MIDI_EVIL_BINARY"], os.path.abspath(path)], launch_environment


def open_in_running(path, wait_seconds):
    """Launches the application a second time with a file, as a person opening a project from a
    file manager or a terminal would; the second launch hands the file to the instance under
    test through its socket and exits. It is refused unless that socket is there, in the
    private runtime directory verify_isolation has checked. Should the hand-over fail anyway,
    the second launch can reach nothing: it is given no display at all, X or Wayland, and no
    audio device.
    """
    socket = running_instance_socket()
    try:
        is_socket = stat.S_ISSOCK(os.stat(socket).st_mode)
    except OSError:
        is_socket = False
    if not is_socket:
        raise ProbeError(f"--open-in-running: no running instance's socket at {socket}")
    command, environment = second_launch(path, os.environ)
    try:
        finished = subprocess.run(command, env=environment,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
                                  timeout=wait_seconds)
    except subprocess.TimeoutExpired:
        raise ProbeError(f"--open-in-running {path!r}: the second launch did not hand the file "
                         f"over and exit within {wait_seconds:g} seconds") from None
    if finished.returncode != 0:
        raise ProbeError(f"--open-in-running {path!r}: the second launch exited "
                         f"{finished.returncode}: {finished.stderr.strip()}")
