# Documentation Map

Ghost & Guild has accumulated design notes, experiment prompts and runtime investigation records across several architecture phases. Use this map to avoid treating historical planning text as current implementation guidance.

## Current sources of truth

Read these first for current work:

1. `PROVEN_BASELINES.md` — accepted runtime baselines and exact validated commits.
2. `SDK_ARCHITECTURE.md` — Core/SDK vs app ownership.
3. `FL_CAPABILITY_SURFACES.md` — Gopher, scripting, Codex and MCP capability topology.
4. `agent-work/WORKSPACE_FOUNDATION.md` — scope/status of the current integration phase.
5. relevant validation record under `agent-work/`.

## Durable architecture decisions

- `decisions/001-transparent-fl-studio-adapter.md`
- promoted integration architecture under `agent-work/FL_SCRIPTING_FRAMEWORK.md`

These remain useful when they do not conflict with later live-proven baseline records.

## Historical design sources

The following documents explain how the project reached the current architecture but are not execution prompts for current work:

- `TECHNICAL_RETROSPECTIVE.md`
- `WORKSPACE_MIGRATION_PLAN.md`
- `FL_SCRIPTING_JOURNEY.md`
- `ideas/`

They preserve important failures, constraints and product reasoning. Prefer current baseline/SDK documents when terminology or planned repository structure has changed since they were written.

## Agent-work directory

`agent-work/` should contain current architecture briefs and validation records, not completed one-shot execution prompts.

Completed scripting bridge/framework and MCP implementation prompts have been removed from the active phase tree after their work was validated. Git history remains the archive for those prompts and the superseded app-local scripting bridge briefs.

## DAW API evidence

`daw-apis/` contains checked-in reverse-engineering/runtime documentation used as capability evidence. Treat these as descriptions of external surfaces, not Ghost product policy.
