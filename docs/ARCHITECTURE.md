# Architecture

## Runtime components

```text
DAW
└── Ghost outer CLAP
    ├── realtime capture taps
    ├── nested Pro-Q 4 slot
    ├── nested Pro-C 3 slot
    ├── child window management
    └── local daemon/control bridge

Standalone/internal validation
├── ghost-lab native UI
├── ghost-ui shared egui application surface
├── ghost-cli
├── ghost-core analyzer and contracts
├── ghost-codex agent adapters
├── ghost-host processing adapters
└── ghost-db SQLite/artifact persistence
```

## Current backend implementations

### Mock backend

Complete and deterministic. It implements bell EQ and feed-forward compression, enabling full before/after evaluation without proprietary software.

### CLAP backend boundary

The outer CLAP shell and descriptor scanner are present. Real nested processing is intentionally not faked because it requires lifecycle, thread, GUI, latency, state, and parameter behavior from the installed child plugin.

## Data flow

```text
WAV or captured frame
→ AnalysisConfig
→ Rust AnalysisBundle
→ text-only PromptBundle
→ Mock agent or Codex App Server
→ MixPlan
→ deterministic validation
→ Mock chain or hosted FabFilter chain
→ processed frame
→ Rust re-analysis
→ evaluation record
→ SQLite + artifact store
```

## Persistence rule

SQLite stores history and intelligence data. The eventual outer CLAP state must independently embed the accepted child states needed to restore the DAW project sound.

## Realtime rule

SQLite, Codex, JSON parsing, FFT analysis, GUI work, plugin scanning, and filesystem I/O stay outside the audio callback. The callback only forwards child audio/events and writes to bounded preallocated capture buffers.
