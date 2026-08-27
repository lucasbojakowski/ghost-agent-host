---
name: reference-production-session
description: Compose reference analysis, project planning, FL scaffolding, capture, arrangement planning and groove transcription into a guided producer-agent workflow.
tools:
  - workspace_skill_read
  - workspace_project_get
  - workspace_project_set
  - workspace_plan_get
  - workspace_plan_set
  - ghost_audio_analyze
  - ghost_audio_read
  - ghost_audio_compare
---

# Reference production session

This is the coordinating skill for the current human-guided client workflow. Load the narrower skills when their phase begins rather than copying all their details into every turn.

## Sequence

1. **Establish project context.** Read the workspace project, reference/stem paths, producer description, BPM/time signature hints, and current FL project state.
2. **Reference analysis.** Load `reference-analysis`. Analyze the full mix and relevant stems, then distinguish deterministic measurements, musical projections, and producer-supplied semantic information.
3. **Production architecture.** Before touching FL, describe the reference architecture and write an initial Production Plan covering channel roles, playlist organization, mixer intentions, section hypotheses, timbral targets and open questions.
4. **Project scaffold.** After producer approval, load `project-scaffold` and create only the agreed FL structure. Verify each group of mutations.
5. **Arrangement.** Load `arrangement-planning` to convert reference timelines and the producer description into section/bar ranges, playlist markers and production intentions.
6. **Live measurements when needed.** Load `fl-audio-capture` whenever the question depends on the current rendered signal inside FL rather than the supplied reference files. Follow its arm/play/collect order exactly.
7. **MIDI and groove work.** Load `groove-transcription` for isolated bass/drum stems. Build a short section, validate with the producer, then expand.
8. **Iterate with the human.** This workflow is intentionally guided. Do not rush from reference analysis into large DAW mutations when a production decision is still open.

The Production Plan is semantic intent, the audio analyses are evidence, and the live FL surfaces are authoritative execution state. Keep those layers distinct.
