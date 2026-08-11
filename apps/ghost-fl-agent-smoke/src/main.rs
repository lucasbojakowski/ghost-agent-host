use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_codex::{CodexAppServerHost, CodexThreadConfig, ToolRegistry, TurnOptions};
use ghost_context::{CompiledContext, ContextMessage, MessageRole, OutputContract};
use ghost_fl_studio::{
    register_codex_tools, FlAgentToolPolicy, FlStudioAdapterConfig, GopherNativeAdapter,
};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-agent-smoke",
    about = "Run a bounded multi-thread Codex App Server tool call against a live FL Studio project"
)]
struct Cli {
    #[arg(long, default_value_t = 9222)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[arg(long, default_value_t = 137)]
    target_bpm: u32,

    #[arg(long, default_value = "codex")]
    codex_binary: String,

    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,

    /// Leave the agent's tempo change in the project instead of restoring the original integer BPM.
    #[arg(long)]
    keep_change: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !(20..=300).contains(&cli.target_bpm) {
        bail!("--target-bpm must be between 20 and 300 for this bounded smoke test");
    }

    let config = FlStudioAdapterConfig {
        debug_port: cli.debug_port,
        target_match: cli.target_match.clone(),
        ..Default::default()
    };
    let adapter = Arc::new(
        GopherNativeAdapter::connect(config)
            .context("failed to connect the FL Studio Gopher native adapter")?,
    );

    let manifest = adapter.capability_manifest()?;
    println!(
        "[ghost-fl-agent-smoke] Connected adapter '{}' with {} live native tools.",
        manifest.adapter,
        manifest.tools.len()
    );

    let original = adapter.get_tempo()?;
    let original_rounded = original.round();
    if !cli.keep_change && (original - original_rounded).abs() > 0.01 {
        bail!(
            "The original tempo is {original:.4} BPM. This smoke test restores via the integer set_tempo tool; use a scratch project with an integer original tempo or pass --keep-change."
        );
    }
    if (original - cli.target_bpm as f64).abs() <= 0.01 {
        bail!(
            "FL Studio is already at {} BPM. Choose a different --target-bpm so the smoke test proves a real mutation.",
            cli.target_bpm
        );
    }

    println!(
        "[ghost-fl-agent-smoke] Original tempo: {original:.4} BPM. Agent target: {} BPM.",
        cli.target_bpm
    );

    // Start one long-lived app-server process. The `codex` executable is only the launcher;
    // this host speaks the app-server JSON-RPC protocol and owns multiple Codex threads.
    let mut app_server = CodexAppServerHost::spawn(&cli.codex_binary)
        .context("failed to start the persistent Codex App Server host")?;

    let mut controller_tools = ToolRegistry::default();
    register_codex_tools(
        &mut controller_tools,
        Arc::clone(&adapter),
        FlAgentToolPolicy::tempo_smoke(),
    )?;
    let controller = app_server
        .start_thread(
            CodexThreadConfig::new(&cli.model).service_name("ghost_fl_controller"),
            controller_tools,
        )
        .context("failed to start the FL controller thread")?;

    println!(
        "[ghost-fl-agent-smoke] Persistent app-server controller thread: {}. It receives ONLY fl_get_tempo and fl_set_tempo.",
        controller.id
    );

    let journal_before = adapter.journal_snapshot()?.len();
    let context = CompiledContext {
        schema_version: CompiledContext::SCHEMA.into(),
        messages: vec![
            ContextMessage {
                role: MessageRole::System,
                content: format!(
                    "You are running a controlled live FL Studio integration test. You have exactly two FL tools: fl_get_tempo and fl_set_tempo. First call fl_get_tempo. Then call fl_set_tempo exactly once with bpm={}. Do not merely describe the action; execute it with the tool. After the tool succeeds, reply with a short confirmation including the verified BPM.",
                    cli.target_bpm
                ),
            },
            ContextMessage {
                role: MessageRole::User,
                content: format!(
                    "Set the live FL Studio project tempo to {} BPM using the provided FL tool and verify the result.",
                    cli.target_bpm
                ),
            },
        ],
        output: OutputContract::Text,
        metadata: json!({
            "test": "ghost.fl.codex-app-server-tempo-smoke/2",
            "targetBpm": cli.target_bpm,
            "originalBpm": original,
            "controllerThreadId": controller.id
        }),
    };

