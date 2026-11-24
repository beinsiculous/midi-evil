#!/usr/bin/env python3
"""Read MIDI Evil's accessibility tree over AT-SPI, deliver keys to it, and print what changed.

Keys are sent with XSendEvent to the application's own X window, so the application must run
under XWayland (WAYLAND_DISPLAY unset). Nothing is ever typed into whichever window has focus:
AT-SPI's own key synthesis is XTest underneath, and on this desktop it was not confined to the
application, so the probe does not offer it. run-probe.sh beside this file starts the
application and runs the probe; README.md says what the session needs.
"""

import argparse
import ctypes
import ctypes.util
import os
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

LISTENED_EVENT_TYPES = (
    "object:announcement",
    "object:property-change",
    "object:state-changed:focused",
    "object:text-changed",
    "object:children-changed",
    "object:active-descendant-changed",
)
EVENT_WINDOW_SECONDS = 0.5
POLL_SECONDS = 0.25
KEY_ALIASES = {"enter": "Return", "space": "space", "+": "plus"}
MODIFIER_ALIASES = {"shift": "shift", "control": "control", "ctrl": "control", "alt": "alt"}


class ProbeError(Exception):
    """A failure reported as one line on stderr and exit status 1."""


def describe(accessible):
    """One line: role, name, description, value, text, focus states, actions and extents."""
    parts = [accessible.get_role_name(), repr(accessible.get_name() or "")]
    description = accessible.get_description() or ""
    if description:
        parts.append(f"description={description!r}")
    value_interface = accessible.get_value_iface()
    if value_interface is not None:
        parts.append(f"value={Atspi.Value.get_current_value(value_interface)}")
    text_interface = accessible.get_text_iface()
    if text_interface is not None:
        character_count = Atspi.Text.get_character_count(text_interface)
        text = Atspi.Text.get_text(text_interface, 0, character_count)
        parts.append(f"text={text!r}")
    states = accessible.get_state_set()
    flags = []
    if states.contains(Atspi.StateType.FOCUSABLE):
        flags.append("focusable")
    if states.contains(Atspi.StateType.FOCUSED):
        flags.append("FOCUSED")
    if not states.contains(Atspi.StateType.ENABLED):
        flags.append("disabled")
    if flags:
        parts.append("[" + " ".join(flags) + "]")
    action_interface = accessible.get_action_iface()
    if action_interface is not None:
        action_names = [
            Atspi.Action.get_localized_name(action_interface, action_index)
            for action_index in range(Atspi.Action.get_n_actions(action_interface))
        ]
        if action_names:
            parts.append("actions=" + ",".join(action_names))
    component_interface = accessible.get_component_iface()
    if component_interface is not None:
        extents = Atspi.Component.get_extents(component_interface, Atspi.CoordType.WINDOW)
        parts.append(f"at {extents.x},{extents.y} {extents.width}x{extents.height}")
    return " ".join(parts)


def describe_if_present(accessible):
    # An event can name a node that the application removed before the event was read.
    if accessible is None:
        return "(none)"
    try:
        return describe(accessible)
    except GLib.Error:
        return "(removed)"


def children(accessible):
    """The node's children that still exist; a node can vanish between two calls."""
    try:
        child_count = accessible.get_child_count()
    except GLib.Error:
        return []
    found = []
    for child_index in range(child_count):
        try:
            child = accessible.get_child_at_index(child_index)
        except GLib.Error:
            continue
        if child is not None:
            found.append(child)
    return found


def walk(accessible, path=()):
    """Yield (path, node) depth first; the path is the chain of child indices from the root."""
    yield path, accessible
    for child_index, child in enumerate(children(accessible)):
        yield from walk(child, path + (child_index,))


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


def find_application(application_name, wait_seconds):
    """The desktop child with this name; AccessKit names it after the executable."""

    def attempt():
        desktop = Atspi.get_desktop(0)
        if desktop is None:
            return None
        for application in children(desktop):
            try:
                if application.get_name() == application_name:
                    return application
            except GLib.Error:
                continue
        return None

    return poll(wait_seconds, attempt)


def has_state(accessible, state):
    try:
        return accessible.get_state_set().contains(state)
    except GLib.Error:
        return False


def name_of(accessible):
    try:
        return accessible.get_name() or ""
    except GLib.Error:
        return None


