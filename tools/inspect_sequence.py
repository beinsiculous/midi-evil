#!/usr/bin/env python3
"""Print decoded Package.sng track and pattern records for compatibility research."""

import struct
import sys
from pathlib import Path


def u32(data, offset):
    return struct.unpack_from("<I", data, offset)[0], offset + 4


def main(path, slots):
    data = Path(path).read_bytes()
    offset = 0xC8
    pattern_count, offset = u32(data, offset)
    patterns = {}
    for slot in range(pattern_count):
        length, offset = u32(data, offset)
        event_count, offset = u32(data, offset)
        events = []
        for _ in range(event_count):
            flags, note, velocity, note_flags = struct.unpack_from("<4B", data, offset)
            start, value = struct.unpack_from("<II", data, offset + 4)
            events.append((flags, note, velocity, note_flags, start, value))
            offset += 12
        if events:
            patterns[slot] = (length, events)
    for slot in slots:
        length, events = patterns[slot]
        print(f"pattern {slot}: length={length}, events={len(events)}")
        for event in events:
            print(f"  {event}")
    track_count, offset = u32(data, offset)
    print(f"patterns={len(patterns)}/{pattern_count}, tracks={track_count}")
    for index in range(track_count):
        unknown, instrument = struct.unpack_from("<II", data, offset)
        flags = data[offset + 8]
        part_count = u32(data, offset + 9)[0]
        offset += 13
        parts = []
        for _ in range(part_count):
            slot, preset, start, length = struct.unpack_from("<4I", data, offset)
            parts.append((slot, preset, start, length))
            offset += 16
        print(
            f"track {index}: instrument={instrument}, flags={flags:#04x}, "
            f"unknown={unknown}, parts={len(parts)}"
        )
        for part in parts:
            print(f"  slot={part[0]} preset={part[1]} start={part[2]} length={part[3]}")
    if offset != len(data):
        print(f"trailing bytes: {len(data) - offset}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(f"usage: {sys.argv[0]} Package.sng [pattern-slot ...]")
    main(sys.argv[1], [int(slot) for slot in sys.argv[2:]])
