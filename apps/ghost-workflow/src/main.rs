use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_audio::{analyze_audio, read_audio, AnalysisConfig};
use ghost_codex::{
    AgentEvent, CodexParallelRuntime, ParallelThreadConfig, ToolDefinition, ToolError,
    ToolRegistry, TurnInput, TurnOptions,
};
use ghost_context::{CompiledContext, ContextMessage, MessageRole, OutputContract};
use ghost_fl_studio::{
    FlStudioAdapterConfig, FlStudioManifest, GopherNativeAdapter, NativeToolDefinition,
};
use ghost_tap::{find_live_tap, request_capture, wait_for_capture, TapCaptureCommand};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(
    name = "ghost-workflow",
    about = "Capture live FL Studio audio with Ghost Tap, analyze it, and let one Codex App Server thread operate an app-selected raw Gopher tool surface"
)]
struct Cli {
    #[arg(long, default_value_t = 9222)]
    debug_port: u16,
    #[arg(long, default_value = "gopher")]
    target_match: String,
    #[arg(long, default_value_t = 0)]
    tap_instance: u32,
    #[arg(long, default_value_t = 4.0)]
    capture_seconds: f64,
    #[arg(long, default_value = "1")]
    track: String,
    #[arg(long, default_value_t = 1)]
    slot_start: u32,
    #[arg(long, default_value_t = 4)]
    slot_end: u32,
    #[arg(long = "plugin")]
    plugins: Vec<String>,
    #[arg(
        long,
        default_value = "Build a clearly audible but musical processor chain for this sample from the measured evidence. Improve clarity, balance, dynamics, and usefulness in a mix while preserving the sample's identity."
    )]
    intent: String,
    #[arg(long, default_value_t = 0.70)]
    processing_intensity: f64,
    #[arg(long, default_value = "codex")]
    codex_binary: String,
    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,
    #[arg(long)]
    verbose_agent_events: bool,
    #[arg(
        long = "i-accept-live-fl-writes",
        alias = "i-have-positioned-playhead-and-accepted-scoped-writes"
    )]
    i_accept_live_fl_writes: bool,
}

#[derive(Debug, Clone)]
struct AppFlPolicy {
    track: String,
    slot_start: u32,
    slot_end: u32,
    allowed_plugins: Vec<String>,
}

