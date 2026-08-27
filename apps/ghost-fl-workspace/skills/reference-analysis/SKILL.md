---
name: reference-analysis
description: Analyze a reference mix and separated stems as evidence for production decisions without conflating measurements, musical inference, and creative interpretation.
tools:
  - ghost_audio_analyze
  - ghost_audio_read
  - ghost_audio_compare
  - workspace_project_get
  - workspace_project_set
---

# Reference analysis

Use this skill before project scaffolding when the producer supplies a reference mix and stems.

1. Read the current workspace project and establish the intended reference mix, stem roles, description, tempo hints, and any known section information.
2. Analyze the full reference mix first with `ghost_audio_analyze`.
3. Analyze the stems that matter to the immediate production question. Do not dump every analysis view into context by default.
4. Use `ghost_audio_read` progressively: acoustic for global signal evidence, timeline/sections for arrangement, rhythm for groove, and pitch for isolated monophonic parts such as bass when available.
5. Use `ghost_audio_compare` when the relationship between a stem and the mix matters more than either absolute measurement.
6. Keep three evidence layers explicit in reasoning:
   - **measurement:** deterministic values produced by Ghost;
   - **musical inference:** tempo, onset, section, pitch or groove projections with confidence/limitations;
   - **producer description / creative interpretation:** semantic information supplied by the human or inferred by the model.
7. Do not translate one acoustic number directly into a genre/timbre claim. Use several measurements plus stem role and the producer's description.
8. Before mutating FL, summarize the production architecture you believe the reference implies and identify uncertainties worth resolving.
