#!/usr/bin/env python3
"""Read MIDI Evil's accessibility tree over AT-SPI, deliver keys to it, and print what changed.

Keys are sent with XSendEvent to the application's own X window, so the application must run
under XWayland (WAYLAND_DISPLAY unset). Nothing is ever typed into whichever window has focus:
AT-SPI's own key synthesis is XTest underneath, and on this desktop it was not confined to the
application, so the probe does not offer it. run-probe.sh beside this file starts the
application and runs the probe; README.md says what the session needs.
"""

import argparse
import subprocess
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

from probe_tree import (  # noqa: E402
    children,
    describe_if_present,
    has_state,
    name_of,
    print_focus,
    walk,
)
from probe_expectations import (  # noqa: E402
    LISTENED_EVENT_TYPES,
    Expectations,
    is_focused_change,
)
from probe_support import (  # noqa: E402
    ProbeError,
    ancestor_names,
    check_step_arguments,
    wait_for_node,
    SETTLE_SECONDS,
    WindowKeySender,
    open_in_running,
    poll,
    pump_events,
    verify_isolation,
)

EVENT_WINDOW_SECONDS = 0.5


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


def find_by_name(application, name, interface_getter, interface_label, prefix_must_be_unique,
                 region=None):
    """The node with this name that has the interface; says how many matched and why not.

    A usable node with exactly this name wins; failing that, the usable nodes whose name begins
    with it, because some names carry state ("Piano roll, C 4, bar 1 beat 1, ...") and change with
    every key. Where acting on the wrong node would go unnoticed, several candidates — exact or
    by prefix — are refused; a region (a named ancestor) narrows them.
    """
    named = [(node, name_of(node) or "") for _path, node in walk(application)
             if region is None or region in ancestor_names(node)]
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
        if prefix_must_be_unique:
            raise ProbeError(f"{len(usable)} nodes named {name!r} have {interface_label}; "
                             f"name the region it is in with --press-in REGION|NAME")
        print(f"{len(usable)} nodes named {name!r} with {interface_label}; "
              f"using the first: {describe_if_present(usable[0])}")
    return usable[0]


class EventRecorder:
    def __init__(self, application):
        self.process_id = application.get_process_id()
        self.lines = []
        self.announcements = []
        self.last_step_announcements = []
        # Name changes of whichever node was focused when the change arrived: the roll's name
        # is what a screen reader speaks, and the status line, which repeats it, is never
        # focused.
        self.focused_name_changes = []
        # Every change to the focused node's name, description or value, and the names that
        # nodes without focus changed to.
        self.focused_changes = []
        self.unfocused_names = []
        self.types = []
        self.last_step_types = []
        self.last_step_focused_changes = []
        self.last_step_unfocused_names = []
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
        focused = has_state(event.source, Atspi.StateType.FOCUSED)
        if event.type == "object:property-change:accessible-name":
            (self.focused_name_changes if focused else self.unfocused_names).append(str(payload))
        if is_focused_change(event.type, focused):
            self.focused_changes.append(f"{event.type} {payload!r}")
        self.types.append(event.type)
        self.lines.append(
            f"{event.type} detail1={event.detail1} detail2={event.detail2} "
            f"payload={payload!r} source: {describe_if_present(event.source)}"
        )

    def clear(self):
        self.lines = []
        self.types = []
        self.announcements = []
        self.focused_name_changes = []
        self.focused_changes = []
        self.unfocused_names = []

    def set_aside(self):
        """Prints what arrived since the last step ended, apart, and forgets it, so a window
        that opens next counts only its own events."""
        for line in self.lines:
            print("  event (before the next step) " + line)
        self.clear()

    def print_and_clear(self):
        for line in self.lines:
            print("  event " + line)
        self.last_step_announcements = self.announcements
        self.last_step_focused_changes = self.focused_changes
        self.last_step_unfocused_names = self.unfocused_names
        self.last_step_types = self.types
        self.clear()

    def take_focused_name_changes(self):
        changes, self.focused_name_changes = self.focused_name_changes, []
        return changes


