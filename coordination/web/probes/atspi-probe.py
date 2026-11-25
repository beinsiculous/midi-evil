#!/usr/bin/env python3
"""Read MIDI Evil's accessibility tree over AT-SPI, deliver keys to it, and print what changed.

Keys are sent with XSendEvent to the application's own X window, so the application must run
under XWayland (WAYLAND_DISPLAY unset). Nothing is ever typed into whichever window has focus:
AT-SPI's own key synthesis is XTest underneath, and on this desktop it was not confined to the
application, so the probe does not offer it. run-probe.sh beside this file starts the
application and runs the probe; README.md says what the session needs.
"""

import argparse
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

from probe_support import (  # noqa: E402
    SETTLE_SECONDS,
    ProbeError,
    ancestor_names,
    check_step_arguments,
    wait_for_node,
    WindowKeySender,
    poll,
    pump_events,
    verify_isolation,
)

LISTENED_EVENT_TYPES = (
    "object:announcement",
    "object:property-change",
    "object:state-changed:focused",
    "object:text-changed",
    "object:children-changed",
    "object:active-descendant-changed",
)
EVENT_WINDOW_SECONDS = 0.5


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
    if accessible is None:
        return []
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
    if accessible is None:
        return False
    try:
        return accessible.get_state_set().contains(state)
    except GLib.Error:
        return False


def name_of(accessible):
    try:
        return accessible.get_name() or ""
    except GLib.Error:
        return None


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
        # Name changes of whichever node was focused when the change arrived: the roll's name
        # is what a screen reader speaks, and the status line, which repeats it, is never
        # focused.
        self.focused_name_changes = []
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
        if (event.type == "object:property-change:accessible-name"
                and has_state(event.source, Atspi.StateType.FOCUSED)):
            self.focused_name_changes.append(str(payload))
        self.lines.append(
            f"{event.type} detail1={event.detail1} detail2={event.detail2} "
            f"payload={payload!r} source: {describe_if_present(event.source)}"
        )

    def clear(self):
        self.lines = []
        self.announcements = []
        self.focused_name_changes = []

    def print_and_clear(self):
        for line in self.lines:
            print("  event " + line)
        self.last_step_announcements = self.announcements
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


