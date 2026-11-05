"""Shared NumPy-only PCM16 measurements for Phase 6 preset calibration."""

from __future__ import annotations

import hashlib
import math
from pathlib import Path
import wave

import numpy as np

BANDS_HZ = ((0, 200), (200, 1_000), (1_000, 4_000), (4_000, 8_000), (8_000, 16_000), (16_000, 24_000))
DEFAULT_WINDOWS = ((0.0, 0.05), (0.05, 0.2), (0.2, 0.5), (0.5, 1.0), (1.0, 2.0))
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
            raise ValueError(f"{path}: expected mono/stereo PCM16")
        raw = stream.readframes(stream.getnframes())
    samples = np.frombuffer(raw, dtype="<i2").astype(np.float64) / 32_768.0
    return sample_rate, samples.reshape(-1, channels)


def resample_linear(samples: np.ndarray, source_rate: int, target_rate: int) -> np.ndarray:
    if source_rate == target_rate or samples.shape[0] == 0:
        return samples.copy()
    old_time = np.arange(samples.shape[0], dtype=np.float64) / source_rate
    new_time = np.arange(round(samples.shape[0] * target_rate / source_rate), dtype=np.float64) / target_rate
    return np.column_stack([
        np.interp(new_time, old_time, samples[:, channel])
        for channel in range(samples.shape[1])
    ])


def rms(samples: np.ndarray) -> float:
    return float(np.sqrt(np.mean(samples * samples))) if samples.size else 0.0


def seconds_slice(samples: np.ndarray, sample_rate: int, start: float, end: float) -> np.ndarray:
    return samples[round(start * sample_rate):round(end * sample_rate)]


def block_envelope(samples: np.ndarray, sample_rate: int, block_seconds: float = 0.010) -> np.ndarray:
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
    first = int(active[0]) if active.size else None
    last = int(active[-1]) if active.size else None
    return {
        "onset_seconds": first * 0.001 if first is not None else None,
        "active_duration_seconds": (last + 1) * 0.001 if last is not None else 0.0,
        "last_active_seconds": (last + 1) * 0.001 if last is not None else None,
        "activity_threshold_rms": threshold,
    }


def spectrum_metrics(samples: np.ndarray, sample_rate: int) -> dict[str, object]:
    if samples.size == 0:
        return {"rms": 0.0, "spectral_centroid_hz": 0.0, "bands": {}}
    window = np.hanning(samples.size)
    magnitudes = np.abs(np.fft.rfft(samples * window))
    power = magnitudes * magnitudes
    frequencies = np.fft.rfftfreq(samples.size, 1.0 / sample_rate)
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
    return {
        "rms": rms(samples),
        "spectral_centroid_hz": float(np.sum(frequencies * magnitudes) / total_magnitude) if total_magnitude else 0.0,
        "bands": bands,
    }


def estimate_fundamental(samples: np.ndarray, sample_rate: int, expected_hz: float) -> dict[str, float | None]:
    if samples.size < 16 or expected_hz <= 0.0:
        return {"fundamental_hz": None, "tuning_cents": None}
    windowed = samples * np.hanning(samples.size)
    fft_size = 1 << max(14, (samples.size * 4 - 1).bit_length())
    magnitudes = np.abs(np.fft.rfft(windowed, fft_size))
    frequencies = np.fft.rfftfreq(fft_size, 1.0 / sample_rate)
    selected = np.flatnonzero(
        (frequencies >= expected_hz * 2.0 ** (-0.75 / 12.0))
        & (frequencies <= expected_hz * 2.0 ** (0.75 / 12.0))
    )
    if selected.size == 0:
        return {"fundamental_hz": None, "tuning_cents": None}
    index = int(selected[np.argmax(magnitudes[selected])])
    frequency = float(frequencies[index])
    if 0 < index < magnitudes.size - 1:
        left, center, right = np.log(magnitudes[index - 1:index + 2] + EPSILON)
        denominator = left - 2.0 * center + right
        if abs(denominator) > 1e-12:
            offset = float(np.clip(0.5 * (left - right) / denominator, -0.5, 0.5))
            frequency += offset * sample_rate / fft_size
    return {
        "fundamental_hz": frequency,
        "tuning_cents": float(1200.0 * np.log2(frequency / expected_hz)),
    }


