#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import json
import numpy as np
import soundfile as sf
from scipy import signal

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "fixtures"
OUT.mkdir(parents=True, exist_ok=True)
SR = 48_000
DURATION = 12.0
N = int(SR * DURATION)
T = np.arange(N, dtype=np.float64) / SR
RNG = np.random.default_rng(1337)


def db_gain(db: float) -> float:
    return 10.0 ** (db / 20.0)


def kick_train(bpm: float = 124.0, amp: float = 0.75) -> np.ndarray:
    out = np.zeros(N, dtype=np.float64)
    interval = 60.0 / bpm
    for onset in np.arange(0.25, DURATION, interval):
        start = int(onset * SR)
        length = min(int(0.36 * SR), N - start)
        tt = np.arange(length, dtype=np.float64) / SR
        phase = 2 * np.pi * (70 * tt - 28 * tt**2)
        body = np.sin(phase) * np.exp(-tt * 14)
        click = RNG.normal(0, 1, length) * np.exp(-tt * 90)
        out[start:start + length] += amp * (0.88 * body + 0.12 * click)
    return out


def bass_line(amp: float = 0.32, extra_low_mid: float = 0.0) -> np.ndarray:
    base = np.sin(2 * np.pi * 55 * T) + 0.35 * np.sin(2 * np.pi * 110 * T)
    notes = 0.65 + 0.35 * signal.square(2 * np.pi * (124 / 60 / 2) * T, duty=0.72)
    line = amp * base * notes
    if extra_low_mid:
        line += extra_low_mid * np.sin(2 * np.pi * 230 * T) * notes
    return line


def pad(amp: float = 0.12) -> np.ndarray:
    noise = RNG.normal(0, 1, N)
    b, a = signal.butter(4, [700 / (SR / 2), 9000 / (SR / 2)], btype="band")
    filtered = signal.sosfilt(signal.butter(4, [700 / (SR / 2), 9000 / (SR / 2)], btype="band", output="sos"), noise)
    shimmer = 0.5 + 0.5 * np.sin(2 * np.pi * 0.13 * T)
    return amp * filtered / (np.std(filtered) + 1e-12) * shimmer


def normalize(stereo: np.ndarray, peak_db: float = -1.0) -> np.ndarray:
    peak = np.max(np.abs(stereo))
    return stereo * (db_gain(peak_db) / max(peak, 1e-12))


def stereoize(mono: np.ndarray, width: float = 0.08) -> np.ndarray:
    side = np.roll(mono, 17) - mono
    left = mono + width * side
    right = mono - width * side
    return np.column_stack([left, right])


fixtures: dict[str, np.ndarray] = {}
clean = kick_train() + bass_line() + pad()
fixtures["clean_reference.wav"] = normalize(stereoize(clean, 0.10), -1.0)

muddy = kick_train(amp=0.68) + bass_line(amp=0.35, extra_low_mid=0.27) + pad(0.09)
fixtures["muddy_bass.wav"] = normalize(stereoize(muddy, 0.06), -0.8)

harsh = clean + 0.11 * np.sin(2 * np.pi * 3200 * T) * (0.45 + 0.55 * signal.square(2 * np.pi * 1.2 * T, duty=0.35))
fixtures["harsh_presence.wav"] = normalize(stereoize(harsh, 0.11), -1.0)

phase_base = clean
left = phase_base + 0.15 * pad(0.8)
right = np.roll(phase_base, 23) - 0.20 * signal.sosfilt(signal.butter(2, 6000 / (SR / 2), btype="high", output="sos"), pad(0.8))
fixtures["phasey_wide.wav"] = normalize(np.column_stack([left, right]), -1.2)

crushed = stereoize(kick_train(amp=0.9) + bass_line(0.38) + pad(0.08), 0.07)
crushed = np.tanh(crushed * 4.0) / np.tanh(4.0)
fixtures["crushed_drums.wav"] = normalize(crushed, -0.2)

quiet = np.zeros((N, 2), dtype=np.float64)
quiet[int(4 * SR):] = normalize(stereoize(clean[: N - int(4 * SR)], 0.05), -8.0)
fixtures["silence_then_signal.wav"] = quiet

for name, audio in fixtures.items():
    sf.write(OUT / name, audio.astype(np.float32), SR, subtype="FLOAT")

manifest = {
    "schema_version": "ghost.fixture-manifest/1",
    "sample_rate": SR,
    "duration_seconds": DURATION,
    "seed": 1337,
    "fixtures": {
        "clean_reference.wav": ["balanced synthetic dance-music reference"],
        "muddy_bass.wav": ["excess persistent 230 Hz low-mid energy"],
        "harsh_presence.wav": ["intermittent 3.2 kHz concentration"],
        "phasey_wide.wav": ["inter-channel delay and high-frequency anti-correlation"],
        "crushed_drums.wav": ["low crest and nonlinear peak limiting"],
        "silence_then_signal.wav": ["capture sufficiency and silence handling"],
    },
}
(OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"generated {len(fixtures)} fixtures in {OUT}")
