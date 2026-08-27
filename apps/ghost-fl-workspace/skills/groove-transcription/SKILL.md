---
name: groove-transcription
description: Use isolated stem rhythm and pitch projections to propose editable MIDI while preserving timing uncertainty and validating a small section before scaling up.
tools:
  - ghost_audio_analyze
  - ghost_audio_read
  - fl_scripting_search
  - fl_scripting_describe
  - fl_scripting_call
---

# Groove transcription

1. Prefer isolated stems. A separated kick, snare, hat or bass stem supplies semantic role information that raw audio does not need to re-classify.
2. Establish BPM and time signature from producer context or high-confidence analysis before projecting events to the DAW grid.
3. For drums, read the rhythm view. Preserve meaningful microtiming offsets and velocity proxies instead of quantizing everything automatically.
4. For monophonic bass or lead stems, read the pitch view. Treat note events as proposals; low-confidence frames, octave ambiguity and slides need musical judgement.
5. Explain material uncertainty before writing MIDI.
6. Recreate a short representative section first (typically four to eight bars), then inspect/listen with the producer before repeating or expanding it.
7. Use exact FL scripting/Gopher primitives to create notes and patterns. Verify resulting note positions, lengths and target channel rather than assuming the mutation succeeded.
8. The goal is an editable musical reconstruction, not sample-perfect source separation or forensic transcription.
