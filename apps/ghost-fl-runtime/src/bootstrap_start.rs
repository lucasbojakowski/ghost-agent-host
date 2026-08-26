//! Runtime-owned FL bootstrap experiment.
//!
//! This is the counterpart to `bootstrap_active`: the runtime discovers or
//! launches FL Studio itself, waits for the main window, probes Gopher, and on
//! failure optionally sends the one-shot Alt+F1 activation before probing
//! again. This path is intentionally kept separate because live testing has
//! shown it to be less reliable than the validated `fl_init.ps1` flow.

use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::json;

use super::{ensure_fl, probe_gopher, try_open_gopher, wait_for_fl_window, Runtime, RuntimePhase};

pub(super) fn run(runtime: &Runtime) -> Result<()> {
    runtime.transition(
        RuntimePhase::DiscoveringFl,
        "fl.discovery_started",
        json!({"bootstrap": "start"}),
    )?;
    let (fl_pid, launched_by_ghost) = ensure_fl(&runtime.cli, &runtime.journal)?;
    runtime.record_fl_process(fl_pid, launched_by_ghost)?;

    runtime.transition(
        RuntimePhase::WaitingFlUi,
        "fl.waiting_for_window",
        json!({"pid": fl_pid, "bootstrap": "start"}),
    )?;
    wait_for_fl_window(fl_pid, Duration::from_secs(30))?;
    runtime.mark_fl_window_ready(fl_pid)?;

    runtime.transition(
        RuntimePhase::WaitingGopher,
        "gopher.probing",
        json!({"debugPort": runtime.cli.debug_port, "bootstrap": "start"}),
    )?;
    let quick_probe = probe_gopher(
        runtime.cli.debug_port,
        &runtime.cli.target_match,
        Duration::from_secs(2),
    );
    let manifest = match quick_probe {
        Ok(manifest) => manifest,
        Err(initial_error) if !runtime.cli.no_auto_gopher => {
            runtime.transition(
                RuntimePhase::OpeningGopher,
                "gopher.open_attempt",
                json!({"initialError": initial_error.to_string(), "bootstrap": "start"}),
            )?;
            if !try_open_gopher(fl_pid)? {
                runtime.journal.append(
                    "fl.gopher",
                    "gopher.open_shortcut_failed",
                    "warn",
                    json!({"pid": fl_pid, "bootstrap": "start"}),
                )?;
            }
            runtime.transition(
                RuntimePhase::WaitingGopher,
                "gopher.waiting_after_open",
                json!({"bootstrap": "start"}),
            )?;
            probe_gopher(
                runtime.cli.debug_port,
                &runtime.cli.target_match,
                Duration::from_secs(runtime.cli.gopher_timeout_seconds),
            )
            .with_context(|| {
                if launched_by_ghost {
                    "Gopher did not become ready after the one-shot Alt+F1 activation"
                } else {
                    "attached FL instance did not expose a usable Gopher/CDP target; restart FL through Ghost if it was launched without WebView2 debugging"
                }
            })?
        }
        Err(error) => {
            return Err(
                error.context("Gopher is not ready and automatic activation is disabled")
            )
        }
    };

    runtime.complete_bootstrap(manifest)
}
