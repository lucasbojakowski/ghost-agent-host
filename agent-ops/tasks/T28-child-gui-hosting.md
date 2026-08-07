# T28 — Host Child Plugin GUIs

## Dispatch

- Branch from: STATE_LATENCY_SHA, with CHILD_DISCOVERY_SHA ancestry
- Parallel work: none on native editor/window lifecycle
- Produces: CHILD_GUI_SHA

    Set-Location D:\konko\ghost\ghost-agent-host
    git worktree add ..\gha-wt-28-child-gui -b agent/28-child-gui <STATE_LATENCY_SHA>
    Set-Location ..\gha-wt-28-child-gui

Read agent-ops/WORKTREE_CONTRACT.md before changing files.

## Objective

Allow users to open supported child plugin GUIs with correct Windows parenting, scaling, resize negotiation, and teardown while preserving either parent editor provider.

## Owned paths

- Child GUI extension/window-host modules
- Parent editor integration points dedicated to child windows
- GUI lifecycle harness and Windows smoke documentation

## Required work

- Query child GUI APIs/capabilities and select a compatible Windows API.
- Create, parent, show, hide, resize, focus, and destroy child GUI windows on the required thread.
- Keep child GUI lifetime subordinate to child instance and parent plugin lifetime.
- Handle unsupported GUI, refused resize, DPI changes, parent hide/reopen, project reload, and editor-provider changes.
- Add fake-child GUI lifecycle tests.
- Run optional FL Studio validation with available Pro-Q/Pro-C installations and record versions/IDs.

## Acceptance

- Unsupported GUI degrades without affecting audio.
- Repeated open/hide/reopen/destroy cycles do not leave orphan windows.
- Parent plugin/editor destruction closes every child window before unloading its library.
- Both egui and WebView parent variants remain buildable.
- Fake GUI tests pass and FL-specific manual results are documented when available.

## Handoff

Make one commit named feat: host child clap plugin guis. Return its SHA, supported window API, lifecycle results, and optional FL Studio evidence. The coordinator records it as CHILD_GUI_SHA.
