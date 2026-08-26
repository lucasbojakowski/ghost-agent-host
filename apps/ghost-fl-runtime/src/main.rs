use std::collections::VecDeque;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, ValueEnum};
use ghost_fl_studio::{FlStudioAdapterConfig, GopherNativeAdapter};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

mod bootstrap_active;
mod bootstrap_start;

const INDEX_HTML: &str = include_str!("../web/index.html");
const MAX_HTTP_BYTES: usize = 2 * 1024 * 1024;
const MAX_RECENT_EVENTS: usize = 500;

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-runtime",
    about = "Single-session FL Studio lifecycle supervisor for Ghost applications"
)]
struct Cli {
    #[arg(long, default_value = r"D:\Image-Line\FL Studio 2026\FL64.exe")]
    fl_executable: PathBuf,

    #[arg(long, default_value_t = 9222)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[arg(long, default_value = "127.0.0.1:48750")]
    bind: String,

    #[arg(long)]
    no_webviews: bool,

    #[arg(long, value_enum, default_value_t = BootstrapMode::Active)]
    bootstrap: BootstrapMode,

    #[arg(long, value_enum, default_value_t = AppProfile::Workspace)]
    app: AppProfile,

    #[arg(long)]
    no_app: bool,

    #[arg(long)]
    app_bind: Option<String>,

    #[arg(long, default_value = "127.0.0.1:48766")]
    scripting_bind: String,

    #[arg(long, default_value = "codex")]
    codex_binary: String,

    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,

    #[arg(long, default_value = "cargo")]
    cargo_binary: String,

    #[arg(long)]
    app_binary: Option<PathBuf>,

    #[arg(long)]
    no_build_app: bool,

    #[arg(long)]
    no_launch: bool,

    #[arg(long)]
    no_auto_gopher: bool,

    #[arg(long, default_value_t = 45)]
    gopher_timeout_seconds: u64,

    #[arg(long, default_value_t = 45)]
    app_timeout_seconds: u64,

    #[arg(long, default_value_t = 30)]
    scripting_timeout_seconds: u64,

    #[arg(long)]
    state_dir: Option<PathBuf>,

    #[arg(long)]
    shutdown_fl_on_exit: bool,

    #[arg(long = "i-accept-live-fl-writes")]
    i_accept_live_fl_writes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
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
    Workspace,
    Agent,
}

