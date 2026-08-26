//! Validated externally-owned FL bootstrap.
//!
//! Proven path on `runtime-v1-working-script`:
//! `scripts/fl_init.ps1` -> set WebView2 CDP flag -> start FL Studio ->
//! wait until `http://localhost:9222/json` contains Gopher -> start runtime.
//!
//! This mode deliberately does not launch FL Studio and does not attempt to
//! open Gopher. It attaches to the already-active session, reuses the runtime's
//! existing window/Gopher checks, then enters the shared app readiness tail.

use std::time::Duration;

use anyhow::{Context, Result};
#[cfg(not(windows))]
use anyhow::bail;
use serde_json::json;

use super::{probe_gopher, wait_for_fl_window, Runtime, RuntimePhase};

#[cfg(windows)]
use super::discover_fl_pid;

pub(super) fn run(runtime: &Runtime) -> Result<()> {
    runtime.transition(
        RuntimePhase::DiscoveringFl,
        "fl.discovery_started",
        json!({"bootstrap": "active"}),
    )?;

    let fl_pid = discover_active_fl_pid()?;
    runtime.journal.append(
        "fl.process",
        "fl.attached",
        "info",
        json!({"pid": fl_pid, "bootstrap": "active"}),
    )?;
    runtime.record_fl_process(fl_pid, false)?;

    runtime.transition(
        RuntimePhase::WaitingFlUi,
        "fl.waiting_for_window",
        json!({"pid": fl_pid, "bootstrap": "active"}),
    )?;
    wait_for_fl_window(fl_pid, Duration::from_secs(30))?;
    runtime.mark_fl_window_ready(fl_pid)?;

    runtime.transition(
        RuntimePhase::WaitingGopher,
        "gopher.probing",
        json!({"debugPort": runtime.cli.debug_port, "bootstrap": "active"}),
    )?;
    let manifest = probe_gopher(
        runtime.cli.debug_port,
        &runtime.cli.target_match,
        Duration::from_secs(2),
    )
    .context(
        "active bootstrap requires an already-visible Gopher session; start FL through scripts/fl_init.ps1 and open Gopher before launching the runtime, or use --bootstrap start to exercise runtime-owned startup",
    )?;

    runtime.complete_bootstrap(manifest)
}

#[cfg(windows)]
fn discover_active_fl_pid() -> Result<u32> {
    discover_fl_pid()?.context(
        "active bootstrap requires an existing FL64.exe process; start FL through scripts/fl_init.ps1 before launching the runtime",
    )
}

#[cfg(not(windows))]
fn discover_active_fl_pid() -> Result<u32> {
    bail!("active FL bootstrap currently requires Windows")
}