def find_by_name(application, name, interface_getter, interface_label, prefix_must_be_unique):
    """The node with this name that has the interface; says how many matched and why not.

    A usable node with exactly this name wins; failing that, the usable nodes whose name begins
    with it, because some names carry state ("Piano roll, C 4, bar 1 beat 1, ...") and change with
    every key. A prefix that several usable nodes share is refused where acting on the wrong one
    would go unnoticed.
    """
    named = [(node, name_of(node) or "") for _path, node in walk(application)]
    exact = [node for node, node_name in named if node_name == name]
    beginning = [node for node, node_name in named if node_name.startswith(name)]
    if not beginning:
        raise ProbeError(f"no node named {name!r} or with a name beginning with it")
    usable = [node for node in exact if interface_getter(node) is not None]
    if not usable:
        usable = [node for node in beginning if interface_getter(node) is not None]
        if not usable:
            raise ProbeError(f"{len(beginning)} node(s) named {name!r} or beginning with it, "
                             f"none with {interface_label}")
        if prefix_must_be_unique and len(usable) > 1:
            raise ProbeError(f"{len(usable)} nodes with {interface_label} have a name beginning "
                             f"{name!r}; give the whole name")
    if len(usable) > 1:
        print(f"{len(usable)} nodes named {name!r} with {interface_label}; "
              f"using the first: {describe_if_present(usable[0])}")
    return usable[0]


def focused_nodes(application):
    """Every focused node with its path, shallowest first."""
    return [
        (path, node) for path, node in walk(application) if has_state(node, Atspi.StateType.FOCUSED)
    ]


def deepest_focused(application):
    """The deepest focused node, or None."""
    focused = focused_nodes(application)
    if not focused:
        return None
    return max(focused, key=lambda pair: len(pair[0]))[1]


def print_focus(application, heading):
    """Print the deepest focused node; print all of them when they are not one ancestor chain."""
    focused = focused_nodes(application)
    if not focused:
        print(f"{heading}: (nothing focused)")
        return
    paths = [path for path, _node in focused]
    one_chain = all(
        deeper[: len(shallower)] == shallower for shallower, deeper in zip(paths, paths[1:])
    )
    deepest = max(focused, key=lambda pair: len(pair[0]))[1]
    print(f"{heading}: {describe_if_present(deepest)}")
    if not one_chain:
        print(f"  {len(focused)} focused nodes outside one ancestor chain:")
        for _path, node in focused:
            print(f"    {describe_if_present(node)}")


class EventRecorder:
    def __init__(self, application):
        self.process_id = application.get_process_id()
        self.lines = []
        self.announcements = []
        self.last_step_announcements = []
        self.listener = Atspi.EventListener.new(self.record)
        for event_type in LISTENED_EVENT_TYPES:
            self.listener.register(event_type)

    def is_from_application(self, event):
        try:
            return event.source is not None and event.source.get_process_id() == self.process_id
        except GLib.Error:
            return False

    def record(self, event):
        if not self.is_from_application(event):
            return
        payload = event.any_data
        if isinstance(payload, Atspi.Accessible):
            payload = "accessible " + describe_if_present(payload)
        if event.type == "object:announcement":
            self.announcements.append(str(payload))
        self.lines.append(
            f"{event.type} detail1={event.detail1} detail2={event.detail2} "
            f"payload={payload!r} source: {describe_if_present(event.source)}"
        )

    def clear(self):
        self.lines = []
        self.announcements = []

    def print_and_clear(self):
        for line in self.lines:
            print("  event " + line)
        self.last_step_announcements = self.announcements
        self.clear()


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


def perform_action(application, name, recorder):
    target = find_by_name(application, name, lambda node: node.get_action_iface(), "an action",
                          prefix_must_be_unique=True)
    if not Atspi.Action.do_action(target.get_action_iface(), 0):
        raise ProbeError(f"press {name!r}: the action was refused")
    pump_events(EVENT_WINDOW_SECONDS)
    print(f"pressed {name!r}")
    recorder.print_and_clear()


def focusable_component(node):
    if not has_state(node, Atspi.StateType.FOCUSABLE):
        return None
    return node.get_component_iface()


def give_focus(application, name, recorder):
    target = find_by_name(application, name, focusable_component, "a focusable component",
                          prefix_must_be_unique=False)
    if not Atspi.Component.grab_focus(target.get_component_iface()):
        raise ProbeError(f"focus {name!r}: grab_focus was refused")
    pump_events(EVENT_WINDOW_SECONDS)
    print_focus(application, f"focus {name!r}")
    recorder.print_and_clear()