impl AppProfile {
    fn spec(self) -> AppSpec {
        match self {
            Self::Workspace => AppSpec {
                package: "ghost-fl-workspace",
                display_name: "Workspace",
                default_bind: "127.0.0.1:48775",
                requires_scripting: true,
            },
            Self::Agent => AppSpec {
                package: "ghost-fl-agent",
                display_name: "Raw Agent",
                default_bind: "127.0.0.1:48765",
                requires_scripting: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimePhase {
    Booting,
    DiscoveringFl,
    WaitingFlUi,
    WaitingGopher,
    OpeningGopher,
    StartingApp,
    WaitingApp,
    WaitingScripting,
    Ready,
    Degraded,
    Failed,
    Stopping,
    Stopped,
}

impl RuntimePhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Booting => "booting",
            Self::DiscoveringFl => "discovering_fl",
            Self::WaitingFlUi => "waiting_fl_ui",
            Self::WaitingGopher => "waiting_gopher",
            Self::OpeningGopher => "opening_gopher",
            Self::StartingApp => "starting_app",
            Self::WaitingApp => "waiting_app",
            Self::WaitingScripting => "waiting_scripting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FlState {
    pid: Option<u32>,
    launched_by_ghost: bool,
    window_ready: bool,
    debug_port: u16,
    gopher_ready: bool,
    gopher_target: Option<String>,
    gopher_tool_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    profile: String,
    pid: Option<u32>,
    endpoint: String,
    healthy: bool,
    scripting_connected: Option<bool>,
    thread_id: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeState {
    session_id: String,
    phase: RuntimePhase,
    started_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    fl: FlState,
    app: AppState,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEvent {
    sequence: u64,
    timestamp_unix_ms: u64,
    component: String,
    event: String,
    severity: String,
    data: Value,
}

#[derive(Clone)]
struct EventJournal {
    inner: Arc<Mutex<JournalInner>>,
}

struct JournalInner {
    sequence: u64,
    recent: VecDeque<RuntimeEvent>,
    events_path: PathBuf,
}

impl EventJournal {
    fn new(events_path: PathBuf) -> Result<Self> {
        if let Some(parent) = events_path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(JournalInner {
                sequence: 0,
                recent: VecDeque::with_capacity(MAX_RECENT_EVENTS),
                events_path,
            })),
        })
    }

    fn append(&self, component: &str, event: &str, severity: &str, data: Value) -> Result<()> {
        let mut inner = lock(&self.inner, "event journal")?;
        inner.sequence = inner.sequence.saturating_add(1);
        let record = RuntimeEvent {
            sequence: inner.sequence,
            timestamp_unix_ms: unix_ms(),
            component: component.to_owned(),
            event: event.to_owned(),
            severity: severity.to_owned(),
            data,
        };
        let line = serde_json::to_string(&record)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inner.events_path)
            .with_context(|| format!("failed to append {}", inner.events_path.display()))?;
        writeln!(file, "{line}")?;
        if inner.recent.len() == MAX_RECENT_EVENTS {
            inner.recent.pop_front();
        }
        inner.recent.push_back(record);
        Ok(())
    }

    fn recent(&self) -> Result<Vec<RuntimeEvent>> {
        Ok(lock(&self.inner, "event journal")?
            .recent
            .iter()
            .cloned()
            .collect())
    }
}

#[derive(Clone, Copy)]
struct AppSpec {
    package: &'static str,
    display_name: &'static str,
    default_bind: &'static str,
    requires_scripting: bool,
}

#[derive(Clone)]
struct Runtime {
    cli: Arc<Cli>,
    state: Arc<Mutex<RuntimeState>>,
    app_child: Arc<Mutex<Option<Child>>>,
    runtime_ui_child: Arc<Mutex<Option<Child>>>,
    app_ui_child: Arc<Mutex<Option<Child>>>,
    bootstrap_active: Arc<AtomicBool>,
    journal: EventJournal,
    session_path: PathBuf,
    shutdown: Arc<AtomicBool>,
}

impl Runtime {
    fn new(cli: Cli) -> Result<Self> {
        if !cli.no_app && !cli.i_accept_live_fl_writes {
            bail!(
                "registered Ghost FL apps can perform live writes; pass --i-accept-live-fl-writes or use --no-app"
            );
        }
        let session_id = Uuid::new_v4().to_string();
        let state_root = cli.state_dir.clone().unwrap_or_else(default_state_root);
        let runtime_dir = state_root.join("runtime");
        let logs_dir = state_root.join("logs");
        fs::create_dir_all(&runtime_dir)?;
        fs::create_dir_all(&logs_dir)?;
        let session_path = runtime_dir.join("session.json");
        let journal = EventJournal::new(logs_dir.join(format!("{session_id}.jsonl")))?;
        let spec = cli.app.spec();
        let app_bind = cli
            .app_bind
            .clone()
            .unwrap_or_else(|| spec.default_bind.to_owned());
        let now = unix_ms();
        let state = RuntimeState {
            session_id: session_id.clone(),
            phase: RuntimePhase::Booting,
            started_at_unix_ms: now,
            updated_at_unix_ms: now,
            fl: FlState {
                pid: None,
                launched_by_ghost: false,
                window_ready: false,
                debug_port: cli.debug_port,
                gopher_ready: false,
                gopher_target: None,
                gopher_tool_count: None,
            },
            app: AppState {
                profile: if cli.no_app {
                    "none".into()
                } else {
                    format!("{:?}", cli.app).to_lowercase()
                },
                pid: None,
                endpoint: app_bind,
                healthy: false,
                scripting_connected: (!cli.no_app && spec.requires_scripting).then_some(false),
                thread_id: None,
                last_error: None,
            },
            last_error: None,
        };
        let runtime = Self {
            cli: Arc::new(cli),
            state: Arc::new(Mutex::new(state)),
            app_child: Arc::new(Mutex::new(None)),
            runtime_ui_child: Arc::new(Mutex::new(None)),
            app_ui_child: Arc::new(Mutex::new(None)),
            bootstrap_active: Arc::new(AtomicBool::new(false)),
            journal,
            session_path,
            shutdown: Arc::new(AtomicBool::new(false)),
        };
        runtime.persist_state()?;
        runtime.journal.append(
            "runtime",
            "runtime.created",
            "info",
            json!({"sessionId": session_id}),
        )?;
        Ok(runtime)
    }