def perform_action(application, name, recorder, region=None):
    target = wait_for_node(lambda: find_by_name(
        application, name, lambda node: node.get_action_iface(), "an action",
        prefix_must_be_unique=True, region=region))
    if not Atspi.Action.do_action(target.get_action_iface(), 0):
        raise ProbeError(f"press {name!r}: the action was refused")
    pump_events(EVENT_WINDOW_SECONDS)
    print(f"pressed {name!r}")
    recorder.print_and_clear()


def set_value(application, text, recorder):
    """Sets the value box with this name through AT-SPI's Value interface, which AccessKit turns
    into the widget's SetValue action, as a screen reader's value change would be. Focus does
    not move."""
    name, _, value = text.rpartition(":")
    target = wait_for_node(lambda: find_by_name(
        application, name, lambda node: node.get_value_iface(), "a value",
        prefix_must_be_unique=True))
    try:
        if not Atspi.Value.set_current_value(target.get_value_iface(), float(value)):
            raise ProbeError(f"set {name!r} to {value}: the value was refused")
    except GLib.Error as error:
        raise ProbeError(f"set {name!r} to {value}: {error.message}") from error
    pump_events(EVENT_WINDOW_SECONDS)
    print(f"set {name!r} to {value}")
    recorder.print_and_clear()


def key_until(application, sender, chord, text, recorder, expectations, wait_seconds):
    """Sends the chord, then again every half second, until the focused name contains the text:
    Enter on the transport readout until a render has finished and it says playing."""
    expectations.before_step()
    deadline = time.monotonic() + wait_seconds
    while True:
        sender.wait_for_focus(wait_seconds)
        sender.send(chord)
        pump_events(EVENT_WINDOW_SECONDS)
        if text in (expectations.focused_name() or ""):
            break
        if time.monotonic() >= deadline:
            recorder.print_and_clear()
            raise ProbeError(f"{chord.text} for {wait_seconds:g} seconds and the focused name "
                             f"never contained {text!r}; it is {expectations.focused_name()!r}")
    print_focus(application, f"after {chord.text} until {text!r}")
    recorder.print_and_clear()


def enable_screen_reader_after_launch(application_name):
    """Shows the application is absent from the accessibility bus while no screen reader is on,
    then turns ScreenReaderEnabled on — on the private session bus that verify_isolation has
    just checked — as a screen reader started after the application would."""
    absent_for = 3.0
    if find_application(application_name, absent_for) is not None:
        raise ProbeError(f"{application_name!r} built its tree before a screen reader was on")
    print(f"no {application_name!r} on the accessibility bus for {absent_for:g} seconds "
          f"with ScreenReaderEnabled off")
    command = ["gdbus", "call", "--session", "--dest", "org.a11y.Bus",
               "--object-path", "/org/a11y/bus",
               "--method", "org.freedesktop.DBus.Properties.Set",
               "org.a11y.Status", "ScreenReaderEnabled", "<true>"]
    try:
        subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                       text=True, timeout=10)
    except subprocess.TimeoutExpired:
        raise ProbeError("turning ScreenReaderEnabled on: gdbus did not answer within 10 "
                         "seconds") from None
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or str(error)
        raise ProbeError(f"turning ScreenReaderEnabled on failed: {detail.strip()}") from None
    print("ScreenReaderEnabled turned on after launch")


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


