# Analysis and Runtime Configuration

The analyzer is designed around explicit, versioned profiles. `Maximum` is the internal research default; custom TOML can override every analysis field.

## Built-in profiles

| Profile | FFT sizes | Hop ratio | Frame evidence | Intended use |
|---|---:|---:|---|---|
| Live | 1,024 / 4,096 | 0.50 | No | Realtime telemetry and rapid checks |
| High | 2,048 / 8,192 / 16,384 | 0.25 | Yes | Interactive Listen/proposal work |
| Maximum | 2,048 / 8,192 / 32,768 | 0.125 | Yes | Research evaluation and dataset generation |

Maximum mode uses multiple analysis resolutions so low-frequency discrimination is not traded directly against transient timing. It retains frame centroid evidence and uses the densest overlap of the built-in modes.

## Custom TOML

`config/default.toml` is a complete example:

```toml
[analysis]
profile = "maximum"
fft_sizes = [2048, 8192, 32768]
hop_ratio = 0.125
minimum_frequency_hz = 10.0
maximum_frequency_hz = 24000.0
resonance_threshold_db = 4.5
transient_sensitivity = 2.3
true_peak_oversample = 8
retain_frame_series = true
```

The CLI accepts the file directly:

```bash
cargo run -p ghost-cli -- analyze \
  --input fixtures/muddy_bass.wav \
  --analysis-config config/default.toml \
  --output artifacts/muddy-analysis.json
```

When a file is supplied, the persisted profile is marked `custom` so results remain distinguishable from built-in defaults.

## Quality controls

- **FFT sizes:** powers of two. Larger windows improve low-frequency bin spacing but reduce temporal localization.
- **Hop ratio:** fraction of the FFT window advanced per frame. Lower values mean greater overlap, more computation, and denser evidence.
- **Frequency range:** bounds finding and reporting. Values above Nyquist are naturally unavailable for the current sample rate.
- **Resonance threshold:** prominence required before a spectral concentration becomes a candidate.
- **Transient sensitivity:** scales transient detection threshold.
- **True-peak oversample:** records the requested quality intent. The current standards-oriented loudness implementation delegates true-peak calculation to `ebur128`; future analyzers can use the field to select explicit kernels.
- **Retain frame series:** stores frame-level evidence. Disable to reduce prompt/database size.

## Prompt payload rule

Plots are never part of `PromptBundle`. The bundle contains only:

1. System prompt text.
2. User intent JSON serialized as text.
3. Analysis JSON serialized as text.
4. Plugin capability JSON serialized as text.
5. Output contract text.

Plot PNGs remain in `artifacts/plots/` and `artifacts/mock-evaluation/` for human visualization.