    fn bootstrap(&self) -> Result<()> {
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
        self.journal
            .append("fl.process", "fl.window_ready", "info", json!({"pid": pid}))
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

    fn start_app(&self) -> Result<()> {
        if self.cli.no_app {
            bail!("runtime was started with --no-app");
        }
        {
            let state = self.state()?;
            if state.fl.pid.is_none() || !state.fl.gopher_ready {
                bail!(
                    "FL Studio and Gopher must be ready before starting the registered Ghost app"
                );
            }
        }
        self.stop_app(false)?;
        let spec = self.cli.app.spec();
        self.transition(
            RuntimePhase::StartingApp,
            "app.starting",
            json!({"package": spec.package}),
        )?;
        let binary = self.resolve_app_binary(spec)?;
        let app_bind = self.state()?.app.endpoint.clone();
        let mut command = Command::new(&binary);
        command
            .arg("--debug-port")
            .arg(self.cli.debug_port.to_string())
            .arg("--target-match")
            .arg(&self.cli.target_match)
            .arg("--bind")
            .arg(&app_bind)
            .arg("--scripting-bind")
            .arg(&self.cli.scripting_bind)
            .arg("--codex-binary")
            .arg(&self.cli.codex_binary)
            .arg("--model")
            .arg(&self.cli.model)
            .arg("--i-accept-live-fl-writes")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start {} at {}", spec.package, binary.display()))?;
        let pid = child.id();
        if let Some(stdout) = child.stdout.take() {
            spawn_output_reader(stdout, self.journal.clone(), "stdout");
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_output_reader(stderr, self.journal.clone(), "stderr");
        }
        *lock(&self.app_child, "app child")? = Some(child);
        {
            let mut state = self.state()?;
            state.app.pid = Some(pid);
            state.app.healthy = false;
            state.app.thread_id = None;
            state.app.last_error = None;
            if spec.requires_scripting {
                state.app.scripting_connected = Some(false);
            }
        }
        self.persist_state()?;
        self.journal.append(
            "app",
            "app.started",
            "info",
            json!({"package": spec.package, "pid": pid, "endpoint": app_bind}),
        )?;
        Ok(())
    }

    fn resolve_app_binary(&self, spec: AppSpec) -> Result<PathBuf> {
        if let Some(path) = &self.cli.app_binary {
            if !path.is_file() {
                bail!("configured app binary does not exist: {}", path.display());
            }
            return Ok(path.clone());
        }
        if !self.cli.no_build_app {
            let status = Command::new(&self.cli.cargo_binary)
                .arg("build")
                .arg("-p")
                .arg(spec.package)
                .status()
                .with_context(|| format!("failed to invoke cargo for {}", spec.package))?;
            if !status.success() {
                bail!("cargo build -p {} failed with {status}", spec.package);
            }
        }
        let filename = format!("{}{}", spec.package, env::consts::EXE_SUFFIX);
        let path = env::current_dir()?
            .join("target")
            .join("debug")
            .join(filename);
        if !path.is_file() {
            bail!(
                "registered app binary is unavailable at {}; run from the workspace root, build it first, or pass --app-binary",
                path.display()
            );
        }
        Ok(path)
    }

    fn wait_for_app_ready(&self) -> Result<()> {
        let spec = self.cli.app.spec();
        self.transition(
            RuntimePhase::WaitingApp,
            "app.waiting_for_health",
            json!({"endpoint": self.state()?.app.endpoint.clone()}),
        )?;
        let deadline = Instant::now() + Duration::from_secs(self.cli.app_timeout_seconds);
        loop {
            self.ensure_app_still_running()?;
            let endpoint = self.state()?.app.endpoint.clone();
            if let Ok(info) = http_get_json(&endpoint, "/api/info") {
                let thread_id = info
                    .get("threadId")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                {
                    let mut state = self.state()?;
                    state.app.healthy = true;
                    state.app.thread_id = thread_id.clone();
                }
                self.persist_state()?;
                self.journal.append(
                    "app",
                    "app.healthy",
                    "info",
                    json!({"threadId": thread_id}),
                )?;
                break;
            }
            if Instant::now() >= deadline {
                bail!(
                    "{} did not become healthy before timeout",
                    spec.display_name
                );
            }
            thread::sleep(Duration::from_millis(300));
        }
        if !spec.requires_scripting {
            return Ok(());
        }
        self.transition(
            RuntimePhase::WaitingScripting,
            "scripting.waiting",
            json!({}),
        )?;
        let deadline = Instant::now() + Duration::from_secs(self.cli.scripting_timeout_seconds);
        loop {
            self.ensure_app_still_running()?;
            let endpoint = self.state()?.app.endpoint.clone();
            if let Ok(status) = http_get_json(&endpoint, "/api/scripting/status") {
                let connected = status
                    .get("connected")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                {
                    let mut state = self.state()?;
                    state.app.scripting_connected = Some(connected);
                }
                self.persist_state()?;
                if connected {
                    self.journal
                        .append("fl.scripting", "scripting.connected", "info", status)?;
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                bail!("FL MIDI Scripting bridge did not connect before timeout");
            }
            thread::sleep(Duration::from_millis(300));
        }
    }

    fn ensure_app_still_running(&self) -> Result<()> {
        let exited = {
            let mut slot = lock(&self.app_child, "app child")?;
            let child = slot
                .as_mut()
                .ok_or_else(|| anyhow!("registered app process is not running"))?;
            match child.try_wait()? {
                Some(status) => {
                    let pid = child.id();
                    *slot = None;
                    Some((pid, status))
                }
                None => None,
            }
        };
        if let Some((pid, status)) = exited {
            {
                let mut state = self.state()?;
                state.app.pid = None;
                state.app.healthy = false;
                state.app.thread_id = None;
                state.app.last_error = Some(format!("registered app exited with {status}"));
                if state.app.scripting_connected.is_some() {
                    state.app.scripting_connected = Some(false);
                }
            }
            self.persist_state()?;
            self.journal.append(
                "app",
                "app.exited",
                "error",
                json!({"pid": pid, "status": status.to_string()}),
            )?;
            bail!("registered app exited with {status}");
        }
        Ok(())
    }

    fn stop_app(&self, record: bool) -> Result<()> {
        let mut slot = lock(&self.app_child, "app child")?;
        if let Some(mut child) = slot.take() {
            let pid = child.id();
            if let Err(error) = child.kill() {
                self.journal.append(
                    "app",
                    "app.kill_failed",
                    "warn",
                    json!({"pid": pid, "error": error.to_string()}),
                )?;
            }
            if let Err(error) = child.wait() {
                self.journal.append(
                    "app",
                    "app.wait_failed",
                    "warn",
                    json!({"pid": pid, "error": error.to_string()}),
                )?;
            }
            if record {
                self.journal
                    .append("app", "app.stopped", "info", json!({"pid": pid}))?;
            }
        }
        {
            let mut state = self.state()?;
            state.app.pid = None;
            state.app.healthy = false;
            state.app.thread_id = None;
            if state.app.scripting_connected.is_some() {
                state.app.scripting_connected = Some(false);
            }
        }
        self.persist_state()
    }

    fn restart_app(&self) -> Result<()> {
        self.stop_app(true)?;
        self.start_app()?;
        self.wait_for_app_ready()?;
        self.open_app_webview();
        self.transition(RuntimePhase::Ready, "app.restart_complete", json!({}))
    }

    fn refresh_health(&self) -> Result<()> {
        if self.cli.no_app {
            return Ok(());
        }
        if self.state()?.app.pid.is_none() {
            return Ok(());
        }
        if let Err(error) = self.ensure_app_still_running() {
            self.mark_degraded(error.to_string())?;
            return Ok(());
        }
        let endpoint = self.state()?.app.endpoint.clone();
        let info = http_get_json(&endpoint, "/api/info");
        let scripting = http_get_json(&endpoint, "/api/scripting/status");
        let healthy = info.is_ok();
        let scripting_connected = scripting
            .as_ref()
            .ok()
            .and_then(|value| value.get("connected"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let thread_id = info.ok().and_then(|value| {
            value
                .get("threadId")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        });
        let spec = self.cli.app.spec();
        let ready = healthy && (!spec.requires_scripting || scripting_connected);
        let previous_phase = {
            let mut state = self.state()?;
            state.app.healthy = healthy;
            state.app.scripting_connected = spec.requires_scripting.then_some(scripting_connected);
            state.app.thread_id = thread_id;
            let previous = state.phase;
            if matches!(state.phase, RuntimePhase::Ready | RuntimePhase::Degraded) {
                state.phase = if ready {
                    RuntimePhase::Ready
                } else {
                    RuntimePhase::Degraded
                };
            }
            state.updated_at_unix_ms = unix_ms();
            previous
        };
        self.persist_state()?;
        let current_phase = self.state()?.phase;
        if current_phase != previous_phase {
            self.journal.append(
                "runtime",
                if current_phase == RuntimePhase::Ready {
                    "runtime.recovered"
                } else {
                    "runtime.degraded"
                },
                if current_phase == RuntimePhase::Ready {
                    "info"
                } else {
                    "warn"
                },
                json!({"appHealthy": healthy, "scriptingConnected": scripting_connected}),
            )?;
        }
        Ok(())
    }

    fn refresh_fl_health(&self) -> Result<()> {
        let fl = self.state()?.fl.clone();
        let Some(pid) = fl.pid else {
            return Ok(());
        };
        if !process_is_running(pid)? {
            return self.handle_fl_exit(pid);
        }
        if fl.gopher_ready
            && probe_gopher(
                self.cli.debug_port,
                &self.cli.target_match,
                Duration::from_millis(750),
            )
            .is_err()
        {
            let changed = {
                let mut state = self.state()?;
                if !state.fl.gopher_ready {
                    false
                } else {
                    state.fl.gopher_ready = false;
                    state.fl.gopher_target = None;
                    state.fl.gopher_tool_count = None;
                    if matches!(state.phase, RuntimePhase::Ready | RuntimePhase::Degraded) {
                        state.phase = RuntimePhase::Degraded;
                        state.last_error = Some("FL Studio Gopher target is unavailable".into());
                    }
                    state.updated_at_unix_ms = unix_ms();
                    true
                }
            };
            if changed {
                self.persist_state()?;
                self.journal
                    .append("fl.gopher", "gopher.lost", "warn", json!({"pid": pid}))?;
            }
        }
        Ok(())
    }

    fn handle_fl_exit(&self, pid: u32) -> Result<()> {
        self.stop_app(false)?;
        self.stop_webview(&self.app_ui_child, "app")?;
        let error = format!("FL Studio process {pid} exited");
        {
            let mut state = self.state()?;
            state.fl.pid = None;
            state.fl.launched_by_ghost = false;
            state.fl.window_ready = false;
            state.fl.gopher_ready = false;
            state.fl.gopher_target = None;
            state.fl.gopher_tool_count = None;
            state.app.last_error = Some("FL Studio is unavailable".into());
            if !matches!(state.phase, RuntimePhase::Stopping | RuntimePhase::Stopped) {
                state.phase = RuntimePhase::Degraded;
            }
            state.last_error = Some(error.clone());
            state.updated_at_unix_ms = unix_ms();
        }
        self.persist_state()?;
        self.journal.append(
            "fl.process",
            "fl.exited",
            "error",
            json!({"pid": pid, "error": error}),
        )
    }

    fn mark_degraded(&self, error: String) -> Result<()> {
        {
            let mut state = self.state()?;
            if !matches!(state.phase, RuntimePhase::Stopping | RuntimePhase::Stopped) {
                state.phase = RuntimePhase::Degraded;
            }
            state.app.healthy = false;
            state.app.last_error = Some(error.clone());
            state.last_error = Some(error.clone());
            state.updated_at_unix_ms = unix_ms();
        }
        self.persist_state()?;
        self.journal.append(
            "runtime",
            "runtime.degraded",
            "error",
            json!({"error": error}),
        )
    }

    fn fail(&self, error: &anyhow::Error) {
        if let Ok(mut state) = self.state() {
            state.phase = RuntimePhase::Failed;
            state.last_error = Some(format!("{error:#}"));
            state.updated_at_unix_ms = unix_ms();
        }
        let _ignored = self.persist_state();
        let _ignored = self.journal.append(
            "runtime",
            "runtime.failed",
            "error",
            json!({"error": format!("{error:#}")}),
        );
    }

    fn transition(&self, phase: RuntimePhase, event: &str, data: Value) -> Result<()> {
        {
            let mut state = self.state()?;
            state.phase = phase;
            state.last_error = None;
            state.updated_at_unix_ms = unix_ms();
        }
        self.persist_state()?;
        self.journal
            .append("runtime", event, "info", data)
            .with_context(|| format!("failed to record transition to {}", phase.as_str()))
    }

    fn persist_state(&self) -> Result<()> {
        let state = self.state()?.clone();
        let bytes = serde_json::to_vec_pretty(&state)?;
        fs::write(&self.session_path, bytes)
            .with_context(|| format!("failed to write {}", self.session_path.display()))
    }

    fn state(&self) -> Result<MutexGuard<'_, RuntimeState>> {
        lock(&self.state, "runtime state")
    }

    fn start_monitor(&self) {
        let runtime = self.clone();
        thread::Builder::new()
            .name("ghost-fl-runtime-monitor".into())
            .spawn(move || {
                while !runtime.shutdown.load(Ordering::Relaxed) {
                    for result in [runtime.refresh_fl_health(), runtime.refresh_health()] {
                        if let Err(error) = result {
                            eprintln!("[ghost-fl-runtime] monitor warning: {error:#}");
                            let _ignored = runtime.journal.append(
                                "runtime",
                                "monitor.warning",
                                "warn",
                                json!({"error": format!("{error:#}")}),
                            );
                        }
                    }
                    thread::sleep(Duration::from_secs(1));
                }
            })
            .expect("failed to spawn runtime monitor");
    }

    fn open_gopher_now(&self) -> Result<()> {
        let pid = self
            .state()?
            .fl
            .pid
            .ok_or_else(|| anyhow!("no FL Studio PID is attached"))?;
        if !try_open_gopher(pid)? {
            bail!("could not foreground FL Studio to send Alt+F1");
        }
        let manifest = probe_gopher(
            self.cli.debug_port,
            &self.cli.target_match,
            Duration::from_secs(self.cli.gopher_timeout_seconds),
        )?;
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
        )
    }

    fn start_server(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.cli.bind).with_context(|| {
            format!("failed to bind runtime control panel at {}", self.cli.bind)
        })?;
        listener.set_nonblocking(true)?;
        println!("[ghost-fl-runtime] control panel: http://{}", self.cli.bind);
        let runtime = self.clone();
        thread::Builder::new()
            .name("ghost-fl-runtime-http".into())
            .spawn(move || {
                if let Err(error) = runtime.serve_listener(listener) {
                    eprintln!("[ghost-fl-runtime] HTTP server failed: {error:#}");
                    runtime.fail(&error);
                    runtime.shutdown.store(true, Ordering::Relaxed);
                }
            })
            .context("failed to spawn runtime HTTP server")?;
        Ok(())
    }

    fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        while !self.shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let request = (|| -> Result<()> {
                        stream.set_nonblocking(false)?;
                        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                        self.handle_http(&mut stream)
                    })();
                    if let Err(error) = request {
                        eprintln!("[ghost-fl-runtime] HTTP request failed: {error:#}");
                        let response = json!({"error": error.to_string()});
                        let _ignored =
                            send_json(&mut stream, "500 Internal Server Error", &response);
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn start_bootstrap(&self) -> Result<()> {
        if self.bootstrap_active.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let runtime = self.clone();
        let spawn = thread::Builder::new()
            .name("ghost-fl-runtime-bootstrap".into())
            .spawn(move || {
                if let Err(error) = runtime.bootstrap() {
                    eprintln!("[ghost-fl-runtime] FL bootstrap failed: {error:#}");
                    runtime.fail(&error);
                }
                runtime.bootstrap_active.store(false, Ordering::Release);
            });
        if let Err(error) = spawn {
            self.bootstrap_active.store(false, Ordering::Release);
            return Err(error).context("failed to spawn runtime bootstrap thread");
        }
        Ok(())
    }

    fn run(&self) -> Result<()> {
        self.start_server()?;
        self.open_runtime_webview();
        self.start_monitor();
        self.start_bootstrap()?;
        while !self.shutdown.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
        }
        self.teardown()
    }

    fn open_runtime_webview(&self) {
        let url = format!("http://{}", self.cli.bind);
        if let Err(error) = self.spawn_webview(
            &self.runtime_ui_child,
            &url,
            "Ghost & Guild · FL Runtime",
            1180,
            820,
            "runtime",
        ) {
            eprintln!("[ghost-fl-runtime] runtime webview failed: {error:#}");
            let _ignored = self.journal.append(
                "ui",
                "ui.webview_failed",
                "warn",
                json!({"kind": "runtime", "error": format!("{error:#}")}),
            );
        }
    }

    fn open_app_webview(&self) {
        if self.cli.no_app {
            return;
        }
        let endpoint = match self.state() {
            Ok(state) => state.app.endpoint.clone(),
            Err(error) => {
                eprintln!("[ghost-fl-runtime] app webview state read failed: {error:#}");
                return;
            }
        };
        let url = format!("http://{endpoint}");
        let title = format!("Ghost & Guild · {}", self.cli.app.spec().display_name);
        if let Err(error) = self.spawn_webview(&self.app_ui_child, &url, &title, 1280, 900, "app") {
            eprintln!("[ghost-fl-runtime] app webview failed: {error:#}");
            let _ignored = self.journal.append(
                "ui",
                "ui.webview_failed",
                "warn",
                json!({"kind": "app", "error": format!("{error:#}")}),
            );
        }
    }

    fn spawn_webview(
        &self,
        slot: &Arc<Mutex<Option<Child>>>,
        url: &str,
        title: &str,
        width: u32,
        height: u32,
        kind: &str,
    ) -> Result<()> {
        if self.cli.no_webviews {
            return Ok(());
        }
        #[cfg(not(windows))]
        {
            let _ = (slot, url, title, width, height, kind);
            Ok(())
        }
        #[cfg(windows)]
        {
            self.stop_webview(slot, kind)?;
            let helper = env::current_exe()?.with_file_name(format!(
                "ghost-fl-runtime-webview{}",
                env::consts::EXE_SUFFIX
            ));
            if !helper.is_file() {
                bail!(
                    "runtime webview helper is unavailable at {}",
                    helper.display()
                );
            }
            let child = Command::new(&helper)
                .arg(url)
                .arg(title)
                .arg(width.to_string())
                .arg(height.to_string())
                .spawn()
                .with_context(|| {
                    format!("failed to spawn {} webview at {}", kind, helper.display())
                })?;
            let pid = child.id();
            *lock(slot, "webview child")? = Some(child);
            self.journal.append(
                "ui",
                "ui.webview_started",
                "info",
                json!({"kind": kind, "pid": pid, "url": url}),
            )?;
            Ok(())
        }
    }

    fn stop_webview(&self, slot: &Arc<Mutex<Option<Child>>>, kind: &str) -> Result<()> {
        let mut slot = lock(slot, "webview child")?;
        if let Some(mut child) = slot.take() {
            let pid = child.id();
            if child.try_wait()?.is_none() {
                let _ignored = child.kill();
                let _ignored = child.wait();
            }
            self.journal.append(
                "ui",
                "ui.webview_stopped",
                "info",
                json!({"kind": kind, "pid": pid}),
            )?;
        }
        Ok(())
    }

    fn handle_http(&self, stream: &mut TcpStream) -> Result<()> {
        let Some(request) = read_request(stream)? else {
            return Ok(());
        };
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => send_response(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                INDEX_HTML.as_bytes(),
            ),
            ("GET", "/api/state") => {
                let state = self.state()?.clone();
                send_json(stream, "200 OK", &state)
            }
            ("GET", "/api/events") => {
                let events = self.journal.recent()?;
                send_json(stream, "200 OK", &events)
            }
            ("POST", "/api/app/start") => {
                self.start_app()?;
                self.wait_for_app_ready()?;
                self.open_app_webview();
                self.transition(RuntimePhase::Ready, "app.start_complete", json!({}))?;
                let state = self.state()?.clone();
                send_json(stream, "200 OK", &state)
            }
            ("POST", "/api/app/restart") => {
                self.restart_app()?;
                let state = self.state()?.clone();
                send_json(stream, "200 OK", &state)
            }
            ("POST", "/api/app/stop") => {
                self.stop_app(true)?;
                self.mark_degraded("registered Ghost app is stopped".into())?;
                let state = self.state()?.clone();
                send_json(stream, "200 OK", &state)
            }
            ("POST", "/api/gopher/open") => {
                self.open_gopher_now()?;
                let state = self.state()?.clone();
                send_json(stream, "200 OK", &state)
            }
            ("POST", "/api/fl/retry") => {
                self.start_bootstrap()?;
                let state = self.state()?.clone();
                send_json(stream, "202 Accepted", &state)
            }
            ("POST", "/api/shutdown") => {
                self.shutdown.store(true, Ordering::Relaxed);
                send_json(stream, "200 OK", &json!({"stopping": true}))
            }
            _ => send_json(stream, "404 Not Found", &json!({"error": "not found"})),
        }
    }

