#!/usr/bin/env python3
"""Measure the user-owned Chiptune reference bank without copying its audio.

Usage:
    python3 -I tools/calibrate_chiptune.py SOURCE_DIRECTORY OUTPUT_JSON [--native-dir NATIVE_PAD_DIRECTORY]

The JSON is deterministic: it contains relative file names, checksums and
measurements, but no timestamps or absolute source paths. NumPy is the only
third-party dependency.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import wave
import xml.etree.ElementTree as ET

import numpy as np

SCHEMA = "midi-evil-chiptune-measurements-v1"
BANDS_HZ = ((0, 100), (100, 300), (300, 1_000), (1_000, 3_000), (3_000, 8_000), (8_000, 22_050))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def read_pcm16_mono(path: Path) -> tuple[int, np.ndarray]:
    with wave.open(str(path), "rb") as stream:
        if stream.getparams()[:3] != (1, 2, 44_100):
            raise ValueError(f"{path.name}: expected mono PCM16 at 44100 Hz")
        frames = stream.getnframes()
        raw = stream.readframes(frames)
    samples = np.frombuffer(raw, dtype="<i2").astype(np.float64) / 32_768.0
    return 44_100, samples


def first_crossing(values: np.ndarray, threshold: float) -> int | None:
    indices = np.flatnonzero(values >= threshold)
    return int(indices[0]) if indices.size else None


def first_below(values: np.ndarray, threshold: float) -> int | None:
    indices = np.flatnonzero(values < threshold)
    return int(indices[0]) if indices.size else None


def spectral_summary(samples: np.ndarray, sample_rate: int) -> dict[str, object]:
    window = np.hanning(samples.size)
    spectrum = np.abs(np.fft.rfft(samples * window))
    power = spectrum * spectrum
    frequencies = np.fft.rfftfreq(samples.size, 1.0 / sample_rate)
    total = float(power.sum())
    if total == 0.0:
        return {
            "dominant_hz": 0.0,
            "centroid_hz": 0.0,
            "rolloff_85_hz": 0.0,
            "band_energy_fraction": [0.0] * len(BANDS_HZ),
        }
    cumulative = np.cumsum(power)
    rolloff_index = int(np.searchsorted(cumulative, cumulative[-1] * 0.85))
    bands = []
    for low, high in BANDS_HZ:
        selected = (frequencies >= low) & (frequencies < high)
        bands.append(float(power[selected].sum() / total))
    return {
        "dominant_hz": float(frequencies[int(np.argmax(power))]),
        "centroid_hz": float(np.sum(frequencies * power) / total),
        "rolloff_85_hz": float(frequencies[min(rolloff_index, frequencies.size - 1)]),
        "band_energy_fraction": bands,
    }


def estimate_f0(samples: np.ndarray, sample_rate: int) -> float | None:
    """Normalized autocorrelation pitch estimate over the first 100 ms."""
    segment = samples[: min(samples.size, sample_rate // 10)].copy()
    segment -= float(segment.mean())
    if segment.size < 128 or float(segment @ segment) < 1e-12:
        return None
    # FFT correlation keeps this dependency-free and quick for every pad.
    size = 1 << (2 * segment.size - 1).bit_length()
    spectrum = np.fft.rfft(segment, size)
    correlation = np.fft.irfft(spectrum * np.conj(spectrum), size)[: segment.size]
    correlation /= np.maximum(correlation[0], 1e-20)
    minimum_lag = max(1, sample_rate // 2_000)
    maximum_lag = min(segment.size - 1, sample_rate // 40)
    if minimum_lag >= maximum_lag:
        return None
    lags = np.arange(minimum_lag, maximum_lag + 1)
    # Unbiased correlation avoids systematically choosing the longest lag.
    normalized = correlation[lags] * segment.size / (segment.size - lags)
    best = int(np.argmax(normalized))
    if normalized[best] < 0.25:
        return None
    return float(sample_rate / lags[best])


def measure(path: Path) -> dict[str, object]:
    sample_rate, samples = read_pcm16_mono(path)
    absolute = np.abs(samples)
    peak = float(absolute.max())
    peak_index = int(np.argmax(absolute))
    rms = float(np.sqrt(np.mean(samples * samples)))
    onset_threshold = max(peak * 0.01, 1.0 / 32_768.0)
    onset_index = first_crossing(absolute, onset_threshold)
    if onset_index is None:
        onset_index = 0
    envelope_size = max(1, sample_rate // 1_000)
    envelope_frames = (samples.size + envelope_size - 1) // envelope_size
    padded = np.pad(samples, (0, envelope_frames * envelope_size - samples.size))
    envelope = np.sqrt(np.mean(padded.reshape(-1, envelope_size) ** 2, axis=1))
    envelope_peak_index = int(np.argmax(envelope))
    envelope_peak = float(envelope[envelope_peak_index])
    decay_envelope = envelope[envelope_peak_index:]
    t10_index = first_below(decay_envelope, envelope_peak * 0.1)
    t1_index = first_below(decay_envelope, envelope_peak * 0.01)
    signs = np.signbit(samples)
    zero_crossings = int(np.count_nonzero(signs[1:] != signs[:-1]))
    analysis = samples[onset_index:]
    return {
        "file": path.name,
        "sha256": sha256(path),
        "format": {"channels": 1, "encoding": "PCM16", "sample_rate_hz": sample_rate},
        "frames": int(samples.size),
        "duration_seconds": float(samples.size / sample_rate),
        "rms": rms,
        "peak": peak,
        "crest_factor": float(peak / rms) if rms else None,
        "onset_seconds": float(onset_index / sample_rate),
        "peak_time_seconds": float(peak_index / sample_rate),
        "attack_from_onset_seconds": float((peak_index - onset_index) / sample_rate),
        "time_below_10_percent_seconds": float((envelope_peak_index + t10_index) * envelope_size / sample_rate - onset_index / sample_rate) if t10_index is not None else None,
        "time_below_1_percent_seconds": float((envelope_peak_index + t1_index) * envelope_size / sample_rate - onset_index / sample_rate) if t1_index is not None else None,
        "zero_crossing_rate_hz": float(zero_crossings * sample_rate / samples.size),
        "estimated_f0_hz": estimate_f0(analysis, sample_rate),
        "spectrum": spectral_summary(analysis, sample_rate),
    }


def package_settings(path: Path) -> list[dict[str, object]]:
    root = ET.parse(path).getroot()
    settings = []
    for index in range(16):
        pad = root.find(f"Pad{index}")
        if pad is None:
            raise ValueError(f"Package.trg: missing Pad{index}")
        envelope = pad.find("AmpEnv")
        if envelope is None:
            raise ValueError(f"Package.trg: Pad{index} has no AmpEnv")
        settings.append({
            "pad_index": index,
            "file": pad.attrib["Nam"],
            "volume": float(pad.attrib["Vol"]),
            "pan": float(pad.attrib["Pan"]),
            "transpose_semitones": int(pad.attrib["Tps"]),
            "fine_pitch": int(pad.attrib["Pit"]),
            "output_bus": int(pad.attrib["Bus"]),
            "choke_group": int(pad.attrib["Grp"]),
            "start_frame": int(pad.attrib["Sta"]),
            "end_frame": int(pad.attrib["End"]),
            "direction": pad.attrib["Dir"],
            "trigger": pad.attrib["Trg"],
            "amp_envelope": {key: float(envelope.attrib[key]) for key in ("A", "D", "S", "R")},
        })
    return settings


def main() -> int:
    if len(sys.argv) not in (3, 5) or (len(sys.argv) == 5 and sys.argv[3] != "--native-dir"):
        print(__doc__.strip(), file=sys.stderr)
        return 2
    source = Path(sys.argv[1])
    output = Path(sys.argv[2])
    native_directory = Path(sys.argv[4]) if len(sys.argv) == 5 else None
    package = source / "Package.trg"
    settings = package_settings(package)
    files = [source / str(pad["file"]) for pad in settings]
    if len({path.name for path in files}) != 16:
        raise ValueError("Package.trg does not name 16 distinct WAVs")
    report = {
        "schema": SCHEMA,
        "provenance": "User-owned local NanoStudio 1 factory Chiptune.trg; measurement only; audio not redistributed.",
        "package": {"file": package.name, "sha256": sha256(package), "pads": settings},
        "method": {
            "amplitude": "PCM16 normalized by 32768; full-file RMS/peak; onset is first |x| >= max(1% peak, 1 LSB).",
            "envelope": "Peak/attack use absolute samples. Decay uses non-overlapping 1 ms RMS blocks and reports time from onset until the post-peak envelope falls below 10%/1% of its peak.",
            "spectrum": "Hann-windowed FFT from onset through end; power centroid, 85% power rolloff, and six band-energy fractions.",
            "bands_hz": [list(band) for band in BANDS_HZ],
            "pitch": "Normalized autocorrelation of first 100 ms after onset, lag range 40..2000 Hz; null below 0.25 correlation.",
        },
        "pads": [dict(settings[index], measurement=measure(path)) for index, path in enumerate(files)],
    }
    if native_directory is not None:
        report["native_approximations"] = [
            {
                "pad_index": index,
                "file": f"pad-{index:02}.wav",
                "measurement": measure(native_directory / f"pad-{index:02}.wav"),
            }
            for index in range(16)
        ]
        report["native_note"] = "Original synthesized approximations, not factory audio; compare only with the per-pad source measurements above."
    encoded = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(encoded, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
