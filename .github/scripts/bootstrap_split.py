from pathlib import Path

root = Path('.')
main_path = root / 'apps/ghost-fl-runtime/src/main.rs'
active_path = root / 'apps/ghost-fl-runtime/src/bootstrap_active.rs'
start_path = root / 'apps/ghost-fl-runtime/src/bootstrap_start.rs'

text = main_path.read_text()

module_anchor = 'use uuid::Uuid;\n\n'
module_insert = 'use uuid::Uuid;\n\nmod bootstrap_active;\nmod bootstrap_start;\n\n'
if module_insert not in text:
    if module_anchor not in text:
        raise SystemExit('module insertion anchor not found')
    text = text.replace(module_anchor, module_insert, 1)

cli_anchor = '''    #[arg(long)]
    no_webviews: bool,

    #[arg(long, value_enum, default_value_t = AppProfile::Workspace)]
    app: AppProfile,
'''
cli_replacement = '''    #[arg(long)]
    no_webviews: bool,

    #[arg(long, value_enum, default_value_t = BootstrapMode::Active)]
    bootstrap: BootstrapMode,

    #[arg(long, value_enum, default_value_t = AppProfile::Workspace)]
    app: AppProfile,
'''
if cli_replacement not in text:
    if cli_anchor not in text:
        raise SystemExit('CLI bootstrap insertion anchor not found')
    text = text.replace(cli_anchor, cli_replacement, 1)

enum_anchor = '''#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum AppProfile {
'''
enum_insert = '''#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum BootstrapMode {
    Active,
    Start,
}

impl BootstrapMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Start => "start",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum AppProfile {
'''
if enum_insert not in text:
    if enum_anchor not in text:
        raise SystemExit('BootstrapMode insertion anchor not found')
    text = text.replace(enum_anchor, enum_insert, 1)

bootstrap_start = text.index('    fn bootstrap(&self) -> Result<()> {')
start_app = text.index('    fn start_app(&self) -> Result<()> {', bootstrap_start)
shared_bootstrap = '''    fn bootstrap(&self) -> Result<()> {
        self.journal.append(
            "runtime",
            "bootstrap.selected",
            "info",
            json!({"mode": self.cli.bootstrap.as_str()}),
        )?;
        match self.cli.bootstrap {
            BootstrapMode::Active => bootstrap_active::run(self),
            BootstrapMode::Start => bootstrap_start::run(self),
        }
    }

    fn record_fl_process(&self, pid: u32, launched_by_ghost: bool) -> Result<()> {
        {
            let mut state = self.state()?;
            state.fl.pid = Some(pid);
            state.fl.launched_by_ghost = launched_by_ghost;
        }
        self.persist_state()
    }

    fn mark_fl_window_ready(&self, pid: u32) -> Result<()> {
        {
            let mut state = self.state()?;
            state.fl.window_ready = true;
        }
        self.persist_state()?;
        self.journal.append(
            "fl.process",
            "fl.window_ready",
            "info",
            json!({"pid": pid}),
        )
    }

    fn complete_bootstrap(&self, manifest: ghost_fl_studio::FlStudioManifest) -> Result<()> {
        {
            let mut state = self.state()?;
            state.fl.gopher_ready = true;
            state.fl.gopher_target = Some(manifest.target_title.clone());
            state.fl.gopher_tool_count = Some(manifest.tools.len());
        }
        self.persist_state()?;
        self.journal.append(
            "fl.gopher",
            "gopher.ready",
            "info",
            json!({"target": manifest.target_title, "toolCount": manifest.tools.len()}),
        )?;

        if self.cli.no_app {
            self.transition(RuntimePhase::Ready, "runtime.ready", json!({"app": "none"}))?;
            return Ok(());
        }
        self.start_app()?;
        self.wait_for_app_ready()?;
        self.open_app_webview();
        self.transition(
            RuntimePhase::Ready,
            "runtime.ready",
            json!({"app": self.cli.app.spec().package}),
        )
    }

'''
text = text[:bootstrap_start] + shared_bootstrap + text[start_app:]

# Keep a stable test around the new strategy boundary.
test_anchor = '''    #[test]
    fn registered_apps_are_closed_and_stable() {
'''
test_insert = '''    #[test]
    fn bootstrap_modes_are_machine_stable() {
        assert_eq!(BootstrapMode::Active.as_str(), "active");
        assert_eq!(BootstrapMode::Start.as_str(), "start");
    }

    #[test]
    fn registered_apps_are_closed_and_stable() {
'''
if test_insert not in text:
    if test_anchor not in text:
        raise SystemExit('test insertion anchor not found')
    text = text.replace(test_anchor, test_insert, 1)

main_path.write_text(text)

active_path.write_text(r'''//! Validated externally-owned FL bootstrap.
//!
//! Proven path on `runtime-v1-working-script`:
//! `scripts/fl_init.ps1` -> set WebView2 CDP flag -> start FL Studio ->
//! wait until `http://localhost:9222/json` contains Gopher -> start runtime.
//!
//! This mode deliberately does not launch FL Studio and does not attempt to
//! open Gopher. It attaches to the already-active session, reuses the runtime's
//! existing window/Gopher checks, then enters the shared app readiness tail.

use std::time::Duration;

use anyhow::{bail, Context, Result};
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
''')

start_path.write_text(r'''//! Runtime-owned FL bootstrap experiment.
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
''')
