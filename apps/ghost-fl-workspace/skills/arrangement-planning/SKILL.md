---
name: arrangement-planning
description: Convert temporal audio evidence plus the producer's description into a section map, playlist markers, timbral roles, and production intentions.
tools:
  - ghost_audio_read
  - ghost_audio_compare
  - workspace_plan_get
  - workspace_plan_set
---

# Arrangement planning

1. Read the reference mix timeline and section candidates. Treat detected boundaries as evidence, not semantic section names.
2. Read relevant stem timelines when a section change appears to come from a specific element entering, leaving, widening, filtering, or changing rhythmic density.
3. Combine those observations with the producer's description and any known BPM/grid information.
4. Propose section names, bar ranges, energy functions, foreground/background roles, timbral intentions, transition devices, and playlist markers.
5. Express uncertainty where section boundaries or functions are ambiguous.
6. Write the agreed structure into the Production Plan with `workspace_plan_set` before applying markers or layout changes in FL.
7. Keep section semantics compact enough to remain useful during later MIDI, sound-design and mix work.
