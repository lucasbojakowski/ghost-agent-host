use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ghost_application::{analyze_path, NoProgress};
use ghost_codex::{CodexAppServerAgent, MixingAgent, MockMixingAgent};
use ghost_core::{
    analyze_audio, read_audio, write_wav_f32, AnalysisConfig, RequestEnvelope, UserIntent,
    PROTOCOL_VERSION,
};
use ghost_db::GhostDatabase;
use ghost_host::{
    default_clap_directories, discover_clap_files, AudioBlock, HostedChain, MockFabFilterChain,
    NativeClapSession, ProcessConfig,
};
use ghost_mix::{build_prompt_bundle, validate_mix_plan, MixPlan, PluginCapabilitySummary};
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
    Plugins {
        /// Search root. Uses Windows CLAP defaults and CLAP_PATH when omitted.
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
    },
    /// Instantiate and process a native child without an agent or DAW.
    NativeSmoke {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        plugin_id: String,
        #[arg(long)]
        parameter_id: Option<String>,
        #[arg(long)]
        parameter_value: Option<f64>,
    },
    ClapGuiSmoke {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        plugin_id: String,
    },
    ClapAudioSmoke {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        plugin_id: String,
        #[arg(long)]
        state_json: Option<String>,
    },
    DaemonHealth {
        #[arg(long, default_value = "127.0.0.1:47644")]
        address: String,
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
            let config = resolve_analysis_config(profile, analysis_config.as_deref())?;
            let analysis = analyze_path(&input, &config, &NoProgress)
                .with_context(|| format!("failed to analyze {}", input.display()))?;
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
        Command::Plugins { paths } => {
            let roots = if paths.is_empty() {
                default_clap_directories()
            } else {
                paths
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "roots": roots,
                    "plugins": discover_clap_files(&roots),
                }))?
            );
        }
        Command::NativeSmoke {
            path,
            plugin_id,
            parameter_id,
            parameter_value,
        } => native_smoke(&path, &plugin_id, parameter_id.as_deref(), parameter_value)?,
        Command::ClapGuiSmoke { path, plugin_id } => {
            let size = ghost_host::clack_runtime::smoke_test_clap_gui_id(path, &plugin_id)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            println!("embedded GUI {}×{} passed", size.0, size.1);
        }
        Command::ClapAudioSmoke {
            path,
            plugin_id,
            state_json,
        } => {
            let state = state_json.as_deref().map(str::as_bytes);
            let result = ghost_host::clack_runtime::smoke_test_clap_audio(path, &plugin_id, state)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            println!("{}", serde_json::to_string(&result)?);
        }
        Command::DaemonHealth { address } => daemon_health(&address)?,
    }
    Ok(())
}

fn native_smoke(
    path: &Path,
    plugin_id: &str,
    parameter_id: Option<&str>,
    parameter_value: Option<f64>,
) -> Result<()> {
    let config = ProcessConfig {
        sample_rate: 48_000,
        maximum_frames: 64,
        channels: 2,
    };
    let mut session = NativeClapSession::open(path, plugin_id, config)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    if let (Some(id), Some(value)) = (parameter_id, parameter_value) {
        session
            .set_parameter_plain(id, value)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        session
            .flush_parameter_events()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }
    let mut left = [0.25_f32; 64];
    let mut right = [-0.25_f32; 64];
    let mut channels: [&mut [f32]; 2] = [&mut left, &mut right];
    session
        .process(&mut AudioBlock {
            channels: &mut channels,
            frames: 64,
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let saved = session
        .save_state()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    session
        .load_state(&saved)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    println!(
        "{}",
        serde_json::json!({
            "plugin": session.descriptor().name,
            "parameters": session.descriptor().parameters.len(),
            "state_bytes": saved.bytes.len(),
            "first_output": [left[0], right[0]],
        })
    );
    Ok(())
}

fn daemon_health(address: &str) -> Result<()> {
    let mut stream = TcpStream::connect(address)
        .with_context(|| format!("failed to connect to daemon at {address}"))?;
    let request = RequestEnvelope {
        protocol: PROTOCOL_VERSION.into(),
        request_id: Uuid::new_v4(),
        operation: "health".into(),
        payload: serde_json::Value::Null,
    };
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    let value: serde_json::Value = serde_json::from_str(&response)?;
    if value["request_id"] != serde_json::json!(request.request_id) {
        anyhow::bail!("daemon returned a mismatched request id");
    }
    println!("{}", serde_json::to_string_pretty(&value)?);
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
    let audio = read_audio(fixture)?;
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
            public_parameters: Vec::new(),
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
            public_parameters: Vec::new(),
        },
    ];
    let prompt_bundle = build_prompt_bundle(
        system_prompt,
        intent.clone(),
        &source_analysis,
        &capabilities,
    )?;
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
    database.store_mix_plan(
        agent_run_id,
        &plan,
        "valid",
        serde_json::json!({"ok": true}),
    )?;

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
    println!(
        "{}",
        serde_json::to_string_pretty(&evaluation.metric_deltas)?
    );
    Ok(())
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