    fn teardown(&self) -> Result<()> {
        self.transition(RuntimePhase::Stopping, "runtime.stopping", json!({}))?;
        self.stop_webview(&self.app_ui_child, "app")?;
        self.stop_app(true)?;
        let fl_state = self.state()?.fl.clone();
        if self.cli.shutdown_fl_on_exit && fl_state.launched_by_ghost {
            if let Some(pid) = fl_state.pid {
                terminate_process(pid)?;
                self.journal
                    .append("fl.process", "fl.terminated", "info", json!({"pid": pid}))?;
            }
        }
        self.stop_webview(&self.runtime_ui_child, "runtime")?;
        self.transition(RuntimePhase::Stopped, "runtime.stopped", json!({}))
    }
}

struct HttpRequest {
    method: String,
    path: String,
}

fn probe_gopher(
    debug_port: u16,
    target_match: &str,
    timeout: Duration,
) -> Result<ghost_fl_studio::FlStudioManifest> {
    let bridge_timeout = timeout.min(Duration::from_secs(5));
    let adapter = GopherNativeAdapter::connect(FlStudioAdapterConfig {
        debug_port,
        target_match: target_match.to_owned(),
        connect_timeout: timeout,
        bridge_timeout,
    })?;
    Ok(adapter.manifest()?)
}

fn ensure_fl(cli: &Cli, journal: &EventJournal) -> Result<(u32, bool)> {
    #[cfg(not(windows))]
    {
        let _ = cli;
        let _ = journal;
        bail!("ghost-fl-runtime live supervision currently requires Windows");
    }
    #[cfg(windows)]
    {
        if let Some(pid) = discover_fl_pid()? {
            journal.append("fl.process", "fl.attached", "info", json!({"pid": pid}))?;
            return Ok((pid, false));
        }
        if cli.no_launch {
            bail!("no active FL64.exe process was found and --no-launch was supplied");
        }
        journal.append(
            "fl.process",
            "fl.launching",
            "info",
            json!({"executable": cli.fl_executable.display().to_string()}),
        )?;
        let existing_args = env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").ok();
        let debug_args = append_debug_arg(existing_args.as_deref(), cli.debug_port);
        let child = Command::new(&cli.fl_executable)
            .env("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", debug_args)
            .spawn()
            .with_context(|| format!("failed to launch {}", cli.fl_executable.display()))?;
        let pid = child.id();
        drop(child);
        journal.append("fl.process", "fl.launched", "info", json!({"pid": pid}))?;
        Ok((pid, true))
    }
}

#[cfg(any(windows, test))]
fn append_debug_arg(existing: Option<&str>, port: u16) -> String {
    let debug_arg = format!("--remote-debugging-port={port}");
    match existing.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) if value.contains("--remote-debugging-port=") => value.to_owned(),
        Some(value) => format!("{value} {debug_arg}"),
        None => debug_arg,
    }
}