    let mut options = TurnOptions::default();
    options.summary = "concise".into();

    let turn = app_server.run_turn(&controller, &context, &options, &mut |event| {
        println!("[ghost-fl-agent-smoke] controller event: {event:?}");
    });

    let after_turn = adapter.get_tempo();
    let journal_after = adapter.journal_snapshot();

    let restore_result = if cli.keep_change {
        Ok(None)
    } else {
        println!(
            "[ghost-fl-agent-smoke] Restoring original tempo {:.0} BPM through the same verified adapter path ...",
            original_rounded
        );
        adapter
            .set_tempo_verified(original_rounded as u32)
            .map(Some)
            .map_err(anyhow::Error::from)
    };

    let output = turn.context("Codex App Server controller turn failed")?;
    let after_turn = after_turn.context("failed to read FL tempo after Codex turn")?;
    let journal_after = journal_after?;
    let agent_mutations = &journal_after[journal_before..];
    let agent_called_set_tempo = agent_mutations
        .iter()
        .any(|record| record.tool == "set_tempo" && record.verified);

    println!("[ghost-fl-agent-smoke] Codex final response: {}", output.text);
    println!(
        "[ghost-fl-agent-smoke] Tempo after controller turn: {after_turn:.4} BPM; mutation records observed: {}.",
        agent_mutations.len()
    );

    if !agent_called_set_tempo {
        bail!("Codex completed without a verified fl_set_tempo mutation record");
    }
    if (after_turn - cli.target_bpm as f64).abs() > 0.01 {
        bail!(
            "Codex tool call was observed, but FL Studio read back {after_turn:.4} BPM instead of {} BPM",
            cli.target_bpm
        );
    }

    if let Some(restored) = restore_result? {
        if !restored.verified {
            bail!("tempo restoration did not verify");
        }
        println!(
            "[ghost-fl-agent-smoke] Restoration verified at {} BPM.",
            restored.after
        );
    } else {
        println!("[ghost-fl-agent-smoke] --keep-change set; leaving the agent mutation in place.");
    }

    // Prove the same initialized app-server process can own a second Ghost thread with a
    // different tool scope. No second Codex process is spawned here.
    let mut observer_tools = ToolRegistry::default();
    register_codex_tools(
        &mut observer_tools,
        Arc::clone(&adapter),
        FlAgentToolPolicy {
            inspect_session: false,
            read_tempo: true,
            set_tempo: false,
            transport: false,
            plugin_write_scope: None,
        },
    )?;
    let observer = app_server
        .start_thread(
            CodexThreadConfig::new(&cli.model).service_name("ghost_fl_observer"),
            observer_tools,
        )
        .context("failed to start the FL observer thread on the existing app-server")?;
    let loaded = app_server.loaded_thread_ids()?;
    println!(
        "[ghost-fl-agent-smoke] Same app-server loaded threads: {:?}",
        loaded
    );
    if !loaded.contains(&controller.id) || !loaded.contains(&observer.id) {
        bail!(
            "persistent app-server did not report both Ghost threads loaded (controller={}, observer={})",
            controller.id,
            observer.id
        );
    }

    println!(
        "[ghost-fl-agent-smoke] GREEN: one persistent Codex App Server hosted controller + observer threads; the controller selected a Ghost dynamic tool, Ghost executed it through GopherNativeAdapter, FL Studio changed state, native readback verified the action{}.",
        if cli.keep_change { "" } else { ", and Ghost restored the original tempo" }
    );
    Ok(())
}