def hold_key(application, sender, chord, repeats, recorder, expectations, wait_seconds):
    """Presses a chord, repeats it without a release, then releases it, and records the focused
    name changes during the repeats and after the release apart. Repeats come every 40 ms, a
    desktop's key repeat rate, each in a frame of its own."""
    print_focus(application, f"before holding {chord.text}")
    sender.wait_for_focus(wait_seconds)
    expectations.before_step()
    sender.press(chord)
    pump_events(EVENT_WINDOW_SECONDS)
    print_focus(application, f"after pressing {chord.text}")
    recorder.take_focused_name_changes()
    for _ in range(repeats):
        sender.wait_for_focus(wait_seconds)
        sender.repeat(chord)
        pump_events(0.04)
    pump_events(EVENT_WINDOW_SECONDS)
    print_focus(application, f"after {repeats} repeats of {chord.text}, still held")
    expectations.hold_name_changes = recorder.take_focused_name_changes()
    sender.release(chord)
    pump_events(EVENT_WINDOW_SECONDS)
    print_focus(application, f"after releasing {chord.text}")
    expectations.release_name_changes = recorder.take_focused_name_changes()
    print(f"  name changes while held: {expectations.hold_name_changes!r}")
    print(f"  name changes after the release: {expectations.release_name_changes!r}")
    recorder.print_and_clear()