#[cfg(windows)]
fn discover_fl_pid() -> Result<Option<u32>> {
    let output = powershell_output(
        "$p = Get-Process -Name 'FL64' -ErrorAction SilentlyContinue | Sort-Object StartTime | Select-Object -First 1; if ($p) { $p.Id }",
    )?;
    let text = output.trim();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        text.parse()
            .context("PowerShell returned an invalid FL Studio PID")?,
    ))
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> Result<bool> {
    let output = powershell_output(&format!(
        "$p = Get-Process -Id {pid} -ErrorAction SilentlyContinue; if ($p) {{ '1' }}"
    ))?;
    Ok(output.trim() == "1")
}

#[cfg(not(windows))]
fn process_is_running(pid: u32) -> Result<bool> {
    let _ = pid;
    Ok(false)
}

fn wait_for_fl_window(pid: u32, timeout: Duration) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = pid;
        let _ = timeout;
        bail!("FL window readiness is only supported on Windows");
    }
    #[cfg(windows)]
    {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !process_is_running(pid)? {
                bail!("FL Studio process {pid} exited while waiting for its main window");
            }
            let script = format!(
                "$p = Get-Process -Id {pid} -ErrorAction SilentlyContinue; if ($p -and $p.MainWindowHandle -ne 0) {{ $p.MainWindowHandle }}"
            );
            if !powershell_output(&script)?.trim().is_empty() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(250));
        }
        bail!("FL Studio process {pid} did not expose a main window before timeout")
    }
}

