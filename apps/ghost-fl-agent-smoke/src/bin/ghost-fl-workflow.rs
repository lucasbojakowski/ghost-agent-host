use std::fs;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_codex::{
    AgentEvent, CodexParallelRuntime, ParallelThreadConfig, ToolRegistry, TurnOptions,
};
use ghost_context::{CompiledContext, ContextMessage, MessageRole, OutputContract};
use ghost_core::{
    analyze_audio, find_live_tap, read_audio, request_capture, wait_for_capture, AnalysisConfig,
    TapCaptureCommand,
};
use ghost_fl_studio::{
    register_codex_tools, FlAgentToolPolicy, FlPluginWriteScope, FlStudioAdapterConfig,
    GopherNativeAdapter,
};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-workflow",
    about = "Capture one live FL Studio signal with Ghost Tap, analyze it, and let one Codex App Server thread build a scoped processor chain"
)]
struct Cli {
    #[arg(long, default_value_t = 9222)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    /// Process-local Ghost Tap instance number. With one Ghost Tap loaded this is normally 0.
    #[arg(long, default_value_t = 0)]
    tap_instance: u32,

    #[arg(long, default_value_t = 4.0)]
    capture_seconds: f64,

    /// FL mixer track number/name passed to Gopher. Master is 0; Insert 1 is 1.
    #[arg(long, default_value = "1")]
    track: String,

    /// First mixer effect slot the agent may write. Keep Ghost Tap outside this range.
    #[arg(long, default_value_t = 1)]
    slot_start: u32,

    /// Last mixer effect slot the agent may write.
    #[arg(long, default_value_t = 4)]
    slot_end: u32,

    /// Exact installed plugin names the agent may insert. Repeat --plugin to add more.
    #[arg(long = "plugin")]
    plugins: Vec<String>,

