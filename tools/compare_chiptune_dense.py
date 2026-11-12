#!/usr/bin/env python3
"""Compare a native Chiptune dense render with a user-owned NanoStudio capture.

Usage:
    python3 -I tools/compare_chiptune_dense.py SPEC_JSON NATIVE.wav REFERENCE.wav OUTPUT_JSON [--repeat REPEAT.wav]

NumPy is the only third-party dependency. The source fixture must separately pass
Oracle's verify-dense command before this comparison is used as evidence.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import wave

import numpy as np

SCHEMA = "midi-evil-chiptune-dense-native-v2"
MAX_LAG_SAMPLES = 512


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def read_wav(path: Path) -> np.ndarray:
    with wave.open(str(path), "rb") as stream:
        if stream.getparams()[:3] != (2, 2, 44_100):
            raise ValueError(f"{path.name}: expected stereo PCM16 at 44100 Hz")
        frames = stream.getnframes()
        raw = stream.readframes(frames)
        if len(raw) != frames * 4:
            raise ValueError(f"{path.name}: truncated PCM data")
    return np.frombuffer(raw, dtype="<i2").reshape(-1, 2).astype(np.float64) / 32_768.0


def onset(samples: np.ndarray) -> int:
    mono = samples.mean(axis=1)
    threshold = max(float(np.abs(mono).max()) * 0.01, 1.0 / 32_768.0)
    indices = np.flatnonzero(np.abs(mono) >= threshold)
    return int(indices[0]) if indices.size else 0


def best_lag(source: np.ndarray, native: np.ndarray, maximum_lag: int) -> tuple[int, float]:
    source = source - source.mean()
    native = native - native.mean()
    size = 1 << (source.size + native.size - 1).bit_length()
    correlation = np.fft.irfft(np.fft.rfft(source, size) * np.conj(np.fft.rfft(native, size)), size)
    lags = np.arange(-maximum_lag, maximum_lag + 1)
    values = correlation[lags % size]
    denominator = float(np.linalg.norm(source) * np.linalg.norm(native))
    if denominator == 0.0:
        return 0, 0.0
    index = int(np.argmax(np.abs(values)))
    return int(lags[index]), float(values[index] / denominator)


def aligned(source: np.ndarray, native: np.ndarray, lag: int) -> tuple[np.ndarray, np.ndarray]:
    if lag > 0:
        source = source[lag:]
    elif lag < 0:
        native = native[-lag:]
    frames = max(len(source), len(native))
    source = np.pad(source, [(0, frames - len(source))] + [(0, 0)] * (source.ndim - 1))
    native = np.pad(native, [(0, frames - len(native))] + [(0, 0)] * (native.ndim - 1))
    return source, native


def outside_overlap(source: np.ndarray, native: np.ndarray, lag: int) -> tuple[float, float]:
    source_extent = max(len(source) - max(lag, 0), 0)
    native_extent = max(len(native) - max(-lag, 0), 0)
    overlap = min(source_extent, native_extent)
    mask = np.ones(len(source), dtype=bool)
    start = max(lag, 0)
    mask[start:start + overlap] = False
    energy = float(np.sum(source * source))
    outside_energy = float(np.sum(source[mask] * source[mask]))
    rms = float(np.sqrt(outside_energy / source.size)) if source.size else 0.0
    return rms, outside_energy / energy if energy else 0.0


def metrics(
    source: np.ndarray,
    native: np.ndarray,
    lag: int,
    allow_corrected: bool,
    normalization_scale: float = 1.0,
) -> dict[str, object]:
    outside_rms, outside_fraction = outside_overlap(source, native, lag)
    source, native = aligned(source, native, lag)
    native_energy = float(np.sum(native * native))
    gain = float(np.sum(source * native) / native_energy) if native_energy else 0.0
    difference = source - gain * native
    variable = np.std(source) > 0.0 and np.std(native) > 0.0
    correlation = float(np.corrcoef(source.ravel(), native.ravel())[0, 1]) if variable else 0.0
    unscaled_gain = gain * normalization_scale
    return {
        "lag_samples": lag,
        "least_squares_native_gain": gain if allow_corrected else None,
        "least_squares_native_gain_db": 20.0 * np.log10(abs(gain)) if allow_corrected and gain else None,
        "least_squares_unscaled_native_gain": unscaled_gain if allow_corrected else None,
        "least_squares_unscaled_native_gain_db": 20.0 * np.log10(abs(unscaled_gain)) if allow_corrected and unscaled_gain else None,
        "rmse": float(np.sqrt(np.mean(difference * difference))) if allow_corrected else None,
        "correlation": correlation if allow_corrected else None,
        "outside_overlap_rms": outside_rms,
        "outside_overlap_energy_fraction": outside_fraction,
    }


def envelopes(samples: np.ndarray) -> np.ndarray:
    mono = samples.mean(axis=1)
    block = 441
    padded = np.pad(mono, (0, (-len(mono)) % block))
    return np.sqrt(np.mean(padded.reshape(-1, block) ** 2, axis=1))


def file_summary(path: Path, samples: np.ndarray) -> dict[str, object]:
    return {
        "file": path.name,
        "sha256": sha256(path),
        "frames": len(samples),
        "duration_seconds": len(samples) / 44_100.0,
        "peak": float(np.abs(samples).max()),
        "rms": float(np.sqrt(np.mean(samples * samples))),
        "clipped_samples": int(np.count_nonzero(np.abs(samples) >= 32_767.0 / 32_768.0)),
    }


def main() -> int:
    if len(sys.argv) not in (5, 7) or (len(sys.argv) == 7 and sys.argv[5] != "--repeat"):
        print(__doc__.strip(), file=sys.stderr)
        return 2
    spec_path = Path(sys.argv[1])
    native_path = Path(sys.argv[2])
    reference_path = Path(sys.argv[3])
    output_path = Path(sys.argv[4])
    spec = json.loads(spec_path.read_text())
    if spec.get("schema") != SCHEMA or len(spec.get("hits", [])) != 256:
        raise ValueError("expected dense schema v2 with 256 hits")
    if sha256(native_path) != spec.get("native_render_sha256"):
        raise ValueError("native WAV checksum does not match the dense spec")
    for index, hit in enumerate(spec["hits"]):
        expected_tick = round(index * 1.536) if spec["variant"] == "stress" else index * 64
        if hit["note"] != 48 + hit["pad_index"] or hit["start_tick"] != expected_tick or hit["duration_ticks"] != 1:
            raise ValueError(f"dense hit {index} differs from the expected performance")

    native = read_wav(native_path)
    reference = read_wav(reference_path)
    clipped = int(np.count_nonzero(np.abs(reference) >= 32_767.0 / 32_768.0))
    onset_lag = onset(reference) - onset(native)
    lag, lag_correlation = best_lag(reference.mean(axis=1), native.mean(axis=1), MAX_LAG_SAMPLES)
    alignment_valid = abs(lag - onset_lag) <= 1
    corrected_valid = clipped == 0 and alignment_valid
    onset_aligned_reference, onset_aligned_native = aligned(reference, native, onset_lag)
    native_envelope = envelopes(onset_aligned_native)
    reference_envelope = envelopes(onset_aligned_reference)
    envelope_lag, envelope_lag_correlation = best_lag(reference_envelope, native_envelope, 2)
    envelope_alignment_valid = abs(envelope_lag) < 2
    envelope_metrics_valid = clipped == 0 and envelope_alignment_valid
    envelope_metrics = metrics(reference_envelope, native_envelope, envelope_lag, envelope_metrics_valid, spec["wav_normalization_scale"])
    report = {
        "schema": "midi-evil-chiptune-dense-comparison-v1",
        "variant": spec["variant"],
        "spec": {"file": spec_path.name, "sha256": sha256(spec_path)},
        "native": file_summary(native_path, native),
        "reference": file_summary(reference_path, reference),
        "native_normalization_scale": spec["wav_normalization_scale"],
        "lag_search_samples": [-MAX_LAG_SAMPLES, MAX_LAG_SAMPLES],
        "onset_lag_samples": onset_lag,
        "cross_correlation_lag_samples": lag,
        "cross_correlation_at_lag": lag_correlation,
        "alignment_valid": alignment_valid,
        "corrected_metrics_valid": corrected_valid,
        "raw_zero_lag": metrics(reference, native, 0, True, spec["wav_normalization_scale"]),
        "aligned": metrics(reference, native, lag, corrected_valid, spec["wav_normalization_scale"]),
        "envelope_alignment_valid": envelope_alignment_valid,
        "envelope_metrics_valid": envelope_metrics_valid,
        "envelope_10ms": dict(envelope_metrics, cross_correlation_at_lag=envelope_lag_correlation),
        "interpretation": "Allocator-stress diagnostic" if spec["variant"] == "stress" else "Below-eight-native-voices candidate synthesis comparison; source voice count unresolved; not sample equivalence",
    }
    if len(sys.argv) == 7:
        repeat_path = Path(sys.argv[6])
        repeat = read_wav(repeat_path)
        repeat_clipped = int(np.count_nonzero(np.abs(repeat) >= 32_767.0 / 32_768.0))
        repeat_lag, repeat_lag_correlation = best_lag(reference.mean(axis=1), repeat.mean(axis=1), MAX_LAG_SAMPLES)
        repeat_onset_lag = onset(reference) - onset(repeat)
        repeat_alignment_valid = abs(repeat_lag - repeat_onset_lag) <= 1
        repeat_aligned_reference, repeat_aligned = aligned(reference, repeat, repeat_onset_lag)
        reference_repeat_envelope = envelopes(repeat_aligned_reference)
        repeat_envelope = envelopes(repeat_aligned)
        repeat_envelope_lag, repeat_envelope_lag_correlation = best_lag(reference_repeat_envelope, repeat_envelope, 2)
        repeat_envelope_alignment_valid = abs(repeat_envelope_lag) < 2
        repeat_waveform_metrics_valid = clipped == 0 and repeat_clipped == 0 and repeat_alignment_valid
        repeat_envelope_metrics_valid = clipped == 0 and repeat_clipped == 0 and repeat_envelope_alignment_valid
        report["source_repeat"] = {
            "summary": file_summary(repeat_path, repeat),
            "alignment_valid": repeat_alignment_valid,
            "envelope_alignment_valid": repeat_envelope_alignment_valid,
            "comparison": metrics(reference, repeat, repeat_lag, repeat_waveform_metrics_valid),
            "cross_correlation_at_lag": repeat_lag_correlation,
            "envelope_10ms": dict(
                metrics(reference_repeat_envelope, repeat_envelope, repeat_envelope_lag, repeat_envelope_metrics_valid),
                cross_correlation_at_lag=repeat_envelope_lag_correlation,
            ),
        }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n")
    print(json.dumps({
        "variant": report["variant"],
        "alignment_valid": alignment_valid,
        "corrected_metrics_valid": corrected_valid,
        "lag_samples": lag,
        "onset_lag_samples": onset_lag,
        "envelope_10ms": report["envelope_10ms"],
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
