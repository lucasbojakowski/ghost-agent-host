#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import json
import math
import numpy as np
import soundfile as sf
from scipy import signal
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"
OUT = ROOT / "artifacts" / "reference-analysis"
PLOTS = ROOT / "artifacts" / "plots"
OUT.mkdir(parents=True, exist_ok=True)
PLOTS.mkdir(parents=True, exist_ok=True)

BANDS = {
    "sub_db": (20, 60),
    "bass_db": (60, 150),
    "low_mid_db": (150, 500),
    "mid_db": (500, 2000),
    "high_mid_db": (2000, 5000),
    "presence_db": (5000, 10000),
    "air_db": (10000, 22000),
}


def to_db(value: float) -> float:
    return 20 * math.log10(max(float(value), 1e-20))


def corr(a: np.ndarray, b: np.ndarray) -> float:
    den = np.sqrt(np.sum(a * a) * np.sum(b * b))
    return float(np.sum(a * b) / max(den, 1e-30))


def analyze(path: Path) -> dict:
    audio, sr = sf.read(path, always_2d=True, dtype="float64")
    mono = audio.mean(axis=1)
    peak = np.max(np.abs(audio))
    rms = np.sqrt(np.mean(mono**2))
    nperseg = 32768 if len(mono) >= 32768 else 8192
    freq, time, zxx = signal.stft(mono, fs=sr, window="hann", nperseg=nperseg, noverlap=int(nperseg * 0.875), boundary=None, padded=False)
    mag = np.abs(zxx) + 1e-20
    avg = np.mean(mag, axis=1)
    centroid_frames = np.sum(freq[:, None] * mag, axis=0) / np.sum(mag, axis=0)
    cumulative = np.cumsum(avg)
    rolloff = float(freq[min(np.searchsorted(cumulative, cumulative[-1] * 0.85), len(freq) - 1)])
    flatness = float(np.exp(np.mean(np.log(avg))) / np.mean(avg))
    flux = np.sqrt(np.sum(np.maximum(np.diff(mag, axis=1), 0) ** 2, axis=0)) / mag.shape[0]
    bands = {}
    for name, (lo, hi) in BANDS.items():
        mask = (freq >= lo) & (freq < hi)
        bands[name] = float(10 * np.log10(max(np.sum(avg[mask] ** 2), 1e-30)))

    env = np.abs(signal.hilbert(mono))
    smooth = signal.sosfilt(signal.butter(2, 35 / (sr / 2), output="sos"), env)
    deriv = np.maximum(np.diff(smooth), 0)
    threshold = deriv.mean() + 2.5 * deriv.std()
    peaks, props = signal.find_peaks(deriv, height=threshold, distance=int(sr * 0.02))

    left = audio[:, 0]
    right = audio[:, min(1, audio.shape[1] - 1)]
    low_sos = signal.butter(2, 200 / (sr / 2), btype="low", output="sos")
    left_low = signal.sosfilt(low_sos, left)
    right_low = signal.sosfilt(low_sos, right)
    left_high = left - left_low
    right_high = right - right_low
    mid = 0.5 * (left + right)
    side = 0.5 * (left - right)

    local = signal.medfilt(to_db_array(avg), kernel_size=31)
    prominence = to_db_array(avg) - local
    resonance_idx, _ = signal.find_peaks(prominence, height=5.0, distance=5)
    top = sorted(resonance_idx, key=lambda i: prominence[i], reverse=True)[:12]

    result = {
        "schema_version": "ghost.reference-analysis/1",
        "file": path.name,
        "sample_rate": int(sr),
        "channels": int(audio.shape[1]),
        "frames": int(audio.shape[0]),
        "duration_seconds": float(audio.shape[0] / sr),
        "integrity": {
            "sample_peak_dbfs": to_db(peak),
            "clipped_samples": int(np.sum(np.abs(audio) >= 1.0)),
            "dc_offset": [float(x) for x in audio.mean(axis=0)],
            "silence_ratio": float(np.mean(np.abs(audio) <= 10 ** (-90 / 20))),
        },
        "loudness_proxy": {
            "rms_dbfs": to_db(rms),
            "crest_factor_db": to_db(peak) - to_db(rms),
        },
        "spectrum": {
            "centroid_hz": float(np.mean(centroid_frames)),
            "rolloff_85_hz": rolloff,
            "flatness": flatness,
            "flux_mean": float(np.mean(flux)) if flux.size else 0.0,
            "bands": bands,
            "resonances": [
                {"frequency_hz": float(freq[i]), "prominence_db": float(prominence[i])}
                for i in top
            ],
        },
        "dynamics": {
            "transient_density_hz": float(len(peaks) / (len(mono) / sr)),
            "attack_strength_p90": float(np.quantile(props.get("peak_heights", np.array([0.0])), 0.9)),
            "envelope_variability_db": float(np.quantile(to_db_array(smooth), 0.9) - np.quantile(to_db_array(smooth), 0.1)),
        },
        "stereo": {
            "broadband_correlation": corr(left, right),
            "low_band_correlation": corr(left_low, right_low),
            "high_band_correlation": corr(left_high, right_high),
            "mid_side_ratio_db": float(10 * np.log10(max(np.mean(mid**2), 1e-30) / max(np.mean(side**2), 1e-30))),
        },
    }

    fig = plt.figure(figsize=(10, 5))
    ax = fig.add_subplot(111)
    ax.semilogx(freq[1:], to_db_array(avg[1:]))
    ax.set_xlim(20, min(22000, sr / 2))
    ax.set_xlabel("Frequency (Hz)")
    ax.set_ylabel("Relative magnitude (dB)")
    ax.set_title(path.stem)
    ax.grid(True, which="both", alpha=0.25)
    fig.tight_layout()
    fig.savefig(PLOTS / f"{path.stem}-spectrum.png", dpi=140)
    plt.close(fig)
    return result


def to_db_array(values: np.ndarray) -> np.ndarray:
    return 20 * np.log10(np.maximum(values, 1e-20))


summary = {}
for path in sorted(FIXTURES.glob("*.wav")):
    result = analyze(path)
    summary[path.name] = result
    (OUT / f"{path.stem}.json").write_text(json.dumps(result, indent=2) + "\n")

(OUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(f"analysed {len(summary)} fixtures")
