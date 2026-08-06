# Ghost Agent Daemon JSONL API

`ghost-agentd` is the local process boundary for SQLite, high-resolution analysis, prompt construction, and Codex. The current internal transport is newline-delimited JSON over loopback TCP. The protocol is intentionally small enough to replace with named pipes or Unix-domain sockets without changing domain messages.

Default address: `127.0.0.1:47644`

Every request occupies one line and receives one response line.

## Health

```json
{"method":"health"}
```

## Analyze

```json
{
  "method": "analyze",
  "path": "fixtures/muddy_bass.wav",
  "config": {
    "profile": "maximum",
    "fft_sizes": [2048, 8192, 32768],
    "hop_ratio": 0.125,
    "minimum_frequency_hz": 10.0,
    "maximum_frequency_hz": 24000.0,
    "resonance_threshold_db": 4.5,
    "transient_sensitivity": 2.3,
    "true_peak_oversample": 8,
    "retain_frame_series": true
  }
}
```

Omit `config` to use Maximum mode.

## Propose — freeform

```json
{
  "method": "propose",
  "path": "fixtures/muddy_bass.wav",
  "intent": {
    "mode": "freeform",
    "prompt": "Tighten the low mids while preserving punch."
  }
}
```

## Propose — structured

```json
{
  "method": "propose",
  "path": "fixtures/muddy_bass.wav",
  "intent": {
    "mode": "structured",
    "context": {
      "source": "bass",
      "role": "rhythmic anchor",
      "style": "house",
      "goal": "tight and controlled",
      "problem": "low-mid buildup",
      "intensity": "moderate",
      "preserve": ["initial transient", "sub weight"],
      "scope": ["eq", "compression"],
      "notes": "Do not make the bass smaller."
    }
  }
}
```

## Stats

```json
{"method":"stats"}
```

## Response envelope

Success:

```json
{"ok":true,"result":{},"error":null}
```

Failure:

```json
{"ok":false,"result":null,"error":"diagnostic text"}
```

The service never applies a plan to a realtime child plugin. It only returns a validated semantic plan. The future plugin-side executor owns state snapshots, public-parameter translation, smoothing, application, verification, and rollback.