def measure(path: Path, sample_rate: int, note_seconds: float, expected_hz: float,
            tail_windows: dict[str, tuple[float, float]],
            tuning_window: tuple[float, float] = (0.25, 0.75)) -> tuple[dict[str, object], np.ndarray]:
    source_rate, stereo = read_wav(path)
    stereo = resample_linear(stereo, source_rate, sample_rate)
    mono = stereo.mean(axis=1)
    held = seconds_slice(mono, sample_rate, 0.0, note_seconds)
    if held.size != round(note_seconds * sample_rate):
        raise ValueError(f"{path}: shorter than the {note_seconds:g}-second held-note window")
    activity = onset_and_activity(mono, sample_rate)
    active_start = round(float(activity["onset_seconds"] or 0.0) * sample_rate)
    active_end = round(float(activity["active_duration_seconds"]) * sample_rate)
    active = mono[active_start:active_end]
    measurement = {
        "file": path.name,
        "sha256": sha256(path),
        "source_format": {
            "channels": int(stereo.shape[1]),
            "encoding": "PCM16",
            "sample_rate_hz": source_rate,
        },
        "frames_at_analysis_rate": int(mono.size),
        "duration_seconds": float(mono.size / sample_rate),
        "peak": float(np.max(np.abs(mono), initial=0.0)),
        "full_file_rms": rms(mono),
        "active_rms": rms(active),
        "held_note_rms": rms(held),
        "held_note_spectrum": spectrum_metrics(held, sample_rate),
        "window_spectra": {
            f"{start:g}-{end:g}": spectrum_metrics(seconds_slice(mono, sample_rate, start, end), sample_rate)
            for start, end in DEFAULT_WINDOWS
        },
        "tuning": estimate_fundamental(
            seconds_slice(mono, sample_rate, *tuning_window), sample_rate, expected_hz
        ),
        "tail_rms": {
            name: rms(seconds_slice(mono, sample_rate, start, end))
            for name, (start, end) in tail_windows.items()
        },
        "stereo_difference_rms": rms(stereo[:, 0] - stereo[:, 1]) if stereo.shape[1] == 2 else 0.0,
        **activity,
    }
    measurement["post_gate_active_duration_seconds"] = max(
        0.0, float(measurement["active_duration_seconds"]) - note_seconds
    )
    return measurement, mono


def compare(reference: np.ndarray, native: np.ndarray, sample_rate: int, note_seconds: float) -> dict[str, object]:
    held_count = min(reference.size, native.size, round(note_seconds * sample_rate))
    full_count = max(reference.size, native.size)
    reference_full = np.pad(reference, (0, full_count - reference.size))
    native_full = np.pad(native, (0, full_count - native.size))
    reference_envelope = block_envelope(reference_full, sample_rate)
    native_envelope = block_envelope(native_full, sample_rate)
    scale = max(float(reference_envelope.max(initial=0.0)), EPSILON)
    envelope_db_error = 20.0 * np.log10(
        (native_envelope + scale * 1e-6) / (reference_envelope + scale * 1e-6)
    )
    return {
        "held_note_waveform_correlation": correlation(reference[:held_count], native[:held_count]),
        "held_note_10ms_envelope_correlation": correlation(
            reference_envelope[:math.ceil(note_seconds / 0.010)],
            native_envelope[:math.ceil(note_seconds / 0.010)],
        ),
        "full_tail_10ms_envelope_correlation": correlation(reference_envelope, native_envelope),
        "full_tail_10ms_envelope_rmse_db": float(np.sqrt(np.mean(envelope_db_error ** 2))),
        "full_tail_10ms_envelope_blocks": int(reference_envelope.size),
    }


def parse_window(value: str) -> tuple[float, float]:
    try:
        start_text, end_text = value.split(":", 1)
        start, end = float(start_text), float(end_text)
    except ValueError as error:
        raise ValueError("window must be START:END in seconds") from error
    if not math.isfinite(start) or not math.isfinite(end) or start < 0.0 or end <= start:
        raise ValueError("window requires 0 <= START < END")
    return start, end


def common_method(sample_rate: int, note_seconds: float,
                  tail_windows: dict[str, tuple[float, float]]) -> dict[str, object]:
    return {
        "analysis_sample_rate_hz": sample_rate,
        "resampling": "Linear interpolation of each PCM16 channel, then arithmetic stereo mean.",
        "held_note_seconds": note_seconds,
        "rms": "Full-scale PCM16 normalized by 32768; stereo mean unless stereo_difference_rms is named.",
        "spectrum": "Hann-windowed held-note and time-window FFTs. Centroid is magnitude weighted; bands use squared magnitude.",
        "bands_hz": [list(band) for band in BANDS_HZ],
        "envelope": "Non-overlapping 10 ms RMS blocks; Pearson correlation and log-envelope RMSE. Full-tail comparison zero-pads the shorter file and floors both envelopes at -120 dB relative to reference peak.",
        "onset_and_activity": "Non-overlapping 1 ms RMS blocks at least max(0.1% of peak block RMS, -80 dBFS).",
        "tuning": "Parabolic-interpolated zero-padded Hann FFT peak within +/-75 cents of the equal-tempered fundamental, measured at 0.25-0.75 s unless overridden.",
        "tail_windows_seconds": {name: list(window) for name, window in tail_windows.items()},
    }