    #[arg(
        long,
        default_value = "Build a clearly audible but musical processor chain for this sample from the measured evidence. Improve clarity, balance, dynamics, and usefulness in a mix while preserving the sample's identity."
    )]
    intent: String,

    /// Processing strength requested from the agent. 0 is corrective/subtle, 1 is strongly transformative.
    /// The default aims for an obvious A/B improvement without turning the workflow into sound design.
    #[arg(long, default_value_t = 0.70)]
    processing_intensity: f64,

    #[arg(long, default_value = "codex")]
    codex_binary: String,

    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,

    /// Print the full App Server event stream. By default Ghost logs only turn/tool milestones.
    #[arg(long)]
    verbose_agent_events: bool,

    /// Required safety acknowledgement: position the playhead just before the sample, stop transport,
    /// and confirm the requested target track/write range. Ghost live-checks slot occupancy before inserts.
    #[arg(
        long = "i-have-positioned-playhead-and-accepted-scoped-writes",
        alias = "i-have-positioned-playhead-and-confirmed-empty-slots"
    )]
    i_have_positioned_playhead_and_accepted_scoped_writes: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.i_have_positioned_playhead_and_accepted_scoped_writes {
        bail!(
            "refusing live processor writes: pass --i-have-positioned-playhead-and-accepted-scoped-writes after positioning the playhead, stopping transport, and confirming mixer track {} / slots {}..={}. Ghost will live-check occupancy before every insert",
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

    let adapter = Arc::new(
        GopherNativeAdapter::connect(FlStudioAdapterConfig {
            debug_port: cli.debug_port,
            target_match: cli.target_match.clone(),
            ..Default::default()
        })
        .context("failed to connect Ghost to the FL Studio Gopher native adapter")?,
    );
    let manifest = adapter.capability_manifest()?;
    println!(
        "[ghost-workflow] FL adapter connected with {} live native tools.",
        manifest.tools.len()
    );

    let tap = find_live_tap(cli.tap_instance).with_context(|| {
        format!(
            "Ghost Tap instance {} is not publishing a fresh status. Load the new Ghost Tap CLAP on the target mixer track and let FL activate it.",
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

    adapter.play().context("failed to start FL playback for Ghost Tap capture")?;
    let capture = wait_for_capture(
        &tap,
        command.request_id,
        Duration::from_secs_f64(cli.capture_seconds + 12.0),
    );
    let stop_result = adapter.stop();
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

    let scope = FlPluginWriteScope::new(
        cli.track.clone(),
        cli.slot_start,
        cli.slot_end,
        allowed_plugins.clone(),
    );
    let mut registry = ToolRegistry::default();
    register_codex_tools(
        &mut registry,
        Arc::clone(&adapter),
        FlAgentToolPolicy::single_track_processor(scope),
    )?;

    // Product code uses the parallel-capable dispatcher even though this first real workflow owns
    // exactly one agent thread. Parallel experiments can add threads without changing this boundary.
    let runtime = CodexParallelRuntime::spawn(&cli.codex_binary)
        .context("failed to launch the persistent Codex App Server runtime")?;
    let thread = runtime.start_thread(
        ParallelThreadConfig::new(cli.model.clone()).service_name("ghost_fl_processor"),
        registry,
    )?;
    println!(
        "[ghost-workflow] Started processor thread {} on one persistent Codex App Server.",
        thread.id
    );

    // Preserve the complete analysis artifact on disk, but expose a deliberately compact evidence
    // object to the agent. Dense frame-series data is useful for downstream DSP inspection, not for
    // every reasoning turn, and previously diluted the high-value measurements in the prompt.
    let agent_evidence = compact_agent_evidence(&analysis)?;
    let analysis_json = serde_json::to_string(&agent_evidence)?;
    let allowed = allowed_plugins.join(", ");
    let context = CompiledContext {
        schema_version: CompiledContext::SCHEMA.into(),
        messages: vec![
            ContextMessage {
                role: MessageRole::System,
                content: format!(
                    "You are Ghost's mixing/processor agent operating a controlled live FL Studio workflow. The measured audio evidence below describes the captured signal; interpret features jointly and do not treat any single metric as a command.\n\nYour live write boundary is mixer track {track}, slots {slot_start}..={slot_end}. The only insertable plugin names are: {allowed}. The slot range is a permission boundary, NOT a claim that every slot is empty. Start with fl_get_target_track_context. Preserve existing processors unless changing one is musically justified; fl_add_effect will fail closed rather than overwrite an occupied slot. Never remove/reset effects or touch another track.\n\nPROCESSING_INTENSITY={intensity:.2} on a 0..1 scale. At this setting, aim for a clearly audible A/B improvement while remaining musical. Do not make ceremonial or token parameter movements merely because a tool exists. If a processor is justified, set it far enough to matter for the measured problem. Conversely, do not accumulate processors or changes without evidence. One or two processors and a handful of purposeful settings is usually preferable to a long chain.\n\nTool strategy: fl_find_plugin_parameters treats space-separated terms as OR, so search groups such as `threshold ratio attack release knee range mix output` or `freq gain q used enabled`. Read exact controls with fl_get_plugin_parameter_value; it exposes the plugin's human display value when available. For continuous numeric controls, strongly prefer fl_set_plugin_parameter_display_value and specify musical targets in real units such as dB, Hz, ms, ratio, Q, or percent. Ghost will calibrate the normalized mapping while transport is stopped and verify the displayed result. Use fl_set_plugin_parameter_value only for a discrete/boolean/enum mapping you actually understand.\n\nFor dynamics, reason about threshold together with ratio, timing, range/knee and wet/dry when exposed; a tiny threshold nudge alone is not automatically useful. For EQ, use the measured spectral balance/resonances to justify specific bands and make frequency/gain/Q changes in display units when the controls are exposed. Existing defaults are not sacred, but preserve the sample's identity.\n\nThis is an action workflow: make at least one justified verified mutation. If a suitable processor already exists in the scoped slots, tuning it can satisfy that requirement; otherwise insert the smallest processor chain that serves the intent. Summarize only changes you actually verified.\n\nANALYSIS_EVIDENCE_JSON:\n{analysis_json}",
                    track = cli.track,
                    slot_start = cli.slot_start,
                    slot_end = cli.slot_end,
                    intensity = cli.processing_intensity,
                ),
            },
            ContextMessage {
                role: MessageRole::User,
                content: format!(
                    "Intent: {}\nApply a musically meaningful processor result at intensity {:.2}, then summarize what you actually changed, the displayed settings you verified, and why those changes follow from the evidence.",
                    cli.intent,
                    cli.processing_intensity
                ),
            },
        ],
        output: OutputContract::Text,
        metadata: json!({
            "workflow": "ghost.fl.tap-process/2",
            "captureRequestId": artifact.request_id,
            "capturePath": artifact.wav_path,
            "analysisPath": analysis_path,
            "targetTrack": cli.track,
            "slotRange": [cli.slot_start, cli.slot_end],
            "allowedPlugins": allowed_plugins,
            "processingIntensity": cli.processing_intensity
        }),
    };

    let journal_before = adapter.journal_snapshot()?.len();
    let output = runtime.run_turn(
        &thread,
        &context,
        &TurnOptions::default(),
        &mut |event| print_agent_event(&event, cli.verbose_agent_events),
    )?;
    let journal = adapter.journal_snapshot()?;
    let workflow_mutations = &journal[journal_before..];
    let mutated = workflow_mutations.iter().any(|record| record.verified);
    if !mutated {
        bail!("agent turn completed without a verified FL Studio mutation");
    }

    println!("[ghost-workflow] Agent: {}", output.text);
    println!(
        "[ghost-workflow] Verified workflow mutations:\n{}",
        serde_json::to_string_pretty(workflow_mutations)?
    );
    println!(
        "[ghost-workflow] GREEN: Ghost Tap captured live audio -> Rust analysis -> one Codex App Server processor thread -> scoped FL Studio processor mutation with native verification."
    );
    Ok(())
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
        if let Some(resonances) = spectrum
            .get_mut("resonances")
            .and_then(Value::as_array_mut)
        {
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
            println!("[ghost-workflow] agent turn started: {:?}", turn_id);
        }
        AgentEvent::ItemStarted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            println!(
                "[ghost-workflow] tool -> {} {}",
                item.get("tool").and_then(Value::as_str).unwrap_or("<unknown>"),
                item.get("arguments").cloned().unwrap_or(Value::Null)
            );
        }
        AgentEvent::ItemCompleted { item }
            if item.get("type").and_then(Value::as_str) == Some("dynamicToolCall") =>
        {
            println!(
                "[ghost-workflow] tool <- {} success={} duration_ms={}",
                item.get("tool").and_then(Value::as_str).unwrap_or("<unknown>"),
                item.get("success").and_then(Value::as_bool).unwrap_or(false),
                item.get("durationMs").and_then(Value::as_u64).unwrap_or(0)
            );
        }
        AgentEvent::TurnCompleted { status } => {
            println!("[ghost-workflow] agent turn completed: {status}");
        }
        _ => {}
    }
}
