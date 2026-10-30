#!/usr/bin/env python3
"""Compare mapped controller and pitch-bend events in two MIDI files."""

import importlib.util
import sys
from collections import Counter
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "compare_midi", Path(__file__).with_name("compare_midi.py")
)
compare_midi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare_midi)
parse = compare_midi.parse


def automation_key(event):
    tick, status, *payload = event
    channel = status & 0x0F
    kind = status & 0xF0
    if kind == 0xB0:
        return tick, channel, "cc", payload[0], payload[1]
    if kind == 0xC0:
        return tick, channel, "program", payload[0]
    if kind == 0xE0:
        return tick, channel, "pitch-bend", ((payload[1] << 7) | payload[0]) - 8192
    if kind == 0xA0:
        return tick, channel, "poly-aftertouch", payload[0], payload[1]
    if kind == 0xD0:
        return tick, channel, "channel-aftertouch", payload[0]
    return None


def grouped(tracks):
    result = {}
    for name, events in tracks:
        if not name:
            continue
        automation = result.setdefault(name, Counter())
        automation.update(filter(None, map(automation_key, events)))
    return result


def main(expected_path, actual_path):
    expected_division, expected_tracks = parse(expected_path)
    actual_division, actual_tracks = parse(actual_path)
    print(f"divisions: {expected_division}, {actual_division}")
    differences = int(expected_division != actual_division)
    expected = grouped(expected_tracks)
    actual = grouped(actual_tracks)
    for name in sorted(set(expected) | set(actual)):
        missing = expected.get(name, Counter()) - actual.get(name, Counter())
        extra = actual.get(name, Counter()) - expected.get(name, Counter())
        differences += sum(missing.values()) + sum(extra.values())
        print(
            f"{name!r}: {sum(missing.values())} missing, "
            f"{sum(extra.values())} extra automation events"
        )
        for label, events in (("missing", missing), ("extra", extra)):
            for event, count in sorted(events.items())[:12]:
                print(f"  {label} x{count}: {event}")
    return differences


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} EXPECTED.mid ACTUAL.mid")
    raise SystemExit(main(sys.argv[1], sys.argv[2]))
