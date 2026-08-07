# Coordinator Reconciliation Guide

## Before dispatch

Commit this operations packet and record that commit as `OPS_SHA`. T00 must branch from that exact commit.

## Handoff review

For every returned task:

1. Confirm the branch and worktree match the brief.
2. Confirm the worktree is clean.
3. Inspect `git show --stat <commit>` and `git show <commit>`.
4. Reject unrelated formatting, hidden contract changes, generated secrets, personal paths, and binary build artifacts.
5. Confirm the reported tests from the agent's worktree.

## Integration procedure

Use the dedicated integration branch. Cherry-pick one reviewed commit at a time:

```powershell
Set-Location D:\konko\ghost\ghost-agent-host
git switch integration/ui-platform
git cherry-pick <agent-commit>
```

Resolve source conflicts intentionally. `Cargo.lock` conflicts are mechanical: reconcile manifests first, then regenerate the lockfile with Cargo rather than hand-merging dependency entries.

After each wave:

```powershell
cargo check --workspace
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Additional boundary checks after T12:

```powershell
cargo tree -p ghost-ui
```

`ghost-ui` must not depend on `ghost-codex`, `ghost-host`, or `ghost-db`.

After successful reconciliation, update `CHECKPOINTS.md`, commit the checkpoint update, and use that exact commit for the next wave.

## Cleanup

Only after the commit is reconciled and verified:

```powershell
git worktree remove ..\gha-wt-<task>
git branch -d agent/<task>
git worktree prune
```

Never remove a worktree containing uncommitted work.