impl AppFlPolicy {
    fn authorize(&self, tool: &str, arguments: &Value) -> Result<(), ToolError> {
        match tool {
            "add_effect" => {
                let plugin = string_arg(arguments, "plugin")?;
                let target = string_arg(arguments, "target_tracks")?;
                let slot = u32_arg(arguments, "slot_number")?;
                if target != self.track {
                    return Err(ToolError(format!(
                        "workflow permits add_effect only on mixer track {}",
                        self.track
                    )));
                }
                if !(self.slot_start..=self.slot_end).contains(&slot) {
                    return Err(ToolError(format!(
                        "workflow permits effect slots {}..={} only",
                        self.slot_start, self.slot_end
                    )));
                }
                if !self.allowed_plugins.iter().any(|allowed| allowed == plugin) {
                    return Err(ToolError(format!(
                        "workflow does not permit plugin `{plugin}`; allowed: {}",
                        self.allowed_plugins.join(", ")
                    )));
                }
                Ok(())
            }
            "set_plugin_parameter_value" => {
                let target = string_arg(arguments, "target")?;
                let slot = u32_arg(arguments, "slot_number")?;
                let value = arguments
                    .get("value")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| ToolError("`value` must be a number".into()))?;
                if target != self.track {
                    return Err(ToolError(format!(
                        "workflow permits plugin writes only on mixer track {}",
                        self.track
                    )));
                }
                if !(self.slot_start..=self.slot_end).contains(&slot) {
                    return Err(ToolError(format!(
                        "workflow permits effect slots {}..={} only",
                        self.slot_start, self.slot_end
                    )));
                }
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(ToolError(
                        "workflow requires normalized plugin values in 0..=1".into(),
                    ));
                }
                Ok(())
            }
            _ => Err(ToolError(format!(
                "workflow does not permit mutating FL tool `{tool}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppMutation {
    tool: String,
    arguments: Value,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.i_accept_live_fl_writes {
        bail!(
            "refusing live FL writes: pass --i-accept-live-fl-writes after positioning the playhead, stopping transport, and confirming target mixer track {} / slots {}..={} are appropriate for this test",
            cli.track,
            cli.slot_start,
            cli.slot_end
        );
    }
    if cli.slot_start == 0 || cli.slot_end < cli.slot_start || cli.slot_end > 10 {
        bail!("processor slots must be a non-empty range inside FL mixer slots 1..=10");
    }
    if !cli.processing_intensity.is_finite() || !(0.0..=1.0).contains(&cli.processing_intensity) {
        bail!("--processing-intensity must be a finite value in 0..=1");
    }

    let allowed_plugins = if cli.plugins.is_empty() {
        vec!["Pro-Q 4".to_owned(), "Pro-C 3".to_owned()]
    } else {
        cli.plugins.clone()
    };
    let policy = AppFlPolicy {
        track: cli.track.clone(),
        slot_start: cli.slot_start,
        slot_end: cli.slot_end,
        allowed_plugins: allowed_plugins.clone(),
    };

    let adapter = Arc::new(
        GopherNativeAdapter::connect(FlStudioAdapterConfig {
            debug_port: cli.debug_port,
            target_match: cli.target_match.clone(),
            ..Default::default()
        })
        .context("failed to connect Ghost to the FL Studio Gopher adapter")?,
    );
    let manifest = adapter.manifest()?;
    println!(
        "[ghost-workflow] FL/Gopher target connected with {} live native tools.",
        manifest.tools.len()
    );

    let tap = find_live_tap(cli.tap_instance).with_context(|| {
        format!(
            "Ghost Tap instance {} is not publishing a fresh status. Load Ghost Tap on the target mixer track and let FL activate it.",
            cli.tap_instance
        )
    })?;
    println!(
        "[ghost-workflow] Found Ghost Tap instance {} in FL process {} at sample rate {:?}.",
        tap.instance_id, tap.process_id, tap.sample_rate
    );

    let command = TapCaptureCommand::new(cli.capture_seconds)?;
    request_capture(&tap, &command)?;
    println!(
        "[ghost-workflow] Armed request {} for {:.2}s. Starting FL playback; capture waits for real signal.",
        command.request_id, command.duration_seconds
    );
    adapter
        .call_native("play", json!({}))
        .context("failed to start FL playback for Ghost Tap capture")?;
    let capture = wait_for_capture(
        &tap,
        command.request_id,
        Duration::from_secs_f64(cli.capture_seconds + 12.0),
    );
    let stop_result = adapter.call_native("stop", json!({}));
    let artifact = capture.context("Ghost Tap did not complete the requested capture")?;
    stop_result.context("capture completed, but FL transport could not be stopped")?;

    println!(
        "[ghost-workflow] Captured {} frames at {} Hz ({:.3}s): {}",
        artifact.frames,
        artifact.sample_rate,
        artifact.duration_seconds,
        artifact.wav_path.display()
    );
    let audio = read_audio(&artifact.wav_path)
        .with_context(|| format!("failed to decode {}", artifact.wav_path.display()))?;
    let analysis = analyze_audio(
        artifact.wav_path.display().to_string(),
        &audio,
        &AnalysisConfig::high(),
    )
    .context("Ghost high-resolution analysis failed")?;
    let analysis_path = artifact.wav_path.with_extension("analysis.json");
    fs::write(&analysis_path, serde_json::to_vec_pretty(&analysis)?)?;
    println!(
        "[ghost-workflow] Analysis complete and stored at {}.",
        analysis_path.display()
    );

    let mutations = Arc::new(Mutex::new(Vec::<AppMutation>::new()));
    let registry = build_agent_registry(
        &manifest,
        Arc::clone(&adapter),
        &policy,
        Arc::clone(&mutations),
    )?;
    println!(
        "[ghost-workflow] App selected {} dynamic FL tools from the live catalog.",
        registry.definitions().len()
    );

    let runtime = CodexParallelRuntime::spawn(&cli.codex_binary)
        .context("failed to launch the persistent Codex App Server runtime")?;
    let thread = runtime.start_thread(
        ParallelThreadConfig::new(cli.model.clone()).service_name("ghost_workflow"),
        registry,
    )?;
    println!(
        "[ghost-workflow] Started workflow thread {} on one persistent Codex App Server.",
        thread.id
    );

    let agent_evidence = compact_agent_evidence(&analysis)?;
    let analysis_json = serde_json::to_string(&agent_evidence)?;
    let allowed = allowed_plugins.join(", ");
    let context = CompiledContext {
        schema_version: CompiledContext::SCHEMA.into(),
        messages: vec![
            ContextMessage {
                role: MessageRole::System,
                content: format!(
                    "You are Ghost's audio-processing agent in a live FL Studio experiment. Treat FL Studio itself as current truth: the human may edit the DAW at any time, so inspect current state again before relying on an earlier observation. The measured audio evidence is a snapshot of the captured signal; interpret features jointly.\n\nThis executable selected a live Gopher tool surface from the adapter's raw catalog. Read tools remain raw. The only mutation tools exposed for this workflow are add_effect and set_plugin_parameter_value, wrapped here by app policy: target mixer track {track}, slots {slot_start}..={slot_end}, insertable plugins {allowed}. Do not infer that a permitted slot is empty; inspect before inserting.\n\nGopher argument names are JSON properties but the adapter emits them in the live schema/signature order. Use the tool schemas as authoritative. Plugin parameter writes are normalized 0..1. Read current normalized parameter state before changing it and read it again after changing it. Human display text may lag normalized state or be unavailable for third-party plugins, so do not treat missing/stale display text as proof that a normalized write failed.\n\nPROCESSING_INTENSITY={intensity:.2}. Prefer the smallest purposeful chain that addresses measured spectral/dynamic problems. One or two processors and a few meaningful parameter changes are preferable to ceremonial adjustments. This is an action workflow: make at least one justified processor mutation, then summarize only calls and observations you actually made.\n\nANALYSIS_EVIDENCE_JSON:\n{analysis_json}",
                    track = cli.track,
                    slot_start = cli.slot_start,
                    slot_end = cli.slot_end,
                    intensity = cli.processing_intensity,
                ),
            },
            ContextMessage {
                role: MessageRole::User,
                content: format!(
                    "Intent: {}\nApply a musically meaningful result at intensity {:.2}, then summarize what you changed and why it follows from the evidence and current FL observations.",
                    cli.intent, cli.processing_intensity
                ),
            },
        ],
        output: OutputContract::Text,
        metadata: json!({
            "workflow": "ghost.workflow.tap-process/3",
            "captureRequestId": artifact.request_id,
            "capturePath": artifact.wav_path,
            "analysisPath": analysis_path,
            "targetTrack": cli.track,
            "slotRange": [cli.slot_start, cli.slot_end],
            "allowedPlugins": allowed_plugins,
            "processingIntensity": cli.processing_intensity
        }),
    };

    let turn_input = TurnInput {
        text: context.text(),
        output_schema: match &context.output {
            OutputContract::Text => None,
            OutputContract::Json { schema, .. } => Some(schema.clone()),
        },
    };
    let output = runtime.run_turn(
        &thread,
        &turn_input,
        &TurnOptions::default(),
        &mut |event| print_agent_event(&event, cli.verbose_agent_events),
    )?;
    let workflow_mutations = mutations
        .lock()
        .map_err(|_| anyhow::anyhow!("app mutation journal lock poisoned"))?
        .clone();
    if workflow_mutations.is_empty() {
        bail!("agent turn completed without an app-authorized FL processor mutation");
    }

    println!("[ghost-workflow] Agent: {}", output.text);
    println!(
        "[ghost-workflow] Successful app-authorized mutation calls:\n{}",
        serde_json::to_string_pretty(&workflow_mutations)?
    );
    println!(
        "[ghost-workflow] GREEN: Ghost Tap capture -> Rust analysis -> Codex App Server thread -> app-selected raw FL/Gopher calls."
    );
    Ok(())
}

fn build_agent_registry(
    manifest: &FlStudioManifest,
    adapter: Arc<GopherNativeAdapter>,
    policy: &AppFlPolicy,
    mutations: Arc<Mutex<Vec<AppMutation>>>,
) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::default();
    for tool in &manifest.tools {
        if is_mutating_tool(&tool.name) && !is_workflow_write(&tool.name) {
            continue;
        }
        register_raw_tool(
            &mut registry,
            tool,
            Arc::clone(&adapter),
            policy.clone(),
            Arc::clone(&mutations),
        )?;
    }
    Ok(registry)
}

fn register_raw_tool(
    registry: &mut ToolRegistry,
    tool: &NativeToolDefinition,
    adapter: Arc<GopherNativeAdapter>,
    policy: AppFlPolicy,
    mutations: Arc<Mutex<Vec<AppMutation>>>,
) -> Result<()> {
    let tool_name = tool.name.clone();
    let handler_name = tool_name.clone();
    let definition = ToolDefinition {
        name: tool_name,
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
    };
    registry.register(definition, move |arguments| {
        let is_write = is_workflow_write(&handler_name);
        if is_write {
            policy.authorize(&handler_name, &arguments)?;
        }
        let result = adapter
            .call_native(&handler_name, arguments.clone())
            .map_err(|error| ToolError(error.to_string()))?;
        if is_write {
            mutations
                .lock()
                .map_err(|_| ToolError("app mutation journal lock poisoned".into()))?
                .push(AppMutation {
                    tool: handler_name.clone(),
                    arguments,
                });
        }
        Ok(result.raw)
    })?;
    Ok(())
}

fn is_workflow_write(tool: &str) -> bool {
    matches!(tool, "add_effect" | "set_plugin_parameter_value")
}

fn is_mutating_tool(tool: &str) -> bool {
    matches!(tool, "play" | "stop")
        || tool.starts_with("set_")
        || tool.starts_with("add_")
        || tool.starts_with("remove_")
        || tool.starts_with("delete_")
        || tool.starts_with("create_")
        || tool.starts_with("insert_")
        || tool.starts_with("run_")
}

fn string_arg<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError(format!("`{name}` must be a string")))
}

