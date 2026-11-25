"""Reading the application's accessibility tree over AT-SPI: one line per node, the walk, and
where focus is. A node can vanish between two calls, so every reader here tolerates that.
"""

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402


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
