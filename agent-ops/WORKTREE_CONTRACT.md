# Mandatory Worktree and Handoff Contract

Every task agent must follow this contract.

## Before editing

1. Start in `D:\konko\ghost\ghost-agent-host`.
2. Resolve the task's checkpoint placeholder from `CHECKPOINTS.md` or the coordinator. Replace the entire angle-bracket token in the sample command with the literal commit SHA; angle brackets are not valid PowerShell argument syntax.
3. Confirm the proposed branch and sibling worktree path do not already exist.
4. Run the exact `git worktree add` command in the task brief.
5. Change into the new worktree and perform all reads, edits, builds, and commits there.

If the checkpoint is unavailable, the branch exists, the destination is occupied, or the source repository is unexpectedly dirty, stop and report the blocker. Do not improvise a different base.

## Scope rules

- One task, one agent, one focused commit.
- Modify only the owned paths named in the task.
- Do not edit the coordinator's original worktree.
- Do not merge, rebase, cherry-pick, push, or delete worktrees.
- Do not format unrelated files.
- Do not change public contracts established by an earlier task without reporting a blocker.
- Preserve existing compatibility unless the task explicitly authorizes removal.
- Keep Codex, filesystem, database, network, WebView, and plugin-host work off UI and audio threads.
- Never add allocation, locking, logging, JSON parsing, filesystem access, or IPC to the audio callback.

## Verification

Run the task-specific checks. Also run, when the task changes Rust:

```powershell
cargo fmt -p <changed-package> -- --check
cargo clippy -p <changed-package> --all-targets -- -D warnings
cargo test -p <changed-package>
git diff --check
```

If a task changes multiple packages, run the commands for each affected package. Do not conceal failing workspace checks.

## Commit

```powershell
git status --short
git diff --check
git add <owned-paths-only>
git commit -m "<type>: <single task outcome>"
git status --short
git rev-parse HEAD
```

The final status must be clean.

## Required handoff

Return exactly these sections:

```text
Task:
Branch:
Worktree:
Commit:
Outcome:
Files changed:
Public API or schema changes:
Tests and results:
Known limitations:
Reconciliation notes:
```

Do not describe uncommitted work as complete.
