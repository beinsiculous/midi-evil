"""The checks atspi-probe.py runs on what a step left: the focused node's name and description,
the announcements and name changes the step raised, and the tree as a whole.
"""

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

from probe_support import SETTLE_SECONDS, ProbeError, ancestor_names, poll  # noqa: E402
from probe_tree import deepest_focused, describe_if_present, has_state, name_of, walk  # noqa: E402

LISTENED_EVENT_TYPES = (
    "object:announcement",
    "object:property-change",
    "object:state-changed:focused",
    "object:text-changed",
    "object:children-changed",
    "object:active-descendant-changed",
    "object:value-changed",
)
# What a screen reader would present about the focused node changing: its name, its
# description, its value.
FOCUSED_CHANGE_TYPES = (
    "object:property-change:accessible-name",
    "object:property-change:accessible-description",
    "object:property-change:accessible-value",
    "object:value-changed",
)


def is_listened(event_type):
    """Whether a registration covers the event: AT-SPI delivers an event to a listener
    registered for its type or for any prefix of it, so "object:property-change" hears the
    description's and the value's changes."""
    return any(event_type == registered or event_type.startswith(registered + ":")
               for registered in LISTENED_EVENT_TYPES)


def is_focused_change(event_type, focused):
    """Whether the event is a change a screen reader would present about the focused node."""
    return focused and event_type in FOCUSED_CHANGE_TYPES


def quiet_window_holds(name_before, name_after, focused_changes, announcements):
    """A quiet window: the focused name is what it was, and nothing about the focused node
    changed and nothing was announced while it lasted."""
    return (name_after is not None and name_after == name_before and not focused_changes
            and not announcements)


# Roles a person can reach that egui draws for itself: a scroll bar is not a Tab stop, but a
# screen reader's review lands on it.
ALWAYS_NAMED_ROLES = ("scroll bar",)


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

    def unnamed_nodes(self):
        """Every focusable node, and every node of a role that must be named, with no name."""
        unnamed = []
        for _path, node in walk(self.application):
            try:
                reachable = (has_state(node, Atspi.StateType.FOCUSABLE)
                             or node.get_role_name() in ALWAYS_NAMED_ROLES)
            except GLib.Error:
                continue
            if reachable and not name_of(node):
                unnamed.append(describe_if_present(node))
        return unnamed

    def node_description(self, name):
        nodes = [node for _path, node in walk(self.application) if name_of(node) == name]
        if len(nodes) != 1:
            raise ProbeError(f"{len(nodes)} nodes named {name!r}; the description needs exactly one")
        try:
            return nodes[0].get_description() or ""
        except GLib.Error as error:
            raise ProbeError(f"reading {name!r}'s description: {error.message}") from error

    def event_arrived(self, event_type):
        return event_type in self.recorder.last_step_types + self.recorder.types

    def node_description_holds(self, text):
        node_name, _, wanted_text = text.partition("|")
        try:
            description = self.node_description(node_name)
        except ProbeError:
            return False
        return wanted_text in description if wanted_text else description == ""

    def check(self, kind, text):
        if kind in ("expect", "expect_node"):
            # Something that should be there is waited for; something that should not is not.
            poll(SETTLE_SECONDS, lambda: self.holds(kind, text) or None)
        if kind == "expect_changed_name":
            poll(SETTLE_SECONDS, lambda: self.focused_name() != self.name_before_step or None)
        if kind == "expect_event":
            poll(SETTLE_SECONDS, lambda: self.event_arrived(text) or None)
        if kind == "expect_all_named":
            poll(SETTLE_SECONDS, lambda: not self.unnamed_nodes() or None)
        if kind == "expect_node_description":
            # A panel or a reason can land a frame or two after the step that caused it.
            poll(SETTLE_SECONDS, lambda: self.node_description_holds(text) or None)
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
        elif kind == "expect_changed_name":
            passed = name is not None and name != self.name_before_step
            wanted = f"the focused name changed from {self.name_before_step!r}"
        elif kind == "expect_quiet":
            changes = self.recorder.last_step_focused_changes
            announcements = self.recorder.last_step_announcements
            passed = quiet_window_holds(self.name_before_step, name, changes, announcements)
            wanted = (f"for {text} seconds no announcement ({announcements!r}) and no change to "
                      f"the focused node's name, description or value ({changes!r})")
        elif kind == "expect_unfocused_name_change":
            names = [item for item in self.recorder.last_step_unfocused_names
                     if item.startswith(text)]
            passed = bool(names)
            wanted = f"a node without focus changed its name to one beginning {text!r}: {names!r}"
        elif kind == "expect_event":
            passed = self.event_arrived(text)
            wanted = f"an event {text!r} arrived during the step or since"
        elif kind == "expect_all_named":
            unnamed = self.unnamed_nodes()
            passed = not unnamed
            wanted = f"every focusable node and scroll bar has a name (unnamed: {unnamed!r})"
        elif kind == "expect_node_description":
            node_name, _, wanted_text = text.partition("|")
            description = self.node_description(node_name)
            passed = wanted_text in description if wanted_text else description == ""
            wanted = (f"{node_name!r}'s description " + (f"contains {wanted_text!r}"
                      if wanted_text else "is empty") + f" (it is {description!r})")
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