class Expectations:
    """Checks on what the previous step left: the focused node's name and the announcements."""

    def __init__(self, application, recorder):
        self.application = application
        self.recorder = recorder
        self.name_before_step = None

    def focused_name(self):
        node = deepest_focused(self.application)
        return None if node is None else name_of(node)

    def before_step(self):
        self.name_before_step = self.focused_name()

    def check(self, kind, text):
        name = self.focused_name()
        if kind == "expect":
            passed = name is not None and text in name
            wanted = f"the focused name contains {text!r}"
        elif kind == "expect_not":
            # Nothing focused is not a pass: the check exists to show focus went somewhere else.
            passed = name is not None and text not in name
            wanted = f"a node is focused and its name does not contain {text!r}"
        elif kind == "expect_same_name":
            passed = name is not None and name == self.name_before_step
            wanted = f"the focused name is unchanged ({self.name_before_step!r})"
        elif kind == "expect_node":
            passed = any(text in (name_of(node) or "") for _path, node in walk(self.application))
            wanted = f"some node's name contains {text!r}"
        else:
            announcements = self.recorder.last_step_announcements
            passed = any(text in announcement for announcement in announcements)
            wanted = f"an announcement contains {text!r} (heard: {announcements!r})"
        if not passed:
            raise ProbeError(f"expected {wanted}; the focused name is {name!r}")
        print(f"  ok: {wanted}")


def run(arguments):
    keys = [value for kind, value in arguments.steps if kind == "key"]
    sender = WindowKeySender() if keys else None
    # Every chord is checked before the first is sent.
    chords = {text: sender.parse(text) for text in keys}

    Atspi.init()
    application = find_application(arguments.application, arguments.wait)
    if application is None:
        raise ProbeError(f"no AT-SPI application named {arguments.application!r} "
                         f"within {arguments.wait} seconds")
    if sender is not None:
        sender.attach(arguments.window_title, arguments.wait)
    recorder = EventRecorder(application)
    expectations = Expectations(application, recorder)

    for kind, value in arguments.steps:
        if kind == "heading":
            print(f"== {value}")
        elif kind == "press":
            expectations.before_step()
            perform_action(application, value, recorder)
        elif kind == "focus":
            expectations.before_step()
            give_focus(application, value, recorder)
        elif kind == "dump":
            for path, accessible in walk(application):
                print("  " * len(path) + describe_if_present(accessible))
        elif kind == "key":
            chord = chords[value]
            print_focus(application, f"before {chord.text}")
            sender.wait_for_focus(arguments.wait)
            expectations.before_step()
            sender.send(chord)
            pump_events(EVENT_WINDOW_SECONDS)
            print_focus(application, f"after {chord.text}")
            recorder.print_and_clear()
        else:
            expectations.check(kind, value)

    if arguments.listen:
        pump_events(arguments.listen)
        recorder.print_and_clear()


class StepAction(argparse.Action):
    """Keeps every step option in one list, in the order given on the command line."""

    def __call__(self, parser, namespace, values, option_string=None):
        steps = getattr(namespace, "steps", None) or []
        steps.append((self.dest, True if values is None else values))
        namespace.steps = steps


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--application", default="midi-evil",
                        help="the AT-SPI application name, which is the executable's name")
    parser.add_argument("--window-title", default="MIDI Evil",
                        help="the exact X window title that keys are sent to")
    parser.add_argument("--wait", type=float, default=15.0,
                        help="seconds to wait for the application, its window and its focus")
    parser.add_argument("--listen", type=float, default=0.0,
                        help="seconds to keep printing events after the last step")
    parser.set_defaults(steps=[])
    steps = parser.add_argument_group(
        "steps", "run in the order given; every --expect checks what the last --press, --focus or "
                 "--key left")
    steps.add_argument("--heading", action=StepAction, metavar="TEXT",
                       help="print a heading line into the transcript")
    steps.add_argument("--press", action=StepAction, metavar="NAME",
                       help="perform the first action of the node with this name "
                            "(or, failing an exact match, a name beginning with it)")
    steps.add_argument("--focus", action=StepAction, metavar="NAME",
                       help="give focus to the node with this name through AT-SPI")
    steps.add_argument("--dump", action=StepAction, nargs=0, help="print the whole tree")
    steps.add_argument("--key", action=StepAction, metavar="CHORD",
                       help="deliver a chord such as Tab, shift+Tab, ctrl+Left, F6 or Enter")
    steps.add_argument("--expect", action=StepAction, metavar="TEXT",
                       help="fail unless the focused node's name contains TEXT")
    steps.add_argument("--expect-not", action=StepAction, metavar="TEXT",
                       help="fail if nothing is focused or the focused node's name contains TEXT")
    steps.add_argument("--expect-node", action=StepAction, metavar="TEXT",
                       help="fail unless some node, focused or not, has a name containing TEXT")
    steps.add_argument("--expect-same-name", action=StepAction, nargs=0,
                       help="fail unless the focused node's name is what it was before the step")
    steps.add_argument("--expect-announcement", action=StepAction, metavar="TEXT",
                       help="fail unless an object:announcement during the step contains TEXT")
    try:
        run(parser.parse_args())
    except ProbeError as error:
        print(f"atspi-probe: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
