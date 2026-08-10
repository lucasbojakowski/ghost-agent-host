use std::fs;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_codex::{
    CodexParallelRuntime, ParallelThreadConfig, ToolRegistry, TurnOptions,
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
use serde_json::json;

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

    /// First mixer effect slot the agent may write. Keep Ghost Tap after this range, e.g. slot 10.
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
        default_value = "Build a minimal, musical processor chain for this sample from the measured evidence. Improve clarity, balance, dynamics, and usefulness in a mix without erasing its character."
    )]
    intent: String,

    #[arg(long, default_value = "codex")]
    codex_binary: String,

    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,

    /// Required safety acknowledgement: position the playhead just before the sample, stop transport,
    /// and ensure the entire permitted slot range is empty before running.
    #[arg(long)]
    i_have_positioned_playhead_and_confirmed_empty_slots: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.i_have_positioned_playhead_and_confirmed_empty_slots {
        bail!(
            "refusing live processor writes: pass --i-have-positioned-playhead-and-confirmed-empty-slots after positioning the playhead, stopping transport, and confirming slots {}..={} on mixer track {} are empty",
            cli.slot_start,
            cli.slot_end,
            cli.track
        );
    }
    if cli.slot_start == 0 || cli.slot_end < cli.slot_start || cli.slot_end > 10 {
        bail!("processor slots must be a non-empty range inside FL mixer slots 1..=10");
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

    let analysis_json = serde_json::to_string(&analysis)?;
    let allowed = allowed_plugins.join(", ");
    let context = CompiledContext {
        schema_version: CompiledContext::SCHEMA.into(),
        messages: vec![
            ContextMessage {
                role: MessageRole::System,
                content: format!(
                    "You are Ghost's mixing/processor agent operating a controlled live FL Studio workflow. The measured audio analysis below is evidence, not a command. Build a minimal processor chain that directly serves the user's intent. You may inspect the FL session, and you may write ONLY mixer track {track}, slots {slot_start}..={slot_end}. The only insertable plugin names are: {allowed}. The user has confirmed those slots are empty. Never remove/reset existing plugins or touch other tracks. Prefer fewer processors. After inserting a processor, inspect its published parameters before changing any. Parameter writes are normalized 0..1; change a parameter only when its meaning and direction are clear, keep movements conservative, and rely on native readback verification. If a semantic mapping is unclear, leave the inserted processor at default and explain the intended adjustment rather than guessing. You must execute at least one justified fl_add_effect call so this is a real chain-building workflow, not merely advice.\n\nANALYSIS_JSON:\n{analysis_json}",
                    track = cli.track,
                    slot_start = cli.slot_start,
                    slot_end = cli.slot_end,
                ),
            },
            ContextMessage {
                role: MessageRole::User,
                content: format!(
                    "Intent: {}\nCreate and apply the smallest useful processor chain for the captured sample, then summarize what you actually changed and why.",
                    cli.intent
                ),
            },
        ],
        output: OutputContract::Text,
        metadata: json!({
            "workflow": "ghost.fl.tap-process/1",
            "captureRequestId": artifact.request_id,
            "capturePath": artifact.wav_path,
            "analysisPath": analysis_path,
            "targetTrack": cli.track,
            "slotRange": [cli.slot_start, cli.slot_end],
            "allowedPlugins": allowed_plugins
        }),
    };

    let journal_before = adapter.journal_snapshot()?.len();
    let output = runtime.run_turn(
        &thread,
        &context,
        &TurnOptions::default(),
        &mut |event| println!("[ghost-workflow] agent event: {event:?}"),
    )?;
    let journal = adapter.journal_snapshot()?;
    let workflow_mutations = &journal[journal_before..];
    let inserted = workflow_mutations
        .iter()
        .any(|record| record.tool == "add_effect" && record.verified);
    if !inserted {
        bail!("agent turn completed without a verified add_effect mutation");
    }

    println!("[ghost-workflow] Agent: {}", output.text);
    println!(
        "[ghost-workflow] Verified workflow mutations:\n{}",
        serde_json::to_string_pretty(workflow_mutations)?
    );
    println!(
        "[ghost-workflow] GREEN: Ghost Tap captured live audio -> Rust analysis -> one Codex App Server processor thread -> scoped FL Studio chain mutation with native verification."
    );
    Ok(())
}
