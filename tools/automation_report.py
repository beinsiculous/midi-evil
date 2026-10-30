#!/usr/bin/env python3
"""Summarize raw Package.sng controllers and official MIDI automation."""

import importlib.util
import struct
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

spec = importlib.util.spec_from_file_location("compare_midi", Path(__file__).with_name("compare_midi.py"))
compare_midi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare_midi)
parse = compare_midi.parse


def u32(data, offset):
    return struct.unpack_from("<I", data, offset)[0], offset + 4


def raw_controllers(project):
    data = (project / "Package.sng").read_bytes()
    root = ET.parse(project / "Package.prj").getroot()
    names = {
        int(node.attrib["Index"]): node.find("UI").attrib["Name"]
        for node in root.find("Instruments")
    }
    offset = 0xC8
    pattern_count, offset = u32(data, offset)
    patterns = {}
    for slot in range(pattern_count):
        length, offset = u32(data, offset)
        event_count, offset = u32(data, offset)
        events = []
        for _ in range(event_count):
            flags, first, second, third = struct.unpack_from("<4B", data, offset)
            start, value = struct.unpack_from("<II", data, offset + 4)
            if flags & 1:
                events.append((first, second, third, start, value))
            offset += 12
        if events:
            patterns[slot] = (length, events)
    track_count, offset = u32(data, offset)
    tracks = []
    for _ in range(track_count):
        instrument = struct.unpack_from("<I", data, offset + 4)[0]
        part_count = u32(data, offset + 9)[0]
        offset += 13
        controllers = []
        for _ in range(part_count):
            slot, _, start, length = struct.unpack_from("<4I", data, offset)
            offset += 16
            if slot not in patterns:
                continue
            pattern_length, events = patterns[slot]
            repeat = 0
            while repeat < length:
                for first, second, third, event_start, value in events:
                    absolute = start + repeat + event_start
                    if absolute < start + length:
                        controllers.append(
                            (absolute, first, second, third, value, slot)
                        )
                repeat += pattern_length
        tracks.append((names[instrument], controllers))
    return tracks


def midi_kind(event):
    _, status, *payload = event
    kind = status & 0xF0
    if kind == 0xB0:
        return f"cc{payload[0]}"
    return {
        0xA0: "poly-aftertouch",
        0xC0: "program",
        0xD0: "channel-aftertouch",
        0xE0: "pitch-bend",
    }.get(kind)


def main(project, midi):
    raw = dict(raw_controllers(project))
    official = dict(parse(midi)[1])
    for name in sorted(set(raw) | set(official)):
        raw_types = Counter((event[1], event[3]) for event in raw.get(name, []))
        official_types = Counter(
            kind
            for event in official.get(name, [])
            if (kind := midi_kind(event)) is not None
        )
        print(f"\n{name}: raw={sum(raw_types.values())}, official={sum(official_types.values())}")
        print(f"  raw (data0, data2): {sorted(raw_types.items())}")
        print(f"  official: {sorted(official_types.items())}")
        for event in sorted(raw.get(name, []))[:8]:
            print(f"  raw sample: {event}")
        for event in official.get(name, [])[:8]:
            if midi_kind(event) is not None:
                print(f"  midi sample: {event}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} PROJECT.prj REFERENCE.mid")
    main(Path(sys.argv[1]), sys.argv[2])
