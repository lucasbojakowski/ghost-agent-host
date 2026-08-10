use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_codex::{AgentRuntime, CodexAppServerAgent, ToolRegistry, TurnOptions};
use ghost_context::{CompiledContext, ContextMessage, MessageRole, OutputContract};
use ghost_fl_studio::{
    register_codex_tools, FlAgentToolPolicy, FlStudioAdapterConfig, GopherNativeAdapter,
};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-agent-smoke",
    about = "Run one bounded Codex tool call against a live FL Studio project"
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
    println!(
        "[ghost-fl-agent-smoke] Codex will receive ONLY fl_get_tempo and fl_set_tempo. The write tool verifies native readback."
    );

    let journal_before = adapter.journal_snapshot()?.len();
    let mut registry = ToolRegistry::default();
    register_codex_tools(
        &mut registry,
        Arc::clone(&adapter),
        FlAgentToolPolicy::tempo_smoke(),
    )?;

    let mut agent = CodexAppServerAgent::spawn_with_tools(&cli.codex_binary, &cli.model, registry)
        .context("failed to start Codex App Server with FL Studio dynamic tools")?;

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
            "test": "ghost.fl.codex-tempo-smoke/1",
            "targetBpm": cli.target_bpm,
            "originalBpm": original
        }),
    };

    let mut options = TurnOptions::default();
    options.summary = "concise".into();

    let turn = agent.run_turn(&context, &options, &mut |event| {
        println!("[ghost-fl-agent-smoke] agent event: {event:?}");
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

    let output = turn.context("Codex turn failed")?;
    let after_turn = after_turn.context("failed to read FL tempo after Codex turn")?;
    let journal_after = journal_after?;
    let agent_mutations = &journal_after[journal_before..];
    let agent_called_set_tempo = agent_mutations
        .iter()
        .any(|record| record.tool == "set_tempo" && record.verified);

    println!("[ghost-fl-agent-smoke] Codex final response: {}", output.text);
    println!(
        "[ghost-fl-agent-smoke] Tempo after Codex turn: {after_turn:.4} BPM; mutation records observed: {}.",
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

    println!(
        "[ghost-fl-agent-smoke] GREEN: Codex selected a Ghost dynamic tool, Ghost executed it through GopherNativeAdapter, FL Studio changed state, native readback verified the action{}.",
        if cli.keep_change { "" } else { ", and Ghost restored the original tempo" }
    );
    Ok(())
}
