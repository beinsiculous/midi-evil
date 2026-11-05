#!/usr/bin/env python3
"""Compare original-synthesis Evil-303 renders with user-owned B14 references.

Render all cases first, for example:
    cargo run -p evil-synth --example phase6_render -- \
      --preset nano-303 --output-dir target/phase6/nano303-after
    python3 -I tools/calibrate_nano303.py \
      --native-dir target/phase6/nano303-after \
      --output target/phase6/nano303-after.json

Use --compare LABEL REFERENCE.wav NATIVE.wav for arbitrary pairs. Labels c2,
c3, c4, c5, soft, accent, and glide enable the corresponding expected pitch
and derived velocity/glide measurements. No reference audio is copied into
JSON or public assets. NumPy is the only third-party dependency.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import sys

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from calibrate_phase6_audio import (
    EPSILON, common_method, compare, measure, parse_window as parse_window_value,
)

SCHEMA = "midi-evil-b14-nano303-comparison-v1"
DEFAULT_REFERENCES = {
    "c2": "b14-c2-nanostudio.wav",
    "c3": "b14-c3-nanostudio.wav",
    "c4": "b14-nanostudio.wav",
    "c5": "b14-c5-nanostudio.wav",
    "soft": "b14-soft-nanostudio.wav",
    "accent": "b14-accent-nanostudio.wav",
    "glide": "b14-glide-nanostudio.wav",
}
PITCH_HZ = {"c2": 65.406391, "c3": 130.812783, "c4": 261.625565, "c5": 523.251131,
            "soft": 261.625565, "accent": 261.625565, "glide": 261.625565}


def parse_window(value: str) -> tuple[float, float]:
    try:
        return parse_window_value(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def glide_trajectory(samples: np.ndarray, sample_rate: int) -> dict[str, object]:
    window_seconds = 0.10
    hop_seconds = 0.01
    window_frames = round(window_seconds * sample_rate)
    hop_frames = round(hop_seconds * sample_rate)
    fft_size = 65_536
    frequencies = np.fft.rfftfreq(fft_size, 1.0 / sample_rate)
    selected = np.flatnonzero((frequencies >= 230.0) & (frequencies <= 360.0))
    points = []
    for center in np.arange(0.80, 1.6001, hop_seconds):
        middle = round(center * sample_rate)
        segment = samples[middle - window_frames // 2:middle + window_frames // 2]
        if segment.size != window_frames:
            continue
        magnitudes = np.abs(np.fft.rfft(segment * np.hanning(window_frames), fft_size))
        index = int(selected[np.argmax(magnitudes[selected])])
        frequency = float(frequencies[index])
        if 0 < index < magnitudes.size - 1:
            left, peak, right = np.log(magnitudes[index - 1:index + 2] + EPSILON)
            denominator = left - 2.0 * peak + right
            if abs(denominator) > 1e-12:
                frequency += float(np.clip(0.5 * (left - right) / denominator, -0.5, 0.5)) * sample_rate / fft_size
        points.append((float(center), frequency))
    before = np.median([frequency for time, frequency in points if 0.85 <= time <= 0.95])
    after = np.median([frequency for time, frequency in points if 1.45 <= time <= 1.55])
    thresholds = {}
    for fraction in (0.1, 0.5, 0.9):
        target = before + (after - before) * fraction
        crossing = next((time for time, frequency in points if time >= 1.0 and frequency >= target), None)
        thresholds[f"{fraction:g}"] = crossing
    t10, t50, t90 = (thresholds[key] for key in ("0.1", "0.5", "0.9"))
    return {
        "window_seconds": window_seconds,
        "hop_seconds": hop_seconds,
        "search_hz": [230.0, 360.0],
        "start_frequency_hz": float(before),
        "target_frequency_hz": float(after),
        "transition_10_to_90_seconds": t90 - t10 if t10 is not None and t90 is not None else None,
        "transition_midpoint_seconds": t50,
        "midpoint_offset_from_second_note_seconds": t50 - 1.0 if t50 is not None else None,
        "crossing_seconds": thresholds,
        "trajectory": [{"time_seconds": time, "fundamental_hz": frequency} for time, frequency in points],
    }


def relative_response(comparisons: list[dict[str, object]], label: str) -> dict[str, object] | None:
    by_label = {entry["label"]: entry for entry in comparisons}
    if "c4" not in by_label or label not in by_label:
        return None
    result = {}
    for side in ("reference", "native"):
        control = by_label["c4"][side]
        changed = by_label[label][side]
        control_spectrum = control["held_note_spectrum"]
        changed_spectrum = changed["held_note_spectrum"]
        result[side] = {
            "held_rms_ratio_to_c4": changed["held_note_rms"] / max(control["held_note_rms"], EPSILON),
            "peak_ratio_to_c4": changed["peak"] / max(control["peak"], EPSILON),
            "centroid_ratio_to_c4": changed_spectrum["spectral_centroid_hz"] / max(control_spectrum["spectral_centroid_hz"], EPSILON),
            "band_energy_db_difference_from_c4": {
                band: changed_spectrum["bands"][band]["energy_db"] - control_spectrum["bands"][band]["energy_db"]
                for band in control_spectrum["bands"]
            },
        }
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--reference-dir", type=Path, default=Path("target/phase6"))
    parser.add_argument("--native-dir", type=Path, default=Path("target/phase6/nano303-after"))
    parser.add_argument("--native-prefix", default="")
    parser.add_argument("--output", type=Path, default=Path("target/phase6/nano303-after.json"))
    parser.add_argument("--compare", nargs=3, action="append", metavar=("LABEL", "REFERENCE_WAV", "NATIVE_WAV"))
    parser.add_argument("--sample-rate", type=int, default=48_000)
    parser.add_argument("--note-seconds", type=float, default=2.0)
    parser.add_argument("--release-tail", type=parse_window, default=parse_window("2:2.5"))
    parser.add_argument("--early-tail", type=parse_window, default=parse_window("3:4"))
    parser.add_argument("--late-tail", type=parse_window, default=parse_window("6:7"))
    args = parser.parse_args()
    if args.sample_rate < 48_000 or not math.isfinite(args.note_seconds) or args.note_seconds != 2.0:
        parser.error("use --sample-rate >= 48000 and the verified two-second note gate")
    args.tail_windows = {"release": args.release_tail, "early": args.early_tail, "late": args.late_tail}
    args.pairs = ([(label, Path(reference), Path(native)) for label, reference, native in args.compare]
                  if args.compare else [
                      (label, args.reference_dir / filename, args.native_dir / f"{args.native_prefix}{label}.wav")
                      for label, filename in DEFAULT_REFERENCES.items()
                  ])
    return args


def main() -> int:
    args = parse_args()
    comparisons = []
    for label, reference_path, native_path in args.pairs:
        expected = PITCH_HZ.get(label, 261.625565)
        reference, reference_audio = measure(reference_path, args.sample_rate, args.note_seconds, expected, args.tail_windows)
        native, native_audio = measure(native_path, args.sample_rate, args.note_seconds, expected, args.tail_windows)
        entry = {
            "label": label,
            "reference": reference,
            "native": native,
            "comparison": compare(reference_audio, native_audio, args.sample_rate, args.note_seconds),
        }
        if label == "glide":
            entry["glide"] = {
                "reference": glide_trajectory(reference_audio, args.sample_rate),
                "native": glide_trajectory(native_audio, args.sample_rate),
            }
        comparisons.append(entry)
        print(f"{label:6s} RMS {native['held_note_rms']:.6f} / ref {reference['held_note_rms']:.6f}; "
              f"centroid {native['held_note_spectrum']['spectral_centroid_hz']:.1f} / ref "
              f"{reference['held_note_spectrum']['spectral_centroid_hz']:.1f} Hz; "
              f"tail corr {entry['comparison']['full_tail_10ms_envelope_correlation']:.4f}")
    report = {
        "schema": SCHEMA,
        "provenance": "Original synthesis compared with user-owned local NanoStudio 1.42 B14 captures; no copied reference audio in public assets.",
        "method": common_method(args.sample_rate, args.note_seconds, args.tail_windows),
        "velocity_response": {label: relative_response(comparisons, label) for label in ("soft", "accent")},
        "comparisons": comparisons,
    }
    encoded = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(encoded, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
