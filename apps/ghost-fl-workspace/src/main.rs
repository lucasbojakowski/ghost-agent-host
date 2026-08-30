mod audio_tools;
mod gateway;
mod history;
mod project;
mod skills;
mod snapshot;
mod threads;
mod workspace_tools;

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use audio_tools::{AnalyzeAudioRequest, ReadAudioRequest};
use clap::Parser;
use gateway::{build_workspace_registry, SCRIPTING_TOOL_NAMES};
use ghost_codex::{
    AgentEvent, CodexParallelRuntime, ParallelCodexThread, ParallelThreadConfig, ToolRegistry,
    TurnInput, TurnOptions,
};
use ghost_fl_scripting::{FlScriptingAdapter, FlScriptingConfig, FlScriptingStatus};
use ghost_fl_studio::{FlStudioAdapterConfig, GopherNativeAdapter};
use history::ThreadHistoryResponse;
use project::{AssetIdRequest, AssetRequest, ProjectContext, ProjectUpdate, WorkspaceProjectHub};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snapshot::{capture_workspace_snapshot, WorkspaceSnapshot};
use threads::{WorkspaceThreadRecord, WorkspaceThreadStore};
use workspace_tools::{register_workspace_tools, WorkspaceToolState, WORKSPACE_TOOL_NAMES};

const INDEX_HTML: &str = include_str!("../web/index.html");
const MAIN_TS: &str = include_str!("../web/src/main.ts");
const STYLE_CSS: &str = include_str!("../web/src/style.css");
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;

const BASE_SYSTEM_PROMPT: &str = r#"You are Ghost, an empirical FL Studio production workspace agent collaborating with a human producer on real projects.

You have four evidence/execution layers:

1. Every raw native Gopher tool advertised by the live FL Studio Gopher catalog. Their schemas and descriptions are authoritative and exposed unchanged.
2. Exactly three FL MIDI Scripting gateways: fl_scripting_search, fl_scripting_describe, and fl_scripting_call. They provide progressive access to the checked-in runtime-evidenced scripting catalog.
3. Ghost workspace tools for deterministic local audio analysis, Ghost Tap capture, persistent production-project context, structured Production Plans, and app-bundled skills.
4. The producer's description, decisions and musical intent.

Keep these epistemic layers distinct:
- deterministic measurements are acoustic evidence;
- tempo/onset/pitch/section outputs are lightweight musical projections with uncertainty;
- producer language and your musical interpretation are semantic/creative evidence;
- the Production Plan is intended project structure, not proof of live DAW state;
- FL Studio is authoritative for what currently exists in the DAW.

Use ghost_audio_analyze for supplied reference mixes and stems, then use ghost_audio_read progressively instead of flooding context with every available analysis view. Use ghost_audio_compare when relationships between stems/reference are the actual question. Isolated stem labels are useful semantic evidence: do not re-classify a stem the producer already identified unless correctness requires questioning that label.

When live rendered audio inside FL must be measured, load the fl-audio-capture skill before using Ghost Tap. Verify the exact mixer insert and Tap slot relative to processing, set and verify transport position, arm the Tap before playback, start playback through FL, then collect the same request id and verify the artifact. A fresh Tap status alone does not establish the intended mixer location.

When a listed workspace skill directly matches the task, call workspace_skill_read and follow it. Skills are operational guidance, not observations of current FL state.

The FL scripting snapshot and compact project context injected before every user request are point-in-time conveniences. The human may edit FL Studio between actions. Re-observe whenever correctness depends on live state.

Use fl_scripting_search and fl_scripting_describe when you need to discover a scripting primitive or evidence-backed signature. Use fl_scripting_call only with an explicit module/function and positional JSON arguments. For Gopher calls, use live schemas exactly. For relative or structural changes, inspect current values and preserve unrelated state unless the producer asks otherwise.

Do not claim a mutation or capture succeeded unless its tool call succeeded and, when practical, the result has been inspected. For client production work, prefer a guided sequence: analyze -> synthesize a Production Plan -> get producer direction -> scaffold FL -> plan sections/timbres -> build short MIDI/sample sections -> validate -> expand."#;

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-workspace",
    about = "Production workspace combining FL Studio, deterministic audio analysis and Codex"
)]
struct Cli {
    #[arg(long, default_value_t = 9222)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[arg(long, default_value = "127.0.0.1:48775")]
    bind: String,

