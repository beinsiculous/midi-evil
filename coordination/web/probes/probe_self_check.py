#!/usr/bin/env python3
"""Checks, without an application, that what the probe counts as a change to the focused node
is what it listens for: a description or value change on the focused node is heard and fails a
quiet window, and the same change on a node without focus does not; and that a second launch is
refused before anything runs when the launcher's environment is missing, and is given no
display. Exit status 1 on a failure.
"""

import os
import sys

from probe_support import ProbeError, check_step_arguments, second_launch

from probe_expectations import (
    FOCUSED_CHANGE_TYPES,
    is_focused_change,
    is_listened,
    quiet_window_holds,
)


def main():
    failures = []
    for event_type in FOCUSED_CHANGE_TYPES:
        if not is_listened(event_type):
            failures.append(f"{event_type} is not listened for")
    description = "object:property-change:accessible-description"
    if not is_focused_change(description, focused=True):
        failures.append("a focused description change is not counted")
    if is_focused_change(description, focused=False):
        failures.append("a description change without focus is counted")
    name = "Transport, playing"
    if quiet_window_holds(name, name, [f"{description} 'help'"], []):
        failures.append("a focused description change passes a quiet window")
    if quiet_window_holds(name, name, [], ["Removed."]):
        failures.append("an announcement passes a quiet window")
    if quiet_window_holds(name, "Transport, stopped", [], []):
        failures.append("a changed name passes a quiet window")
    if not quiet_window_holds(name, name, [], []):
        failures.append("a quiet window with nothing in it fails")
    saved = {name: os.environ.pop(name, None) for name in ("MIDI_EVIL_BINARY", "XDG_RUNTIME_DIR")}
    try:
        check_step_arguments([("open_in_running", __file__)])
        failures.append("a second launch without MIDI_EVIL_BINARY passes the pre-flight")
    except ProbeError:
        pass
    finally:
        os.environ.update({name: value for name, value in saved.items() if value is not None})
    command, environment = second_launch("song.evil", {
        "MIDI_EVIL_BINARY": "/bin/false", "XDG_RUNTIME_DIR": "/tmp/runtime",
        "DISPLAY": ":0", "WAYLAND_DISPLAY": "wayland-0"})
    if "DISPLAY" in environment or "WAYLAND_DISPLAY" in environment:
        failures.append("a second launch is given a display")
    if not os.path.isabs(command[1]):
        failures.append("a second launch is handed a relative path")
    for failure in failures:
        print(f"probe self-check: {failure}", file=sys.stderr)
    if not failures:
        print("probe self-check: a focused description or value change fails a quiet window, "
              "every counted change is listened for, and a second launch is refused without "
              "its environment and given no display")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
