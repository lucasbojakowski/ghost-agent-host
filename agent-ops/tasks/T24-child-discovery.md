# T24 — Discover and Instantiate Child CLAP Plugins

## Dispatch

- Branch from: EDITOR_SELECTION_SHA
- Do not overlap plugin implementation with: T22
- Produces: CHILD_DISCOVERY_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-24-child-discovery -b agent/24-child-discovery <EDITOR_SELECTION_SHA>
    Set-Location ..\gha-wt-24-child-discovery

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Add reliable CLAP discovery, selection, loading, and instantiation for intended child effects such as Pro-Q and Pro-C. Do not forward live audio yet.

## Owned paths

- Dedicated child-host/discovery modules or crate
- Plugin composition files required to own child instances
- Discovery fixtures and tests

## Required work

- Scan explicit configured paths plus documented Windows CLAP locations.
- Cache descriptors without loading arbitrary binaries on the audio thread.
- Select plugins by stable CLAP identifiers, never display names alone.
- Load libraries and instantiate children off the audio callback.
- Validate plugin features and audio-effect suitability.
- Define activation/deactivation/destruction ownership and error reporting.
- Test with fake CLAP libraries; make commercial-plugin smoke tests optional.

## Acceptance

- Missing, duplicate, invalid-architecture, and load-failing plugins are distinguishable.
- No plugin scanning or library loading occurs in process().
- A fake child can be discovered and instantiated repeatedly without leaks.
- Optional Pro-Q/Pro-C smoke procedure records actual discovered IDs without hardcoding guesses.
- Tests pass without commercial plugins installed.

## Handoff

Make one commit named feat: discover and instantiate child clap plugins. Return its SHA, configured search policy, and observed optional plugin IDs. The coordinator records it as CHILD_DISCOVERY_SHA.
