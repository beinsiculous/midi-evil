#!/usr/bin/env python3
"""Compare original-synthesis A17.Nasty renders with user-owned references.

Examples:
    python3 -I tools/calibrate_nasty.py \
      --native-dir target/nasty/phase6-before \
      --output target/nasty/phase6-before.json

    python3 -I tools/calibrate_nasty.py \
      --compare c4 reference.wav native.wav \
      --early-tail 3:4 --late-tail 6:7 --output comparison.json

No reference audio is copied or embedded in the JSON. NumPy is the only
third-party dependency. Paths in the report are basenames so repeated runs
from different directories produce identical reports for identical audio.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import wave

import numpy as np

SCHEMA = "midi-evil-a17-nasty-comparison-v1"
DEFAULT_REFERENCES = {
    "c2": "a17-c2-nanostudio.wav",
    "c3": "a17-c3-nanostudio.wav",
    "c4": "a17-wholebar-clean-nanostudio.wav",
    "c5": "a17-c5-nanostudio.wav",
}
BANDS_HZ = ((0, 200), (200, 1_000), (1_000, 4_000), (4_000, 8_000), (8_000, 16_000), (16_000, 24_000))
EPSILON = 1e-30


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def read_wav(path: Path) -> tuple[int, np.ndarray]:
    with wave.open(str(path), "rb") as stream:
        channels = stream.getnchannels()
        sample_rate = stream.getframerate()
        width = stream.getsampwidth()
        if channels not in (1, 2) or width != 2:
            raise ValueError(f"{path}: expected mono/stereo PCM16, got {channels} channels and {width * 8}-bit samples")
        raw = stream.readframes(stream.getnframes())
    samples = np.frombuffer(raw, dtype="<i2").astype(np.float64) / 32_768.0
    return sample_rate, samples.reshape(-1, channels)


def resample_linear(samples: np.ndarray, source_rate: int, target_rate: int) -> np.ndarray:
    if source_rate == target_rate:
        return samples
    if samples.shape[0] == 0:
        return samples.copy()
    old_time = np.arange(samples.shape[0], dtype=np.float64) / source_rate
    new_time = np.arange(round(samples.shape[0] * target_rate / source_rate), dtype=np.float64) / target_rate
    if samples.ndim == 1:
        return np.interp(new_time, old_time, samples)
    return np.column_stack([np.interp(new_time, old_time, samples[:, channel]) for channel in range(samples.shape[1])])


def rms(samples: np.ndarray) -> float:
    if samples.size == 0:
        return 0.0
    return float(np.sqrt(np.mean(samples * samples)))


def seconds_slice(samples: np.ndarray, sample_rate: int, start: float, end: float) -> np.ndarray:
    return samples[round(start * sample_rate) : round(end * sample_rate)]


def block_envelope(samples: np.ndarray, sample_rate: int, block_seconds: float) -> np.ndarray:
    block = max(1, round(sample_rate * block_seconds))
    count = math.ceil(samples.size / block)
    padded = np.pad(samples, (0, count * block - samples.size))
    return np.sqrt(np.mean(padded.reshape(count, block) ** 2, axis=1))


def correlation(left: np.ndarray, right: np.ndarray) -> float | None:
    count = min(left.size, right.size)
    if count == 0:
        return None
    left = left[:count].astype(np.float64, copy=False)
    right = right[:count].astype(np.float64, copy=False)
    left = left - left.mean()
    right = right - right.mean()
    denominator = float(np.linalg.norm(left) * np.linalg.norm(right))
    return float(np.dot(left, right) / denominator) if denominator > 0.0 else None


def onset_and_activity(samples: np.ndarray, sample_rate: int) -> dict[str, float | int | None]:
    envelope = block_envelope(samples, sample_rate, 0.001)
    peak = float(envelope.max(initial=0.0))
    threshold = max(peak * 0.001, 10.0 ** (-80.0 / 20.0))
    active = np.flatnonzero(envelope >= threshold)
    onset_frame = int(active[0]) if active.size else None
    last_frame = int(active[-1]) if active.size else None
    return {
        "onset_seconds": onset_frame * 0.001 if onset_frame is not None else None,
        "active_duration_seconds": (last_frame + 1) * 0.001 if last_frame is not None else 0.0,
        "last_active_seconds": (last_frame + 1) * 0.001 if last_frame is not None else None,
        "activity_threshold_rms": threshold,
        "activity_block_seconds": 0.001,
    }


def measure(path: Path, args: argparse.Namespace) -> tuple[dict[str, object], np.ndarray]:
    source_rate, stereo = read_wav(path)
    stereo = resample_linear(stereo, source_rate, args.sample_rate)
    mono = stereo.mean(axis=1)
    held = seconds_slice(mono, args.sample_rate, 0.0, args.note_seconds)
    if held.size != round(args.note_seconds * args.sample_rate):
        raise ValueError(f"{path}: shorter than the {args.note_seconds:g}-second held-note analysis window")
    window = np.hanning(held.size)
    magnitudes = np.abs(np.fft.rfft(held * window))
    power = magnitudes * magnitudes
    frequencies = np.fft.rfftfreq(held.size, 1.0 / args.sample_rate)
    total_magnitude = float(magnitudes.sum())
    total_power = float(power.sum())
    bands = {}
    for low, high in BANDS_HZ:
        selected = (frequencies >= low) & (frequencies < high)
        band_power = float(power[selected].sum())
        bands[f"{low}-{high}"] = {
            "energy_db": float(10.0 * np.log10(band_power + EPSILON)),
            "energy_fraction": band_power / total_power if total_power > 0.0 else 0.0,
        }
    windows = {
        "release": args.release_tail,
        "early": args.early_tail,
        "late": args.late_tail,
    }
    tail_rms = {
        name: rms(seconds_slice(mono, args.sample_rate, start, end))
        for name, (start, end) in windows.items()
    }
    measurement = {
        "file": path.name,
        "sha256": sha256(path),
        "source_format": {
            "channels": int(stereo.shape[1]),
            "encoding": "PCM16",
            "sample_rate_hz": source_rate,
        },
        "frames_at_analysis_rate": int(mono.size),
        "duration_seconds": float(mono.size / args.sample_rate),
        "peak": float(np.max(np.abs(mono), initial=0.0)),
        "full_file_rms": rms(mono),
        "held_note_rms": rms(held),
        "held_note_spectral_centroid_hz": float(np.sum(frequencies * magnitudes) / total_magnitude) if total_magnitude > 0.0 else 0.0,
        "held_note_bands": bands,
        "tail_rms": tail_rms,
        **onset_and_activity(mono, args.sample_rate),
    }
    measurement["post_gate_active_duration_seconds"] = max(0.0, float(measurement["active_duration_seconds"]) - args.note_seconds)
    return measurement, mono


def compare(reference: np.ndarray, native: np.ndarray, args: argparse.Namespace) -> dict[str, object]:
    count = min(reference.size, native.size, round(args.note_seconds * args.sample_rate))
    reference_held = reference[:count]
    native_held = native[:count]
    envelope_count = round(args.note_seconds / 0.010)
    block = round(args.sample_rate * 0.010)
    reference_envelope = np.sqrt(np.mean(reference_held[: envelope_count * block].reshape(envelope_count, block) ** 2, axis=1))
    native_envelope = np.sqrt(np.mean(native_held[: envelope_count * block].reshape(envelope_count, block) ** 2, axis=1))
    full_count = max(reference.size, native.size)
    reference_full = np.pad(reference, (0, full_count - reference.size))
    native_full = np.pad(native, (0, full_count - native.size))
    full_blocks = math.ceil(full_count / block)
    reference_full_envelope = block_envelope(reference_full, args.sample_rate, 0.010)
    native_full_envelope = block_envelope(native_full, args.sample_rate, 0.010)
    return {
        "held_note_waveform_correlation": correlation(reference_held, native_held),
        "held_note_10ms_envelope_correlation": correlation(reference_envelope, native_envelope),
        "full_tail_10ms_envelope_correlation": correlation(reference_full_envelope, native_full_envelope),
        "full_tail_10ms_envelope_blocks": full_blocks,
    }


def parse_window(value: str) -> tuple[float, float]:
    try:
        start_text, end_text = value.split(":", 1)
        start, end = float(start_text), float(end_text)
    except ValueError as error:
        raise argparse.ArgumentTypeError("window must be START:END in seconds") from error
    if not math.isfinite(start) or not math.isfinite(end) or start < 0.0 or end <= start:
        raise argparse.ArgumentTypeError("window requires 0 <= START < END")
    return start, end


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--reference-dir", type=Path, default=Path("target/nasty"))
    parser.add_argument("--native-dir", type=Path, default=Path("target/nasty/phase6-after"))
    parser.add_argument("--native-prefix", default="", help="prefix before c2/c3/c4/c5 and the .wav suffix")
    parser.add_argument("--output", type=Path, default=Path("target/nasty/phase6-after.json"))
    parser.add_argument("--compare", nargs=3, action="append", metavar=("LABEL", "REFERENCE_WAV", "NATIVE_WAV"), help="explicit comparison; may be repeated and replaces the default four pitches")
    parser.add_argument("--sample-rate", type=int, default=48_000, help="common analysis rate; references are linearly resampled")
    parser.add_argument("--note-seconds", type=float, default=2.0)
    parser.add_argument("--release-tail", type=parse_window, default=parse_window("2:2.5"))
    parser.add_argument("--early-tail", type=parse_window, default=parse_window("3:4"))
    parser.add_argument("--late-tail", type=parse_window, default=parse_window("6:7"))
    args = parser.parse_args()
    if args.sample_rate < 48_000:
        parser.error("--sample-rate must be at least 48000 Hz so the 16-24 kHz band is represented")
    if (
        not math.isfinite(args.note_seconds)
        or args.note_seconds <= 0.0
        or not math.isclose(round(args.note_seconds / 0.010) * 0.010, args.note_seconds, rel_tol=0.0, abs_tol=1e-9)
    ):
        parser.error("--note-seconds must be a positive multiple of 0.010 seconds")
    if args.compare:
        args.pairs = [(label, Path(reference), Path(native)) for label, reference, native in args.compare]
    else:
        args.pairs = [
            (label, args.reference_dir / filename, args.native_dir / f"{args.native_prefix}{label}.wav")
            for label, filename in DEFAULT_REFERENCES.items()
        ]
    return args


def main() -> int:
    args = parse_args()
    comparisons = []
    for label, reference_path, native_path in args.pairs:
        reference, reference_audio = measure(reference_path, args)
        native, native_audio = measure(native_path, args)
        comparisons.append({
            "label": label,
            "reference": reference,
            "native": native,
            "comparison": compare(reference_audio, native_audio, args),
        })
    report = {
        "schema": SCHEMA,
        "provenance": "Original synthesis compared with user-owned local NanoStudio captures; no copied reference audio in public assets.",
        "method": {
            "analysis_sample_rate_hz": args.sample_rate,
            "resampling": "Linear interpolation of each stereo channel, then arithmetic stereo mean.",
            "held_note_seconds": args.note_seconds,
            "rms": "Full-scale PCM16 samples normalized by 32768; RMS of the stereo mean.",
            "spectrum": "Hann-windowed held-note FFT. Centroid uses magnitude weighting to remain comparable with earlier A17 reports; band energy uses squared magnitude.",
            "bands_hz": [list(band) for band in BANDS_HZ],
            "envelope": "Non-overlapping 10 ms RMS blocks of the stereo mean; Pearson correlation. Full-tail comparison zero-pads the shorter file.",
            "onset_and_activity": "Non-overlapping 1 ms RMS blocks. Active blocks are at least max(0.1% of peak block RMS, -80 dBFS); onset and last-active times use block boundaries.",
            "tail_windows_seconds": {
                "release": list(args.release_tail),
                "early": list(args.early_tail),
                "late": list(args.late_tail),
            },
        },
        "comparisons": comparisons,
    }
    encoded = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(encoded, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