fn try_open_gopher(pid: u32) -> Result<bool> {
    #[cfg(not(windows))]
    {
        let _ = pid;
        Ok(false)
    }
    #[cfg(windows)]
    {
        let script = format!(
            "$wshell = New-Object -ComObject WScript.Shell; if ($wshell.AppActivate({pid})) {{ Start-Sleep -Milliseconds 250; $wshell.SendKeys('%{{F1}}'); exit 0 }} else {{ exit 2 }}"
        );
        Ok(powershell_status(&script)?.success())
    }
}

fn terminate_process(pid: u32) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = pid;
        Ok(())
    }
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T"])
            .status()
            .context("failed to run taskkill")?;
        if !status.success() {
            bail!("taskkill failed for FL Studio PID {pid} with {status}");
        }
        Ok(())
    }
}

#[cfg(windows)]
fn powershell_output(script: &str) -> Result<String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .context("failed to execute PowerShell")?;
    if !output.status.success() {
        bail!(
            "PowerShell failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("PowerShell output was not UTF-8")
}

#[cfg(windows)]
fn powershell_status(script: &str) -> Result<std::process::ExitStatus> {
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .status()
        .context("failed to execute PowerShell")
}

fn spawn_output_reader<R>(reader: R, journal: EventJournal, stream: &'static str)
where
    R: Read + Send + 'static,
{
    thread::Builder::new()
        .name(format!("ghost-app-{stream}"))
        .spawn(move || {
            for line in BufReader::new(reader).lines() {
                match line {
                    Ok(line) => {
                        if journal
                            .append(
                                "app.output",
                                "app.output",
                                "info",
                                json!({"stream": stream, "line": line}),
                            )
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
        .expect("failed to spawn app output reader");
}

fn http_get_json(address: &str, path: &str) -> Result<Value> {
    let socket: SocketAddr = address
        .parse()
        .with_context(|| format!("invalid app address {address}"))?;
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .context("app returned malformed HTTP response")?;
    if !headers.lines().next().unwrap_or_default().contains(" 200 ") {
        bail!(
            "app health request failed: {}",
            headers.lines().next().unwrap_or_default()
        );
    }
    serde_json::from_str(body).context("app health response was not JSON")
}

fn read_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            if bytes.is_empty() {
                return Ok(None);
            }
            bail!("HTTP connection closed before headers completed");
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HTTP_BYTES {
            bail!("HTTP request exceeded {MAX_HTTP_BYTES} bytes");
        }
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let headers = std::str::from_utf8(&bytes).context("HTTP request was not UTF-8")?;
    let request_line = headers
        .lines()
        .next()
        .context("missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().context("missing HTTP method")?.to_owned();
    let path = parts.next().context("missing HTTP path")?.to_owned();
    Ok(Some(HttpRequest { method, path }))
}

fn send_json<T: Serialize>(stream: &mut TcpStream, status: &str, value: &T) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    send_response(stream, status, "application/json; charset=utf-8", &body)
}

fn send_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn default_state_root() -> PathBuf {
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data).join("Konko").join("Ghost");
    }
    if let Some(xdg_state) = env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(xdg_state).join("ghost-and-guild");
    }
    env::current_dir()
        .unwrap_or_else(|_| Path::new(".").to_path_buf())
        .join(".ghost-runtime")
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn lock<'a, T>(mutex: &'a Mutex<T>, name: &str) -> Result<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| anyhow!("{name} lock poisoned"))
}

fn main() -> Result<()> {
    let runtime = Runtime::new(Cli::parse())?;
    runtime.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_argument_is_added_once() {
        assert_eq!(append_debug_arg(None, 9222), "--remote-debugging-port=9222");
        assert_eq!(
            append_debug_arg(Some("--disable-features=Example"), 9222),
            "--disable-features=Example --remote-debugging-port=9222"
        );
        assert_eq!(
            append_debug_arg(Some("--remote-debugging-port=9333 --other"), 9222),
            "--remote-debugging-port=9333 --other"
        );
    }

    #[test]
    fn bootstrap_modes_are_machine_stable() {
        assert_eq!(BootstrapMode::Active.as_str(), "active");
        assert_eq!(BootstrapMode::Start.as_str(), "start");
    }

    #[test]
    fn registered_apps_are_closed_and_stable() {
        let workspace = AppProfile::Workspace.spec();
        let agent = AppProfile::Agent.spec();
        assert_eq!(workspace.package, "ghost-fl-workspace");
        assert_eq!(workspace.default_bind, "127.0.0.1:48775");
        assert_eq!(agent.package, "ghost-fl-agent");
        assert_eq!(agent.default_bind, "127.0.0.1:48765");
        assert!(workspace.requires_scripting);
        assert!(agent.requires_scripting);
    }

    #[test]
    fn phase_names_are_machine_stable() {
        assert_eq!(RuntimePhase::WaitingGopher.as_str(), "waiting_gopher");
        assert_eq!(RuntimePhase::Ready.as_str(), "ready");
        assert_eq!(RuntimePhase::Degraded.as_str(), "degraded");
    }
}
