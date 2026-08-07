# T13 — Migrate the CLI to the Application Controller

## Dispatch

- Branch from: IMPLEMENTATION_SHA
- Parallel with: T14, T17, T18
- Produces: CLI_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-13-cli -b agent/13-cli <IMPLEMENTATION_SHA>
    Set-Location ..\gha-wt-13-cli

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Use the same application controller and providers from the CLI while preserving existing commands and output compatibility.

## Owned paths

- CLI crate and CLI-specific tests/fixtures
- CLI documentation

Do not edit daemon or editor implementations.

## Required work

- Compose the concrete agent, render, and repository providers into the controller.
- Route existing commands through FrontendApi or a narrow controller facade.
- Preserve exit codes and machine-readable output.
- Add a diagnostic command that reports provider availability and Codex resolver details without starting a job.
- Handle progress and cancellation appropriately for a terminal.

## Acceptance

- Existing documented CLI flows remain compatible.
- Diagnostic mode is safe and side-effect-light.
- Ctrl+C cancels active work and performs bounded cleanup.
- Fake-provider end-to-end CLI tests cover success and failure.
- CLI-specific and workspace checks pass.

## Handoff

Make one commit named refactor: route cli through application controller. Return its SHA. The coordinator records it as CLI_SHA.