    #[arg(long, default_value = "127.0.0.1:48766")]
    scripting_bind: String,

    #[arg(long, default_value_t = 1500)]
    scripting_timeout_ms: u64,

    #[arg(long, default_value = "codex")]
    codex_binary: String,

    #[arg(long, default_value = "gpt-5.6-sol")]
    model: String,

    #[arg(long)]
    verbose_agent_events: bool,

    #[arg(long = "i-accept-live-fl-writes")]
    i_accept_live_fl_writes: bool,
}

struct AgentSession {
    runtime: CodexParallelRuntime,
    thread: Option<ParallelCodexThread>,
    registry: ToolRegistry,
    workspace_tools: WorkspaceToolState,
    thread_store: WorkspaceThreadStore,
    model: String,
    effort: String,
    cwd: PathBuf,
    scripting: Arc<FlScriptingAdapter>,
    gopher_tool_count: usize,
    total_tool_count: usize,
    bootstrapped_threads: BTreeSet<String>,
    verbose_agent_events: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    message: String,
    model: Option<String>,
    effort: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadIdRequest {
    thread_id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForkThreadRequest {
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameThreadRequest {
    thread_id: Option<String>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct SkillReadRequest {
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatResponse {
    text: String,
    thread_id: String,
    model: String,
    effort: String,
    snapshot: WorkspaceSnapshot,
    trace: Vec<TraceEvent>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InfoResponse {
    model: String,
    reasoning_effort: String,
    thread_id: Option<String>,
    thread_name: Option<String>,
    thread_count: usize,
    gopher_tool_count: usize,
    scripting_tool_count: usize,
    workspace_tool_count: usize,
    total_tool_count: usize,
    profile: &'static str,
    scripting: FlScriptingStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThreadListResponse {
    selected_thread_id: Option<String>,
    threads: Vec<WorkspaceThreadRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceEvent {
    kind: &'static str,
    tool: String,
    arguments: Option<Value>,
    success: Option<bool>,
    duration_ms: Option<u64>,
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // if !cli.i_accept_live_fl_writes {
    //     bail!(
    //         "ghost-fl-workspace can perform live FL Studio writes; pass --i-accept-live-fl-writes only after opening a project you are willing to modify"
    //     );
    // }

    let gopher = Arc::new(
        GopherNativeAdapter::connect(FlStudioAdapterConfig {
            debug_port: cli.debug_port,
            target_match: cli.target_match.clone(),
            ..Default::default()
        })
        .context("failed to connect to the live FL Studio Gopher target")?,
    );
    let manifest = gopher.manifest()?;
    let gopher_tool_count = manifest.tools.len();

    let scripting = Arc::new(FlScriptingAdapter::start(FlScriptingConfig {
        bind: cli.scripting_bind.clone(),
        call_timeout: Duration::from_millis(cli.scripting_timeout_ms),
    })?);

    let project = Arc::new(Mutex::new(WorkspaceProjectHub::open_default()?));
    let workspace_tools = WorkspaceToolState::new(Arc::clone(&project))?;
    let mut registry =
        build_workspace_registry(&manifest, Arc::clone(&gopher), Arc::clone(&scripting))?;
    register_workspace_tools(&mut registry, workspace_tools.clone())?;
    let total_tool_count = registry.definitions().len();

    println!(
        "[ghost-fl-workspace] connected to '{}' with {} Gopher + {} scripting + {} workspace tools",
        manifest.target_title,
        gopher_tool_count,
        SCRIPTING_TOOL_NAMES.len(),
        WORKSPACE_TOOL_NAMES.len()
    );
    println!(
        "[ghost-fl-workspace] FL scripting listener: {}",
        scripting.status().bind
    );

    let runtime = CodexParallelRuntime::spawn(&cli.codex_binary)
        .context("failed to launch persistent Codex App Server")?;
    let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
    let thread_store = WorkspaceThreadStore::open_default()?;
    let mut session = AgentSession {
        runtime,
        thread: None,
        registry,
        workspace_tools,
        thread_store,
        model: cli.model.clone(),
        effort: "high".into(),
        cwd,
        scripting,
        gopher_tool_count,
        total_tool_count,
        bootstrapped_threads: BTreeSet::new(),
        verbose_agent_events: cli.verbose_agent_events,
    };

    if let Some(thread_id) = session.thread_store.selected_id().map(str::to_owned) {
        match session.resume_thread(&thread_id) {
            Ok(thread) => println!(
                "[ghost-fl-workspace] resumed workspace thread {} using {} with {} dynamic tools",
                thread.id, thread.model, total_tool_count
            ),
            Err(error) => eprintln!(
                "[ghost-fl-workspace] selected thread {} could not be resumed yet: {error:#}",
                thread_id
            ),
        }
    } else {
        println!(
            "[ghost-fl-workspace] no workspace thread selected; opening the app will not create an empty Codex thread"
        );
    }

    println!("[ghost-fl-workspace] WARNING: Codex turns run with full host filesystem access");
    println!("[ghost-fl-workspace] WARNING: combined surface can perform live FL Studio writes");
    serve(&cli.bind, &mut session)
}

impl AgentSession {
    fn info(&self) -> InfoResponse {
        let selected = self.thread_store.selected_record();
        InfoResponse {
            model: self
                .thread
                .as_ref()
                .map(|thread| thread.model.clone())
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| self.model.clone()),
            reasoning_effort: self.effort.clone(),
            thread_id: self.thread.as_ref().map(|thread| thread.id.clone()),
            thread_name: selected.and_then(|thread| thread.name.clone()),
            thread_count: self.thread_store.len(),
            gopher_tool_count: self.gopher_tool_count,
            scripting_tool_count: SCRIPTING_TOOL_NAMES.len(),
            workspace_tool_count: WORKSPACE_TOOL_NAMES.len(),
            total_tool_count: self.total_tool_count,
            profile: "workspace",
            scripting: self.scripting.status(),
        }
    }

    fn threads(&self) -> ThreadListResponse {
        ThreadListResponse {
            selected_thread_id: self.thread_store.selected_id().map(str::to_owned),
            threads: self.thread_store.list(),
        }
    }

    fn history(&mut self) -> Result<ThreadHistoryResponse> {
        let Some(thread_id) = self.thread_store.selected_id().map(str::to_owned) else {
            return Ok(ThreadHistoryResponse::empty());
        };
        self.activate_project(&thread_id)?;
        let result = self.runtime.read_full_thread_history(&thread_id)?;
        let history = ThreadHistoryResponse::from_thread_read(&thread_id, &result)?;
        if !history.messages.is_empty() {
            self.bootstrapped_threads.insert(thread_id.clone());
            if !self
                .thread_store
                .record(&thread_id)
                .map(|record| record.has_turns)
                .unwrap_or(false)
            {
                self.thread_store.mark_turn(&thread_id)?;
            }
        }
        Ok(history)
    }

    fn thread_config(&self) -> ParallelThreadConfig {
        ParallelThreadConfig::new(self.model.clone())
            .cwd(self.cwd.clone())
            .service_name("ghost_fl_workspace")
    }

    fn create_thread(&mut self) -> Result<ParallelCodexThread> {
        let thread = self
            .runtime
            .start_thread(self.thread_config(), self.registry.clone())?;
        self.thread_store.register(thread.id.clone(), None, false)?;
        self.activate_project(&thread.id)?;
        self.thread = Some(thread.clone());
        println!(
            "[ghost-fl-workspace] created workspace thread {} using {}",
            thread.id, thread.model
        );
        Ok(thread)
    }

    fn resume_thread(&mut self, thread_id: &str) -> Result<ParallelCodexThread> {
        let has_turns = self
            .thread_store
            .record(thread_id)
            .with_context(|| format!("unknown workspace thread `{thread_id}`"))?
            .has_turns;
        let mut thread = self
            .runtime
            .resume_thread(thread_id, self.registry.clone())?;
        if thread.model.is_empty() {
            thread.model = self.model.clone();
        }
        self.thread_store.select(thread_id)?;
        self.activate_project(thread_id)?;
        if has_turns {
            self.bootstrapped_threads.insert(thread_id.to_owned());
        }
        self.thread = Some(thread.clone());
        Ok(thread)
    }

    fn ensure_active_thread(&mut self) -> Result<ParallelCodexThread> {
        if let Some(thread) = self.thread.clone() {
            self.activate_project(&thread.id)?;
            return Ok(thread);
        }
        if let Some(thread_id) = self.thread_store.selected_id().map(str::to_owned) {
            return self.resume_thread(&thread_id);
        }
        self.create_thread()
    }

    fn select_thread(&mut self, thread_id: &str) -> Result<ThreadListResponse> {
        self.resume_thread(thread_id)?;
        Ok(self.threads())
    }

    fn fork_thread(&mut self, requested_thread_id: Option<&str>) -> Result<ThreadListResponse> {
        let source_id = requested_thread_id
            .map(str::to_owned)
            .or_else(|| self.thread.as_ref().map(|thread| thread.id.clone()))
            .or_else(|| self.thread_store.selected_id().map(str::to_owned))
            .context("there is no workspace thread to fork")?;
        let source_has_turns = self
            .thread_store
            .record(&source_id)
            .with_context(|| format!("unknown workspace thread `{source_id}`"))?
            .has_turns;
        let source_project = {
            let mut hub = self
                .workspace_tools
                .project
                .lock()
                .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?;
            hub.activate(source_id.clone())?
        };

        let mut fork = self
            .runtime
            .fork_thread(&source_id, self.registry.clone())?;
        if fork.model.is_empty() {
            fork.model = self.model.clone();
        }
        self.thread_store
            .register(fork.id.clone(), Some(source_id.clone()), source_has_turns)?;
        self.clone_project_to_fork(&source_project, &fork.id)?;
        if source_has_turns {
            self.bootstrapped_threads.insert(fork.id.clone());
        }
        self.thread = Some(fork.clone());
        println!(
            "[ghost-fl-workspace] forked workspace thread {} from {}",
            fork.id, source_id
        );
        Ok(self.threads())
    }

    fn rename_thread(
        &mut self,
        requested_thread_id: Option<&str>,
        name: &str,
    ) -> Result<ThreadListResponse> {
        let thread_id = requested_thread_id
            .map(str::to_owned)
            .or_else(|| self.thread_store.selected_id().map(str::to_owned))
            .context("there is no workspace thread to rename")?;
        let record = self.thread_store.rename(&thread_id, name)?;
        if record.has_turns {
            if let Err(error) = self
                .runtime
                .set_thread_name(&thread_id, record.name.as_deref().unwrap_or_default())
            {
                eprintln!(
                    "[ghost-fl-workspace] thread name is saved locally but Codex name sync failed for {}: {error}",
                    thread_id
                );
            }
        }
        Ok(self.threads())
    }

    fn run_user_turn(&mut self, request: &ChatRequest) -> Result<ChatResponse> {
        let message = request.message.trim();
        if message.is_empty() {
            bail!("message must not be empty");
        }

        let thread = self.ensure_active_thread()?;
        let model = validate_model(request.model.as_deref().unwrap_or(&self.model))?;
        let effort = validate_effort(request.effort.as_deref().unwrap_or(&self.effort))?;
        self.model = model.clone();
        self.effort = effort.clone();
        if let Some(active) = self.thread.as_mut() {
            active.model = model.clone();
        }
        let mut turn_thread = thread.clone();
        turn_thread.model = model.clone();

        let snapshot = capture_workspace_snapshot(&self.scripting);
        let snapshot_text = serde_json::to_string_pretty(&snapshot)?;
        let project_text = self
            .workspace_tools
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?
            .compact_prompt_context()?;
        let turn_text = format!(
            "POINT-IN-TIME FL MIDI SCRIPTING SNAPSHOT (re-observe if correctness depends on it):\n{snapshot_text}\n\nCURRENT WORKSPACE PROJECT CONTEXT (semantic intent/reference metadata, not live FL state):\n{project_text}\n\nUSER REQUEST:\n{message}"
        );
        let has_persisted_turns = self
            .thread_store
            .record(&thread.id)
            .map(|record| record.has_turns)
            .unwrap_or(false);
        let needs_bootstrap =
            !has_persisted_turns && !self.bootstrapped_threads.contains(&thread.id);
        let input_text = if needs_bootstrap {
            format!("{}\n\n{turn_text}", system_prompt())
        } else {
            turn_text
        };
        let input = TurnInput {
            text: input_text,
            output_schema: None,
        };
        let mut trace = Vec::new();
        let verbose = self.verbose_agent_events;
        let options = full_access_turn_options(&effort);
        let output = self
            .runtime
            .run_turn(&turn_thread, &input, &options, &mut |event| {
                if verbose {
                    println!("[ghost-fl-workspace] agent event: {event:?}");
                }
                if let Some(trace_event) = trace_event(&event) {
                    if !verbose {
                        match trace_event.kind {
                            "tool_started" => println!(
                                "[ghost-fl-workspace] tool -> {} {}",
                                trace_event.tool,
                                trace_event
                                    .arguments
                                    .as_ref()
                                    .map(Value::to_string)
                                    .unwrap_or_else(|| "{}".into())
                            ),
                            "tool_completed" => println!(
                                "[ghost-fl-workspace] tool <- {} success={} duration_ms={}",
                                trace_event.tool,
                                trace_event.success.unwrap_or(false),
                                trace_event.duration_ms.unwrap_or(0)
                            ),
                            _ => {}
                        }
                    }
                    trace.push(trace_event);
                }
            })?;

        if needs_bootstrap {
            self.bootstrapped_threads.insert(thread.id.clone());
        }
        let record = self.thread_store.mark_turn(&thread.id)?;
        if let Some(name) = record.name.as_deref() {
            if let Err(error) = self.runtime.set_thread_name(&thread.id, name) {
                eprintln!(
                    "[ghost-fl-workspace] thread name is saved locally but Codex name sync failed for {}: {error}",
                    thread.id
                );
            }
        }

        Ok(ChatResponse {
            text: output.text,
            thread_id: thread.id,
            model,
            effort,
            snapshot,
            trace,
        })
    }

    fn project_json(&self) -> Result<Value> {
        let hub = self
            .workspace_tools
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?;
        if hub.active_thread_id().is_none() {
            return Ok(json!({"project": Value::Null}));
        }
        Ok(json!({"project": hub.current()?}))
    }

    fn update_project(&self, update: ProjectUpdate) -> Result<ProjectContext> {
        self.workspace_tools
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?
            .update(update)
    }

    fn add_asset(&self, request: AssetRequest) -> Result<ProjectContext> {
        self.workspace_tools
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?
            .add_asset(request)
    }

    fn remove_asset(&self, asset_id: &str) -> Result<ProjectContext> {
        self.workspace_tools
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?
            .remove_asset(asset_id)
    }

    fn activate_project(&self, thread_id: &str) -> Result<()> {
        self.workspace_tools
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?
            .activate(thread_id.to_owned())?;
        Ok(())
    }

    fn clone_project_to_fork(&self, source: &ProjectContext, fork_id: &str) -> Result<()> {
        let mut hub = self
            .workspace_tools
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?;
        hub.activate(fork_id.to_owned())?;
        hub.update(ProjectUpdate {
            title: Some(source.title.clone()),
            description: Some(source.description.clone()),
            tempo_bpm: Some(source.tempo_bpm),
            time_signature: Some(source.time_signature.clone()),
        })?;
        for asset in &source.assets {
            let project = hub.add_asset(AssetRequest {
                path: asset.path.clone(),
                label: Some(asset.label.clone()),
                role: Some(asset.role.clone()),
            })?;
            if let Some(analysis_id) = &asset.analysis_id {
                if let Some(cloned) = project.assets.iter().find(|item| item.path == asset.path) {
                    hub.set_asset_analysis(&cloned.id, analysis_id)?;
                }
            }
        }
        hub.set_plan(source.production_plan.clone())?;
        Ok(())
    }
}

fn system_prompt() -> String {
    format!("{BASE_SYSTEM_PROMPT}\n\n{}", skills::bootstrap_index())
}

fn validate_model(model: &str) -> Result<String> {
    let model = model.trim();
    if model.is_empty() || model.len() > 128 || model.contains(['\r', '\n']) {
        bail!("model must be a non-empty single-line identifier up to 128 characters");
    }
    Ok(model.to_owned())
}

fn validate_effort(effort: &str) -> Result<String> {
    let effort = effort.trim().to_ascii_lowercase();
    if !matches!(effort.as_str(), "low" | "medium" | "high" | "xhigh") {
        bail!("effort must be one of low, medium, high, or xhigh");
    }
    Ok(effort)
}

fn full_access_turn_options(effort: &str) -> TurnOptions {
    TurnOptions {
        effort: effort.to_owned(),
        sandbox_policy: json!({"type": "dangerFullAccess"}),
        ..TurnOptions::default()
    }
}

fn trace_event(event: &AgentEvent) -> Option<TraceEvent> {
    match event {
        AgentEvent::ItemStarted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            Some(TraceEvent {
                kind: "tool_started",
                tool: item
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
                    .to_owned(),
                arguments: item.get("arguments").cloned(),
                success: None,
                duration_ms: None,
            })
        }
        AgentEvent::ItemCompleted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            Some(TraceEvent {
                kind: "tool_completed",
                tool: item
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
                    .to_owned(),
                arguments: item.get("arguments").cloned(),
                success: item.get("success").and_then(Value::as_bool),
                duration_ms: item.get("durationMs").and_then(Value::as_u64),
            })
        }
        _ => None,
    }
}

fn serve(bind: &str, session: &mut AgentSession) -> Result<()> {
    let listener = TcpListener::bind(bind)
        .with_context(|| format!("failed to bind ghost-fl-workspace web UI at {bind}"))?;
    let address = listener.local_addr()?;
    println!("[ghost-fl-workspace] UI: http://{address}");

    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("[ghost-fl-workspace] failed to accept HTTP connection: {error}");
                continue;
            }
        };
        if let Err(error) = handle_connection(&mut stream, session) {
            eprintln!("[ghost-fl-workspace] HTTP request failed: {error:#}");
            let _ = send_json(
                &mut stream,
                "500 Internal Server Error",
                &ErrorResponse {
                    error: error.to_string(),
                },
            );
        }
    }
    Ok(())
}

fn handle_connection(stream: &mut TcpStream, session: &mut AgentSession) -> Result<()> {
    let Some(request) = read_request(stream)? else {
        return Ok(());
    };
    let path = request.path.split('?').next().unwrap_or(&request.path);

    match (request.method.as_str(), path) {
        ("GET", "/") => send_response(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes(),
        ),
        ("GET", "/src/main.ts") => send_response(
            stream,
            "200 OK",
            "text/javascript; charset=utf-8",
            MAIN_TS.as_bytes(),
        ),
        ("GET", "/src/style.css") => send_response(
            stream,
            "200 OK",
            "text/css; charset=utf-8",
            STYLE_CSS.as_bytes(),
        ),
        ("GET", "/src/tailwindcss") => {
            send_response(stream, "200 OK", "text/css; charset=utf-8", b"")
        }
        ("GET", "/api/info") => send_json(stream, "200 OK", &session.info()),
        ("GET", "/api/threads") => send_json(stream, "200 OK", &session.threads()),
        ("GET", "/api/history") => {
            let history = session.history()?;
            send_json(stream, "200 OK", &history)
        }
        ("GET", "/api/project") => send_json(stream, "200 OK", &session.project_json()?),
        ("PUT", "/api/project") => {
            let request: ProjectUpdate =
                serde_json::from_slice(&request.body).context("invalid /api/project JSON body")?;
            send_json(stream, "200 OK", &session.update_project(request)?)
        }
        ("POST", "/api/project/assets") => {
            let request: AssetRequest = serde_json::from_slice(&request.body)
                .context("invalid /api/project/assets JSON body")?;
            send_json(stream, "201 Created", &session.add_asset(request)?)
        }
        ("POST", "/api/project/assets/remove") => {
            let request: AssetIdRequest = serde_json::from_slice(&request.body)
                .context("invalid /api/project/assets/remove JSON body")?;
            send_json(stream, "200 OK", &session.remove_asset(&request.asset_id)?)
        }
        ("POST", "/api/analysis/run") => {
            let request: AnalyzeAudioRequest = serde_json::from_slice(&request.body)
                .context("invalid /api/analysis/run JSON body")?;
            let response = session.workspace_tools.audio.analyze(request)?;
            send_json(stream, "200 OK", &response)
        }
        ("POST", "/api/analysis/read") => {
            let request: ReadAudioRequest = serde_json::from_slice(&request.body)
                .context("invalid /api/analysis/read JSON body")?;
            let response = session.workspace_tools.audio.read(request)?;
            send_json(stream, "200 OK", &response)
        }
        ("GET", "/api/skills") => send_json(stream, "200 OK", &skills::list_skills()),
        ("POST", "/api/skills/read") => {
            let request: SkillReadRequest = serde_json::from_slice(&request.body)
                .context("invalid /api/skills/read JSON body")?;
            send_json(stream, "200 OK", &skills::read_skill(&request.name)?)
        }
        ("GET", "/api/scripting/status") => {
            send_json(stream, "200 OK", &session.scripting.status())
        }
        ("GET", "/api/snapshot") => {
            let snapshot = capture_workspace_snapshot(&session.scripting);
            send_json(stream, "200 OK", &snapshot)
        }
        ("POST", "/api/threads/new") => {
            session.create_thread()?;
            send_json(stream, "201 Created", &session.threads())
        }
        ("POST", "/api/threads/select") => {
            let request: ThreadIdRequest = serde_json::from_slice(&request.body)
                .context("invalid /api/threads/select JSON body")?;
            let response = session.select_thread(&request.thread_id)?;
            send_json(stream, "200 OK", &response)
        }
        ("POST", "/api/threads/fork") => {
            let request = if request.body.is_empty() {
                ForkThreadRequest::default()
            } else {
                serde_json::from_slice(&request.body)
                    .context("invalid /api/threads/fork JSON body")?
            };
            let response = session.fork_thread(request.thread_id.as_deref())?;
            send_json(stream, "201 Created", &response)
        }
        ("POST", "/api/threads/rename") => {
            let request: RenameThreadRequest = serde_json::from_slice(&request.body)
                .context("invalid /api/threads/rename JSON body")?;
            let response = session.rename_thread(request.thread_id.as_deref(), &request.name)?;
            send_json(stream, "200 OK", &response)
        }
        ("POST", "/api/chat") => {
            let request: ChatRequest =
                serde_json::from_slice(&request.body).context("invalid /api/chat JSON body")?;
            let response = session.run_user_turn(&request)?;
            send_json(stream, "200 OK", &response)
        }
        _ => send_json(
            stream,
            "404 Not Found",
            &ErrorResponse {
                error: "not found".into(),
            },
        ),
    }
}

fn read_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            if bytes.is_empty() {
                return Ok(None);
            }
            bail!("HTTP connection closed before request headers completed");
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            bail!("HTTP request exceeded {MAX_REQUEST_BYTES} bytes");
        }
        if let Some(position) = find_bytes(&bytes, b"\r\n\r\n") {
            break position + 4;
        }
    };

    let headers =
        std::str::from_utf8(&bytes[..header_end]).context("HTTP headers were not UTF-8")?;
    let mut lines = headers.split("\r\n");
    let request_line = lines.next().context("missing HTTP request line")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .context("missing HTTP method")?
        .to_owned();
    let path = request_parts
        .next()
        .context("missing HTTP path")?
        .to_owned();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>())
        .transpose()
        .context("invalid Content-Length header")?
        .unwrap_or(0);

    let expected = header_end
        .checked_add(content_length)
        .context("HTTP request length overflow")?;
    if expected > MAX_REQUEST_BYTES {
        bail!("HTTP request exceeded {MAX_REQUEST_BYTES} bytes");
    }
    while bytes.len() < expected {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            bail!("HTTP connection closed before request body completed");
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            bail!("HTTP request exceeded {MAX_REQUEST_BYTES} bytes");
        }
    }

    Ok(Some(HttpRequest {
        method,
        path,
        body: bytes[header_end..expected].to_vec(),
    }))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_http_header_boundary() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\nbody";
        assert_eq!(find_bytes(request, b"\r\n\r\n"), Some(31));
    }

    #[test]
    fn turns_use_requested_effort_and_full_access_without_changing_defaults() {
        let options = full_access_turn_options("xhigh");
        assert_eq!(options.effort, "xhigh");
        assert_eq!(options.sandbox_policy, json!({"type": "dangerFullAccess"}));
        assert_eq!(options.approval_policy, "never");
        assert_eq!(
            TurnOptions::default().sandbox_policy,
            json!({
                "type": "readOnly",
                "access": {"type": "fullAccess"}
            })
        );
    }

    #[test]
    fn reasoning_effort_is_bounded() {
        assert_eq!(validate_effort("medium").unwrap(), "medium");
        assert!(validate_effort("infinite").is_err());
    }
}