fn u32_arg(arguments: &Value, name: &str) -> Result<u32, ToolError> {
    let value = arguments
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError(format!("`{name}` must be a non-negative integer")))?;
    u32::try_from(value).map_err(|_| ToolError(format!("`{name}` is out of range")))
}

fn compact_agent_evidence<T: Serialize>(analysis: &T) -> Result<Value> {
    let mut value = serde_json::to_value(analysis)?;
    if let Some(root) = value.as_object_mut() {
        root.remove("configuration");
        if let Some(capture) = root.get_mut("capture").and_then(Value::as_object_mut) {
            capture.remove("capture_id");
            capture.remove("source_name");
            capture.remove("content_hash");
            capture.remove("transport_start_samples");
        }
    }
    if let Some(spectrum) = value
        .pointer_mut("/signal/spectrum")
        .and_then(Value::as_object_mut)
    {
        spectrum.remove("frame_centroid_hz");
        if let Some(resonances) = spectrum.get_mut("resonances").and_then(Value::as_array_mut) {
            resonances.truncate(10);
        }
    }
    Ok(value)
}

fn print_agent_event(event: &AgentEvent, verbose: bool) {
    if verbose {
        println!("[ghost-workflow] agent event: {event:?}");
        return;
    }
    match event {
        AgentEvent::TurnStarted { turn_id } => {
            println!("[ghost-workflow] agent turn started: {turn_id:?}");
        }
        AgentEvent::ItemStarted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            println!(
                "[ghost-workflow] tool -> {} {}",
                item.get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>"),
                item.get("arguments").cloned().unwrap_or(Value::Null)
            );
        }
        AgentEvent::ItemCompleted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            let tool = item
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            let success = item
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let duration = item.get("durationMs").and_then(Value::as_u64).unwrap_or(0);
            println!("[ghost-workflow] tool <- {tool} success={success} duration_ms={duration}");
        }
        AgentEvent::TurnCompleted { status } => {
            println!("[ghost-workflow] agent turn completed: {status}");
        }
        _ => {}
    }
}