def set_value_after_label(application, text, recorder):
    """Sets the value box that belongs to the label with this name: the first node with a value
    among the label's following siblings, before the next caption. egui lays a value box out
    after its caption, and the inspector's boxes are not named yet. AccessKit turns the call into
    the widget's SetValue action, as a screen reader's value change would be."""
    label, _, value = text.rpartition(":")
    caption = next((node for _path, node in walk(application) if name_of(node) == label), None)
    if caption is None:
        raise ProbeError(f"no label named {label!r}")
    try:
        parent = caption.get_parent()
        if parent is None:
            raise ProbeError(f"the label {label!r} has no parent to find its value box in")
        siblings = children(parent)
        position = caption.get_index_in_parent()
        target = None
        for sibling in siblings[position + 1:]:
            if sibling.get_value_iface() is not None:
                target = sibling
                break
            if sibling.get_role_name() == "label":
                break
        if target is None:
            raise ProbeError(f"no value box belongs to the label {label!r}")
        if not Atspi.Value.set_current_value(target.get_value_iface(), float(value)):
            raise ProbeError(f"set {label!r} to {value}: the value was refused")
    except GLib.Error as error:
        raise ProbeError(f"set {label!r} to {value}: {error.message}") from error
    pump_events(EVENT_WINDOW_SECONDS)
    print(f"set the value after {label!r} to {value}")
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
        self.hold_name_changes = None
        self.release_name_changes = None

    def focused_name(self):
        node = deepest_focused(self.application)
        return None if node is None else name_of(node)

    def before_step(self):
        self.name_before_step = self.focused_name()
        # A count belongs to the hold that made it: a step that is not a hold has none.
        self.hold_name_changes = None
        self.release_name_changes = None

    def holds(self, kind, text):
        if kind == "expect":
            name = self.focused_name()
            return name is not None and text in name
        return any(text in (name_of(node) or "") for _path, node in walk(self.application))

    def check(self, kind, text):
        if kind in ("expect", "expect_node"):
            # Something that should be there is waited for; something that should not is not.
            poll(SETTLE_SECONDS, lambda: self.holds(kind, text) or None)
        name = self.focused_name()
        if kind == "expect":
            passed = self.holds(kind, text)
            wanted = f"the focused name contains {text!r}"
        elif kind == "expect_not":
            # Nothing focused is not a pass: the check exists to show focus went somewhere else.
            passed = name is not None and text not in name
            wanted = f"a node is focused and its name does not contain {text!r}"
        elif kind == "expect_same_name":
            passed = name is not None and name == self.name_before_step
            wanted = f"the focused name is unchanged ({self.name_before_step!r})"
        elif kind == "expect_node":
            passed = self.holds(kind, text)
            wanted = f"some node's name contains {text!r}"
        elif kind == "expect_region":
            node = deepest_focused(self.application)
            regions = [] if node is None else ancestor_names(node)
            passed = text in regions
            wanted = f"the focused node is inside the region {text!r} (inside: {regions!r})"
        elif kind in ("expect_enabled", "expect_disabled"):
            nodes = [node for _path, node in walk(self.application) if name_of(node) == text]
            if len(nodes) != 1:
                raise ProbeError(f"{len(nodes)} nodes named {text!r}; the state needs exactly one")
            enabled = has_state(nodes[0], Atspi.StateType.ENABLED) \
                and has_state(nodes[0], Atspi.StateType.SENSITIVE)
            passed = enabled == (kind == "expect_enabled")
            wanted = f"{text!r} is {'enabled' if kind == 'expect_enabled' else 'disabled'}"
        elif kind == "expect_description":
            node = deepest_focused(self.application)
            try:
                description = "" if node is None else (node.get_description() or "")
            except GLib.Error as error:
                raise ProbeError(f"reading the focused node's description: {error.message}") \
                    from error
            passed = text in description
            wanted = f"the focused node's description contains {text!r}"
        elif kind == "expect_hold_name_changes":
            passed = self.hold_name_changes is not None and len(self.hold_name_changes) == int(text)
            wanted = (f"{text} focused name change(s) while the key repeated "
                      f"(seen: {self.hold_name_changes!r})")
        elif kind == "expect_release_name_changes":
            passed = (self.release_name_changes is not None
                      and len(self.release_name_changes) == int(text))
            wanted = (f"{text} focused name change(s) after the release "
                      f"(seen: {self.release_name_changes!r})")
        elif kind == "expect_announcement_count":
            announcement, _, count = text.rpartition(":")
            heard = [item for item in self.recorder.last_step_announcements if announcement in item]
            passed = len(heard) == int(count)
            wanted = (f"{count} announcement(s) containing {announcement!r} "
                      f"(heard: {self.recorder.last_step_announcements!r})")
        else:
            announcements = self.recorder.last_step_announcements
            passed = any(text in announcement for announcement in announcements)
            wanted = f"an announcement contains {text!r} (heard: {announcements!r})"
        if not passed:
            raise ProbeError(f"expected {wanted}; the focused name is {name!r}")
        print(f"  ok: {wanted}")


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
        elif kind == "press_in":
            region, _, name = value.partition("|")
            expectations.before_step()
            perform_action(application, name, recorder, region=region)
        elif kind == "focus":
            expectations.before_step()
            give_focus(application, value, recorder)
        elif kind == "set_value":
            expectations.before_step()
            set_value_after_label(application, value, recorder)
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
    steps.add_argument("--set-value", action=StepAction, metavar="LABEL:VALUE",
                       help="set the value box that follows the label LABEL to VALUE")
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
