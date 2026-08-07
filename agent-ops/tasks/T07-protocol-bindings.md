# T07 — Generate Schemas and TypeScript Bindings

## Dispatch

- Branch from: PROTOCOL_SHA
- Parallel with: T04, T05, T06
- Produces: one input to CONTRACTS_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-07-bindings -b agent/07-bindings <PROTOCOL_SHA>
    Set-Location ..\gha-wt-07-bindings

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Make the Rust service contract consumable by a Svelte frontend through checked-in deterministic schemas and TypeScript bindings.

## Owned paths

- crates/ghost-protocol schema/export support
- schemas/service/**
- scripts/generate-protocol-bindings.*
- web/generated/**
- CI or repository checks dedicated to binding drift

Do not build a frontend.

## Required work

- Export JSON Schema from the protocol types.
- Generate TypeScript types without hand-maintained duplicates.
- Normalize output ordering and formatting for deterministic diffs.
- Add a drift check that fails when generated files are stale.
- Document the single regeneration command for PowerShell and CI.
- Add fixtures that exercise tagged unions, versioning, errors, progress, and cancellation.

## Acceptance

- Generation succeeds from a clean checkout.
- Running generation twice produces no diff.
- Rust fixtures validate against the schema and deserialize through the generated contract expectations.
- The checked-in generated directory contains a provenance header.
- Protocol and drift tests pass.

## Handoff

Make one commit named feat: generate service schemas and typescript bindings. Return its SHA, generator command, and files expected to conflict during reconciliation.
