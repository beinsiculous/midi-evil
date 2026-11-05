#!/usr/bin/env python3
"""Compare original-synthesis GlockenBlocks renders with user-owned A09 references.

Render and measure the four verified pitches with:
    cargo run -p evil-synth --example phase6_render -- \
      --preset glocken --output-dir target/phase6/glocken-after
    python3 -I tools/calibrate_glocken.py \
      --native-dir target/phase6/glocken-after \
      --output target/phase6/glocken-after.json

Each pitch gets a modal model, not just an FFT centroid: a spectral peak near
each candidate ratio sets the measured frequency, complex demodulation in
100 ms Hann blocks estimates its amplitude trajectory, and a log-linear fit
estimates initial amplitude and exponential decay. Fit quality and unresolved
modes are reported explicitly. NumPy is the only third-party dependency.
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

SCHEMA = "midi-evil-a09-glocken-comparison-v1"
DEFAULT_REFERENCES = {
    "c2": "a09-c2-nanostudio.wav",
    "c3": "a09-c3-nanostudio.wav",
    "c4": "a09-nanostudio.wav",
    "c5": "a09-c5-nanostudio.wav",
}
PITCH_HZ = {"c2": 65.406391, "c3": 130.812783, "c4": 261.625565, "c5": 523.251131}
CANDIDATE_RATIOS = (1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 9.0)


def parse_window(value: str) -> tuple[float, float]:
    try:
        return parse_window_value(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def modal_model(samples: np.ndarray, sample_rate: int, fundamental: float) -> dict[str, object]:
    analysis_end = min(2.0, samples.size / sample_rate)
    early = samples[:round(analysis_end * sample_rate)]
    fft_size = 262_144
    spectrum = np.abs(np.fft.rfft(early * np.hanning(early.size), fft_size))
    frequencies = np.fft.rfftfreq(fft_size, 1.0 / sample_rate)
    maximum = float(spectrum.max(initial=0.0))
    modes = []
    for nominal_ratio in CANDIDATE_RATIOS:
        nominal = nominal_ratio * fundamental
        tolerance = max(2.0, nominal * 0.04)
        selected = np.flatnonzero((frequencies >= nominal - tolerance) & (frequencies <= nominal + tolerance))
        if selected.size == 0 or nominal > sample_rate * 0.45:
            continue
        index = int(selected[np.argmax(spectrum[selected])])
        relative_peak_db = float(20.0 * np.log10((spectrum[index] + EPSILON) / (maximum + EPSILON)))
        if relative_peak_db < -55.0:
            modes.append({"nominal_ratio": nominal_ratio, "resolved": False, "relative_peak_db": relative_peak_db})
            continue
        frequency = float(frequencies[index])
        if 0 < index < spectrum.size - 1:
            left, center, right = np.log(spectrum[index - 1:index + 2] + EPSILON)
            denominator = left - 2.0 * center + right
            if abs(denominator) > 1e-12:
                frequency += float(np.clip(0.5 * (left - right) / denominator, -0.5, 0.5)) * sample_rate / fft_size
        trajectory = demodulated_amplitudes(samples, sample_rate, frequency)
        fit = fit_decay(trajectory)
        modes.append({
            "nominal_ratio": nominal_ratio,
            "resolved": True,
            "relative_peak_db": relative_peak_db,
            "frequency_hz": frequency,
            "measured_ratio_to_equal_tempered_fundamental": frequency / fundamental,
            **fit,
        })
    return {
        "candidate_ratios": list(CANDIDATE_RATIOS),
        "frequency_search": "Largest early 0-2 s Hann FFT peak within +/-4% (minimum +/-2 Hz) of each candidate ratio; peaks below -55 dB relative to the strongest mode are unresolved.",
        "amplitude_estimation": "Complex demodulation in non-overlapping 100 ms Hann blocks, block centers used as times.",
        "decay_fit": "Least-squares log amplitude from 0.05 s through 6 s, using blocks at least 0.5% of that mode's peak and above -60 dBFS; decay_seconds is the exponential amplitude time constant.",
        "modes": modes,
    }


def demodulated_amplitudes(samples: np.ndarray, sample_rate: int, frequency: float) -> list[tuple[float, float]]:
    block = round(0.100 * sample_rate)
    count = samples.size // block
    if count == 0:
        return []
    window = np.hanning(block)
    trajectory = []
    for index in range(count):
        start = index * block
        segment = samples[start:start + block]
        times = np.arange(start, start + block, dtype=np.float64) / sample_rate
        phasor = np.exp(-2j * np.pi * frequency * times)
        amplitude = 2.0 * abs(np.sum(segment * window * phasor) / window.sum())
        trajectory.append(((index + 0.5) * block / sample_rate, float(amplitude)))
    return trajectory


def fit_decay(trajectory: list[tuple[float, float]]) -> dict[str, float | int | None]:
    if not trajectory:
        return {"initial_amplitude": None, "decay_seconds": None, "fit_r_squared": None, "fit_blocks": 0}
    peak = max(amplitude for _, amplitude in trajectory)
    selected = np.array([
        (time, amplitude) for time, amplitude in trajectory
        if 0.05 <= time <= 6.0 and amplitude >= max(peak * 0.005, 10.0 ** (-60.0 / 20.0))
    ])
    if selected.shape[0] < 4:
        return {"initial_amplitude": None, "decay_seconds": None, "fit_r_squared": None,
                "fit_blocks": int(selected.shape[0])}
    times = selected[:, 0]
    logs = np.log(selected[:, 1] + EPSILON)
    slope, intercept = np.polyfit(times, logs, 1)
    predicted = intercept + slope * times
    residual = float(np.sum((logs - predicted) ** 2))
    total = float(np.sum((logs - logs.mean()) ** 2))
    return {
        "initial_amplitude": float(np.exp(intercept)),
        "decay_seconds": float(-1.0 / slope) if slope < 0.0 else None,
        "fit_r_squared": 1.0 - residual / total if total > 0.0 else None,
        "fit_blocks": int(selected.shape[0]),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--reference-dir", type=Path, default=Path("target/phase6"))
    parser.add_argument("--native-dir", type=Path, default=Path("target/phase6/glocken-after"))
    parser.add_argument("--native-prefix", default="")
    parser.add_argument("--output", type=Path, default=Path("target/phase6/glocken-after.json"))
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
            "reference_modal_model": modal_model(reference_audio, args.sample_rate, expected),
            "native_modal_model": modal_model(native_audio, args.sample_rate, expected),
            "comparison": compare(reference_audio, native_audio, args.sample_rate, args.note_seconds),
        }
        comparisons.append(entry)
        resolved = [mode for mode in entry["reference_modal_model"]["modes"] if mode["resolved"]]
        ratios = ", ".join(f"{mode['measured_ratio_to_equal_tempered_fundamental']:.3f}" for mode in resolved)
        print(f"{label:3s} RMS {native['held_note_rms']:.6f} / ref {reference['held_note_rms']:.6f}; "
              f"reference modal ratios: {ratios}")
    report = {
        "schema": SCHEMA,
        "provenance": "Original synthesis compared with user-owned local NanoStudio 1.42 A09 captures; no copied reference audio in public assets.",
        "method": common_method(args.sample_rate, args.note_seconds, args.tail_windows),
        "comparisons": comparisons,
    }
    encoded = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(encoded, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
