# Internal User Guide

## Standalone validation workflow

1. Launch `ghost-lab`.
2. Enter a WAV fixture or another WAV file path.
3. Select Live, High, or Maximum analysis.
4. Describe the intended result.
5. Select **Listen / Analyze / Propose**.
6. Inspect the semantic plan and before/after metrics.

The current laboratory uses a mock processor so the entire pipeline can be validated without FabFilter. Its EQ and compressor are intentionally generic and must not be treated as a sonic match for Pro-Q 4 or Pro-C 3.

## Quality profiles

### Live

- Lowest FFT density.
- Reduced history.
- Intended for realtime displays and rapid iteration.

### High

- Three FFT resolutions.
- Denser overlap.
- Suitable for interactive proposal work.

### Maximum

- Highest overlap and largest low-frequency FFT.
- Retains frame-level evidence.
- Intended for final evaluation and dataset creation.

Custom profiles can be supplied through `AnalysisConfig` in code or through `--analysis-config <file.toml>` in `ghost-cli`. The production UI should expose FFT sizes, overlap, retained frame data, resonance threshold, transient sensitivity, and true-peak oversampling through an advanced panel.

## Text-only model input

The prompt bundle contains:

- technical system prompt;
- user intent;
- analysis JSON serialized as text;
- plugin capability JSON serialized as text;
- strict output contract.

It contains no plot paths, images, screenshots, image URLs, or local image inputs. Plots under `artifacts/plots/` are exclusively for user and engineering inspection.

## Database records

The demo saves:

- capture metadata;
- analysis bundle;
- user intent;
- prompt bundle;
- agent run;
- validated mix plan;
- processed analysis.

The real plugin must additionally store child state snapshots and acceptance/revert decisions.

## DAW workflow after real child integration

1. Insert Ghost Agent Host on a mixer channel.
2. Load Pro-Q 4 and Pro-C 3 inside Ghost.
3. Open either child editor with Show/Hide controls.
4. Select a capture length and press Listen.
5. Play the representative DAW region.
6. Enter freeform intent or structured context.
7. Generate the proposal.
8. Audition Current and Proposed at matched level.
9. Accept, edit, or revert.
10. Save the DAW project; Ghost embeds accepted opaque child state in its own CLAP state.

## Internal daemon workflow

Start `ghost-agentd`, then send `health`, `analyze`, `propose`, or `stats` JSONL requests using `scripts/send_agentd_request.py`. See `DAEMON_API.md`.
