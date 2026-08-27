---
name: project-scaffold
description: Turn an approved production plan into a prepared FL Studio project structure while preserving unrelated live state and verifying each structural mutation.
tools:
  - workspace_plan_get
  - fl_scripting_search
  - fl_scripting_describe
  - fl_scripting_call
---

# Project scaffold

Use this skill after reference analysis and before detailed MIDI/sample work.

1. Read the current Production Plan and inspect live FL state.
2. Reconcile the proposed channels, playlist tracks, mixer inserts, routing, markers, and naming with what already exists.
3. Present any destructive or ambiguous structural choice before applying it.
4. Prefer the transparent FL surfaces already proven in this workspace. Discover exact scripting functions when needed; use raw Gopher schemas exactly when Gopher is the better primitive.
5. Create the scaffold incrementally: channels first, then mixer structure/routing, then playlist organization/markers.
6. Preserve unrelated channels, effects, routing, playlist material, and user state unless the producer explicitly asks for replacement.
7. After each structural group, inspect the live project and verify the result rather than assuming the calls produced the intended topology.
8. Keep the Production Plan semantic object as intent; FL remains authoritative for what actually exists.
