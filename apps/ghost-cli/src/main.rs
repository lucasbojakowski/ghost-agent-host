use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ghost_codex::{CodexAppServerAgent, MixingAgent, MockMixingAgent};
use ghost_core::prompt::PluginCapabilitySummary;
use ghost_core::{
    analyze_audio, build_prompt_bundle, read_wav, validate_mix_plan, write_wav_f32,
    AnalysisConfig, MixPlan, UserIntent,
};
use ghost_db::GhostDatabase;
use ghost_host::{HostedChain, MockFabFilterChain};
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "ghost", version, about = "Ghost Agent Host validation CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Analyze {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = ProfileArg::Maximum)]
        profile: ProfileArg,
        /// Optional TOML file containing an [analysis] table. Overrides --profile.
        #[arg(long)]
        analysis_config: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Demo {
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long)]
        intent: String,
        #[arg(long, value_enum, default_value_t = ProfileArg::Maximum)]
        profile: ProfileArg,
        /// Optional TOML file containing an [analysis] table. Overrides --profile.
        #[arg(long)]
        analysis_config: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = AgentArg::Mock)]
        agent: AgentArg,
        #[arg(long, default_value = "codex")]
        codex_binary: String,
        #[arg(long, default_value = "gpt-5.6-terra")]
        model: String,
        #[arg(long, default_value = ".ghost/ghost.db")]
        database: PathBuf,
        #[arg(long, default_value = ".ghost/artifacts")]
        artifact_root: PathBuf,
        #[arg(long, default_value = "artifacts/demo")]
        output_dir: PathBuf,
    },
    Schema {
        #[arg(long, default_value = "schemas/mix_plan.generated.schema.json")]
        output: PathBuf,
    },
    DbStats {
        #[arg(long, default_value = ".ghost/ghost.db")]
        database: PathBuf,
        #[arg(long, default_value = ".ghost/artifacts")]
        artifact_root: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProfileArg {
    Live,
    High,
    Maximum,
}

impl ProfileArg {
    fn config(self) -> AnalysisConfig {
        match self {
            Self::Live => AnalysisConfig::live(),
            Self::High => AnalysisConfig::high(),
            Self::Maximum => AnalysisConfig::maximum(),
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentArg {
    Mock,
    Codex,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    analysis: AnalysisConfig,
}

#[derive(Debug, Serialize)]
struct DemoEvaluation {
    run_id: Uuid,
    backend: String,
    source_analysis: ghost_core::AnalysisBundle,
    processed_analysis: ghost_core::AnalysisBundle,
    mix_plan: MixPlan,
    metric_deltas: MetricDeltas,
}

#[derive(Debug, Serialize)]
struct MetricDeltas {
    rms_db: f64,
    crest_db: f64,
    spectral_centroid_hz: f64,
    low_mid_db: f64,
    stereo_correlation: f64,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ghost=info".into()),
        )
        .init();
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze {
            input,
            profile,
            analysis_config,
            output,
        } => {
            let audio = read_wav(&input)
                .with_context(|| format!("failed to read {}", input.display()))?;
            let config = resolve_analysis_config(profile, analysis_config.as_deref())?;
            let analysis = analyze_audio(input.display().to_string(), &audio, &config)?;
            let encoded = serde_json::to_string_pretty(&analysis)?;
            if let Some(output) = output {
                ensure_parent(&output)?;
                fs::write(output, encoded)?;
            } else {
                println!("{encoded}");
            }
        }
        Command::Demo {
            fixture,
            intent,
            profile,
            analysis_config,
            agent,
            codex_binary,
            model,
            database,
            artifact_root,
            output_dir,
        } => run_demo(
            &fixture,
            intent,
            resolve_analysis_config(profile, analysis_config.as_deref())?,
            agent,
            &codex_binary,
            &model,
            &database,
            &artifact_root,
            &output_dir,
        )?,
        Command::Schema { output } => {
            ensure_parent(&output)?;
            let schema = schema_for!(MixPlan);
            fs::write(&output, serde_json::to_vec_pretty(&schema)?)?;
            println!("wrote {}", output.display());
        }
        Command::DbStats {
            database,
            artifact_root,
        } => {
            let database = GhostDatabase::open(database, artifact_root)?;
            println!("{}", serde_json::to_string_pretty(&database.counts()?)?);
        }
    }
    Ok(())
}


fn resolve_analysis_config(profile: ProfileArg, path: Option<&Path>) -> Result<AnalysisConfig> {
    let mut config = if let Some(path) = path {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read analysis config {}", path.display()))?;
        toml::from_str::<FileConfig>(&text)
            .with_context(|| format!("failed to parse analysis config {}", path.display()))?
            .analysis
    } else {
        profile.config()
    };
    if path.is_some() {
        config.profile = ghost_core::QualityProfile::Custom;
    }
    config.validate().map_err(anyhow::Error::msg)?;
    Ok(config)
}

#[allow(clippy::too_many_arguments)]
fn run_demo(
    fixture: &Path,
    intent_text: String,
    config: AnalysisConfig,
    agent_kind: AgentArg,
    codex_binary: &str,
    model: &str,
    database_path: &Path,
    artifact_root: &Path,
    output_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let database = GhostDatabase::open(database_path, artifact_root)?;
    let audio = read_wav(fixture)?;
    let source_analysis = analyze_audio(fixture.display().to_string(), &audio, &config)?;
    let analysis_run_id = database.store_analysis(&source_analysis)?;
    let system_prompt = fs::read_to_string("prompts/system.md")
        .context("run the CLI from the repository root so prompts/system.md is available")?;
    let intent = UserIntent::Freeform {
        prompt: intent_text,
    };
    let capabilities = vec![
        PluginCapabilitySummary {
            plugin: "FabFilter Pro-Q 4".into(),
            version: "validated-at-runtime".into(),
            supported_operations: vec![
                "static bell EQ".into(),
                "dynamic bell EQ".into(),
                "channel placement".into(),
            ],
            safety_notes: vec!["Never invent raw parameter IDs.".into()],
        },
        PluginCapabilitySummary {
            plugin: "FabFilter Pro-C 3".into(),
            version: "validated-at-runtime".into(),
            supported_operations: vec![
                "threshold/ratio/knee".into(),
                "attack/release".into(),
                "range/mix/output".into(),
            ],
            safety_notes: vec!["Style names require runtime manifest validation.".into()],
        },
    ];
    let prompt_bundle = build_prompt_bundle(system_prompt, intent.clone(), &source_analysis, &capabilities)?;
    let request_id = database.store_mix_request(analysis_run_id, &intent, &prompt_bundle)?;

    let mut agent: Box<dyn MixingAgent> = match agent_kind {
        AgentArg::Mock => Box::new(MockMixingAgent),
        AgentArg::Codex => Box::new(CodexAppServerAgent::spawn(codex_binary, model)?),
    };
    let agent_run_id = database.begin_agent_run(request_id, agent.backend_name(), Some(model))?;
    let plan = agent.propose(&prompt_bundle)?;
    let plan_text = serde_json::to_string_pretty(&plan)?;
    database.complete_agent_run(agent_run_id, &plan_text)?;
    validate_mix_plan(&plan)?;
    database.store_mix_plan(agent_run_id, &plan, "valid", serde_json::json!({"ok": true}))?;

    let mut host = MockFabFilterChain::default();
    let processed = host.render(&audio, &plan)?;
    let processed_analysis = analyze_audio("mock-processed", &processed, &config)?;
    database.store_analysis(&processed_analysis)?;

    let source = &source_analysis.signal;
    let output = &processed_analysis.signal;
    let evaluation = DemoEvaluation {
        run_id: Uuid::new_v4(),
        backend: host.backend_name().into(),
        metric_deltas: MetricDeltas {
            rms_db: output.loudness.rms_dbfs - source.loudness.rms_dbfs,
            crest_db: output.loudness.crest_factor_db - source.loudness.crest_factor_db,
            spectral_centroid_hz: output.spectrum.centroid_hz - source.spectrum.centroid_hz,
            low_mid_db: output.spectrum.bands.low_mid_db - source.spectrum.bands.low_mid_db,
            stereo_correlation: output.stereo.broadband_correlation
                - source.stereo.broadband_correlation,
        },
        source_analysis,
        processed_analysis,
        mix_plan: plan,
    };

    fs::write(
        output_dir.join("prompt_bundle.json"),
        serde_json::to_vec_pretty(&prompt_bundle)?,
    )?;
    fs::write(
        output_dir.join("evaluation.json"),
        serde_json::to_vec_pretty(&evaluation)?,
    )?;
    write_wav_f32(output_dir.join("processed.wav"), &processed)?;
    println!("{}", serde_json::to_string_pretty(&evaluation.metric_deltas)?);
    Ok(())
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
