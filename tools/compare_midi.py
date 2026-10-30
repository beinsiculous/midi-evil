#!/usr/bin/env python3
"""Compare note timing in two Standard MIDI Files."""

import struct
import sys
from collections import Counter
from pathlib import Path


def read_vlq(data, offset):
    value = 0
    while True:
        byte = data[offset]
        offset += 1
        value = (value << 7) | (byte & 0x7F)
        if not byte & 0x80:
            return value, offset


def parse(path):
    data = Path(path).read_bytes()
    if data[:4] != b"MThd":
        raise ValueError(f"{path}: not a Standard MIDI File")
    header_length = struct.unpack_from(">I", data, 4)[0]
    _, _, division = struct.unpack_from(">HHH", data, 8)
    offset = 8 + header_length
    tracks = []
    while offset < len(data):
        if data[offset : offset + 4] != b"MTrk":
            raise ValueError(f"{path}: expected MTrk at {offset:#x}")
        length = struct.unpack_from(">I", data, offset + 4)[0]
        offset += 8
        end = offset + length
        tick = 0
        status = None
        name = ""
        events = []
        while offset < end:
            delta, offset = read_vlq(data, offset)
            tick += delta
            if data[offset] & 0x80:
                status = data[offset]
                offset += 1
            if status == 0xFF:
                meta = data[offset]
                offset += 1
                size, offset = read_vlq(data, offset)
                payload = data[offset : offset + size]
                offset += size
                if meta == 0x03:
                    name = payload.decode("latin-1")
                status = None
            elif status in (0xF0, 0xF7):
                size, offset = read_vlq(data, offset)
                offset += size
                status = None
            else:
                size = 1 if status & 0xE0 in (0xC0, 0xD0) else 2
                payload = data[offset : offset + size]
                offset += size
                events.append((tick, status, *payload))
        tracks.append((name, events))
        offset = end
    return division, tracks



def note_key(event):
    tick, status, *payload = event
    channel = status & 0x0F
    kind = status & 0xF0
    if kind == 0x80 or (kind == 0x90 and payload[1] == 0):
        return tick, channel, "off", payload[0], 0
    if kind == 0x90:
        return tick, channel, "on", payload[0], payload[1]
    return None


def main(left_path, right_path):
    left_division, left_tracks = parse(left_path)
    right_division, right_tracks = parse(right_path)
    print(f"divisions: {left_division}, {right_division}")
    differences = int(left_division != right_division)
    grouped = []
    for tracks in (left_tracks, right_tracks):
        by_name = {}
        for name, events in tracks:
            if not name:
                continue
            notes = by_name.setdefault(name, Counter())
            notes.update(filter(None, map(note_key, events)))
        grouped.append(by_name)
    for name in sorted(set(grouped[0]) | set(grouped[1])):
        left = grouped[0].get(name, Counter())
        right = grouped[1].get(name, Counter())
        missing = left - right
        extra = right - left
        differences += sum(missing.values()) + sum(extra.values())
        print(
            f"{name!r}: {sum(left.values())}/{sum(right.values())} note events, "
            f"{sum(missing.values())} missing, {sum(extra.values())} extra"
        )
        for label, events in (("missing", missing), ("extra", extra)):
            for event, count in sorted(events.items())[:12]:
                print(f"  {label} x{count}: {event}")
    return differences


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} EXPECTED.mid ACTUAL.mid")
    raise SystemExit(main(sys.argv[1], sys.argv[2]))
