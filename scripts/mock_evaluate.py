#!/usr/bin/env python3
"""Independent executable evaluation of the example MixPlan.

This is deliberately separate from the Rust mock DSP so the sandbox can produce
an audible before/after fixture and numerical report even without rustc/cargo.
It is not intended to model FabFilter internals.
"""
from __future__ import annotations

from pathlib import Path
import json
import math
import numpy as np
import soundfile as sf
from scipy import signal
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures/muddy_bass.wav"
PLAN_PATH = ROOT / "artifacts/examples/mix_plan.json"
OUT = ROOT / "artifacts/mock-evaluation"
OUT.mkdir(parents=True, exist_ok=True)


def peaking_coefficients(sr: float, frequency: float, gain_db: float, q: float):
    a = 10.0 ** (gain_db / 40.0)
    omega = 2.0 * math.pi * frequency / sr
    alpha = math.sin(omega) / (2.0 * max(q, 0.05))
    cos = math.cos(omega)
    a0 = 1.0 + alpha / a
    return (
        (1.0 + alpha * a) / a0,
        (-2.0 * cos) / a0,
        (1.0 - alpha * a) / a0,
        (-2.0 * cos) / a0,
        (1.0 - alpha / a) / a0,
    )


def apply_eq(audio: np.ndarray, sr: int, settings: dict) -> np.ndarray:
    if not settings.get("enabled", True) or settings.get("shape") != "bell":
        return audio
    b0, b1, b2, a1, a2 = peaking_coefficients(
        sr,
        float(settings["frequency_hz"]),
        float(settings["gain_db"]),
        float(settings["q"]),
    )
    return signal.lfilter([b0, b1, b2], [1.0, a1, a2], audio, axis=0)


def apply_compressor(audio: np.ndarray, sr: int, settings: dict) -> np.ndarray:
    if not settings.get("enabled", True):
        return audio
    attack = math.exp(-1.0 / (max(float(settings["attack_ms"]), 0.01) * 0.001 * sr))
    release = math.exp(-1.0 / (max(float(settings["release_ms"]), 1.0) * 0.001 * sr))
    threshold = float(settings["threshold_db"])
    ratio = max(float(settings["ratio"]), 1.0)
    knee = max(float(settings["knee_db"]), 0.01)
    maximum_reduction = abs(float(settings["range_db"]))
    wet = np.clip(float(settings["mix_percent"]) / 100.0, 0.0, 1.0)
    makeup = 10.0 ** (float(settings["output_gain_db"]) / 20.0)
    out = np.empty_like(audio)
    envelope = 0.0
    gain = 1.0
    for frame in range(audio.shape[0]):
        detector = float(np.max(np.abs(audio[frame])))
        coefficient = attack if detector > envelope else release
        envelope = coefficient * envelope + (1.0 - coefficient) * detector
        level_db = 20.0 * math.log10(max(envelope, 1.0e-20))
        over = level_db - threshold
        if over <= -knee * 0.5:
            target_gain_db = 0.0
        elif over >= knee * 0.5:
            target_gain_db = -min(over - over / ratio, maximum_reduction)
        else:
            x = over + knee * 0.5
            compressed = x * x / (2.0 * knee)
            target_gain_db = -min(compressed - compressed / ratio, maximum_reduction)
        target_gain = 10.0 ** (target_gain_db / 20.0)
        gain = 0.95 * gain + 0.05 * target_gain
        processed = audio[frame] * gain * makeup
        out[frame] = audio[frame] * (1.0 - wet) + processed * wet
    return out


