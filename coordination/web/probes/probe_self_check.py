#!/usr/bin/env python3
"""Checks, without an application, that what the probe counts as a change to the focused node
is what it listens for: a description or value change on the focused node is heard and fails a
quiet window, and the same change on a node without focus does not. Exit status 1 on a failure.
"""

import sys

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
    for failure in failures:
        print(f"probe self-check: {failure}", file=sys.stderr)
    if not failures:
        print("probe self-check: a focused description or value change fails a quiet window, "
              "and every counted change is listened for")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