def run(arguments):
    check_step_arguments(arguments.steps)
    verify_isolation()
    keys = [value for kind, value in arguments.steps if kind == "key"]
    keys += [value.rpartition(":")[0] for kind, value in arguments.steps if kind == "hold"]
    keys += [chord for kind, value in arguments.steps if kind == "burst" for chord in value.split()]
    keys += [value.partition("|")[0] for kind, value in arguments.steps if kind == "key_until"]
    sender = WindowKeySender() if keys else None
    # Every chord is checked before the first is sent.
    chords = {text: sender.parse(text) for text in keys}

    Atspi.init()
    if arguments.screen_reader_after_launch:
        enable_screen_reader_after_launch(arguments.application)
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
        elif kind == "press_in":
            region, _, name = value.partition("|")
            expectations.before_step()
            perform_action(application, name, recorder, region=region)
        elif kind == "focus":
            expectations.before_step()
            give_focus(application, value, recorder)
        elif kind == "set_value":
            expectations.before_step()
            set_value(application, value, recorder)
        elif kind in ("pause", "expect_quiet"):
            # A quiet period is a step of its own: what arrived before it opened is set aside,
            # and what it raised is checked once it is over.
            recorder.set_aside()
            expectations.before_step()
            pump_events(float(value))
            print_focus(application, f"after {value} seconds")
            recorder.print_and_clear()
            if kind == "expect_quiet":
                expectations.check(kind, value)
        elif kind == "key_until":
            chord_text, _, text = value.partition("|")
            key_until(application, sender, chords[chord_text], text, recorder, expectations,
                      arguments.wait)
        elif kind == "open_in_running":
            expectations.before_step()
            open_in_running(value, arguments.wait)
            # The hand-over is heard when the running instance opens the file, at its next frame;
            # wait for that announcement, so this step's own events hold it.
            poll(SETTLE_SECONDS, lambda: recorder.announcements or None)
            pump_events(EVENT_WINDOW_SECONDS)
            print_focus(application, f"after a second launch handed over {value!r}")
            recorder.print_and_clear()
        elif kind == "dump":
            for path, accessible in walk(application):
                print("  " * len(path) + describe_if_present(accessible))
        elif kind == "hold":
            chord_text, _, repeats = value.rpartition(":")
            hold_key(application, sender, chords[chord_text], int(repeats), recorder,
                     expectations, arguments.wait)
        elif kind == "burst":
            # Back to back, with nothing pumped between, so the application reads them in one
            # frame: the order of keys within a frame is what the burst exists to show.
            print_focus(application, f"before the burst {value!r}")
            sender.wait_for_focus(arguments.wait)
            expectations.before_step()
            for chord_text in value.split():
                sender.send(chords[chord_text])
            pump_events(EVENT_WINDOW_SECONDS)
            print_focus(application, f"after the burst {value!r}")
            recorder.print_and_clear()
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
    parser.add_argument("--screen-reader-after-launch", action="store_true",
                        help="show the application absent with ScreenReaderEnabled off, then "
                             "turn it on (run-probe.sh leaves it off when this is given)")
    parser.set_defaults(steps=[])
    steps = parser.add_argument_group(
        "steps", "run in the order given; every --expect checks what the last --press, --focus or "
                 "--key left")
    steps.add_argument("--heading", action=StepAction, metavar="TEXT",
                       help="print a heading line into the transcript")
    steps.add_argument("--press", action=StepAction, metavar="NAME",
                       help="perform the first action of the node with this name "
                            "(or, failing an exact match, a name beginning with it)")
    steps.add_argument("--press-in", action=StepAction, metavar="REGION|NAME",
                       help="press the node named NAME inside the node (a region) named REGION")
    steps.add_argument("--expect-enabled", action=StepAction, metavar="NAME",
                       help="fail unless the one node named NAME is enabled and sensitive")
    steps.add_argument("--expect-disabled", action=StepAction, metavar="NAME",
                       help="fail unless the one node named NAME is disabled")
    steps.add_argument("--focus", action=StepAction, metavar="NAME",
                       help="give focus to the node with this name through AT-SPI")
    steps.add_argument("--set-value", action=StepAction, metavar="NAME:VALUE",
                       help="set the value box named NAME to VALUE, without moving focus")
    steps.add_argument("--open-in-running", action=StepAction, metavar="FILE",
                       help="launch the application again with FILE, which hands it to the "
                            "instance under test")
    steps.add_argument("--pause", action=StepAction, metavar="SECONDS",
                       help="wait, recording the application's events")
    steps.add_argument("--key-until", action=StepAction, metavar="CHORD|TEXT",
                       help="send CHORD every half second until the focused name contains TEXT")
    steps.add_argument("--expect-quiet", action=StepAction, metavar="SECONDS",
                       help="wait; fail on any announcement or any change to the focused node's "
                            "name, description or value")
    steps.add_argument("--expect-changed-name", action=StepAction, nargs=0,
                       help="fail unless the focused name differs from before the step")
    steps.add_argument("--expect-unfocused-name-change", action=StepAction, metavar="PREFIX",
                       help="fail unless, during the step, a node without focus changed its name "
                            "to one beginning PREFIX")
    steps.add_argument("--expect-event", action=StepAction, metavar="TYPE",
                       help="fail unless an event of TYPE arrived during the last step or since")
    steps.add_argument("--expect-all-named", action=StepAction, nargs=0,
                       help="fail if any focusable node or scroll bar has no name")
    steps.add_argument("--expect-node-description", action=StepAction, metavar="NAME|TEXT",
                       help="fail unless the one node named NAME has a description containing "
                            "TEXT; with TEXT empty, unless it has none")
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
    steps.add_argument("--burst", action=StepAction, metavar="CHORDS",
                       help="deliver several space-separated chords back to back, in one frame")
    steps.add_argument("--hold", action=StepAction, metavar="CHORD:REPEATS",
                       help="press a chord, repeat it REPEATS times without a release, release it")
    steps.add_argument("--expect-hold-name-changes", action=StepAction, metavar="COUNT",
                       help="fail unless the last --hold saw COUNT focused name changes while held")
    steps.add_argument("--expect-release-name-changes", action=StepAction, metavar="COUNT",
                       help="fail unless the last --hold saw COUNT focused name changes after "
                            "its release")
    steps.add_argument("--expect-announcement-count", action=StepAction, metavar="TEXT:COUNT",
                       help="fail unless exactly COUNT announcements of the step contain TEXT")
    steps.add_argument("--expect-region", action=StepAction, metavar="NAME",
                       help="fail unless the focused node is inside a node named NAME")
    steps.add_argument("--expect-description", action=StepAction, metavar="TEXT",
                       help="fail unless the focused node's description contains TEXT")
    try:
        run(parser.parse_args())
    except ProbeError as error:
        print(f"atspi-probe: {error}", file=sys.stderr)
        return 1
    except GLib.Error as error:
        # A node can vanish between two calls; that is a failed step, not a traceback.
        print(f"atspi-probe: the accessibility bus refused a call: {error.message}",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