def metrics(audio: np.ndarray, sr: int) -> dict:
    mono = audio.mean(axis=1)
    peak = float(np.max(np.abs(audio)))
    rms = float(np.sqrt(np.mean(mono * mono)))
    n = 32768
    freq, psd = signal.welch(mono, sr, window="hann", nperseg=n, noverlap=n // 2)
    mag = np.sqrt(np.maximum(psd, 1e-30))
    centroid = float(np.sum(freq * mag) / np.sum(mag))
    bands = {}
    for key, (lo, hi) in {
        "sub_db": (20, 60), "bass_db": (60, 150), "low_mid_db": (150, 500),
        "mid_db": (500, 2000), "high_mid_db": (2000, 5000),
        "presence_db": (5000, 10000), "air_db": (10000, 22000),
    }.items():
        mask = (freq >= lo) & (freq < hi)
        bands[key] = float(10 * np.log10(max(np.sum(psd[mask]), 1e-30)))
    left, right = audio[:, 0], audio[:, min(1, audio.shape[1] - 1)]
    denom = math.sqrt(float(np.sum(left * left) * np.sum(right * right)))
    return {
        "peak_dbfs": 20 * math.log10(max(peak, 1e-20)),
        "rms_dbfs": 20 * math.log10(max(rms, 1e-20)),
        "crest_factor_db": 20 * math.log10(max(peak, 1e-20)) - 20 * math.log10(max(rms, 1e-20)),
        "spectral_centroid_hz": centroid,
        "bands": bands,
        "stereo_correlation": float(np.sum(left * right) / max(denom, 1e-30)),
    }


audio, sr = sf.read(SOURCE, always_2d=True, dtype="float64")
plan = json.loads(PLAN_PATH.read_text())
processed = audio.copy()
for operation in plan["operations"]:
    if operation["operation"] == "eq_band":
        processed = apply_eq(processed, sr, operation["settings"])
    elif operation["operation"] == "compressor":
        processed = apply_compressor(processed, sr, operation["settings"])

# Prevent the independent validation render from writing invalid floating PCM.
processed = np.clip(processed, -1.25, 1.25)
sf.write(OUT / "muddy_bass-processed.wav", processed.astype(np.float32), sr, subtype="FLOAT")

before = metrics(audio, sr)
after = metrics(processed, sr)
deltas = {
    "rms_db": after["rms_dbfs"] - before["rms_dbfs"],
    "crest_factor_db": after["crest_factor_db"] - before["crest_factor_db"],
    "spectral_centroid_hz": after["spectral_centroid_hz"] - before["spectral_centroid_hz"],
    "low_mid_db": after["bands"]["low_mid_db"] - before["bands"]["low_mid_db"],
    "stereo_correlation": after["stereo_correlation"] - before["stereo_correlation"],
}
report = {
    "schema_version": "ghost.mock-evaluation/1",
    "source": str(SOURCE.relative_to(ROOT)),
    "plan": str(PLAN_PATH.relative_to(ROOT)),
    "renderer": "independent-python-biquad-compressor",
    "disclaimer": "Evaluation renderer is a neutral approximation and does not emulate FabFilter internals.",
    "before": before,
    "after": after,
    "deltas": deltas,
    "expectations": {
        "low_mid_decreased": deltas["low_mid_db"] < -0.5,
        "no_peak_over_plus_2_dBFS": after["peak_dbfs"] < 2.0,
        "stereo_correlation_preserved": abs(deltas["stereo_correlation"]) < 0.02,
    },
}
(OUT / "evaluation.json").write_text(json.dumps(report, indent=2) + "\n")

# Human-only plot; never copied into PromptBundle.
freq, before_psd = signal.welch(audio.mean(axis=1), sr, nperseg=32768)
_, after_psd = signal.welch(processed.mean(axis=1), sr, nperseg=32768)
fig, ax = plt.subplots(figsize=(10, 5))
ax.semilogx(freq[1:], 10 * np.log10(np.maximum(before_psd[1:], 1e-30)), label="Before")
ax.semilogx(freq[1:], 10 * np.log10(np.maximum(after_psd[1:], 1e-30)), label="Processed")
ax.axvspan(150, 500, alpha=0.12, label="Low-mid evaluation band")
ax.set_xlim(20, 22000)
ax.set_xlabel("Frequency (Hz)")
ax.set_ylabel("Power spectral density (dB)")
ax.set_title("Independent mock processing evaluation")
ax.grid(True, which="both", alpha=0.25)
ax.legend()
fig.tight_layout()
fig.savefig(OUT / "before-after-spectrum.png", dpi=150)
plt.close(fig)
print(json.dumps(report["deltas"], indent=2))
