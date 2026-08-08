use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use ghost_application::{analyze_path, NoProgress};
use ghost_codex::{CodexAppServerAgent, MixingAgent, MockMixingAgent};
use ghost_core::{AnalysisConfig, UserIntent};
use ghost_core::{
    ProtocolError, RequestEnvelope, ResponseEnvelope, ResponseStatus, PROTOCOL_VERSION,
};
use ghost_db::GhostDatabase;
use ghost_mix::{build_prompt_bundle, validate_mix_plan, PluginCapabilitySummary};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(
    name = "ghost-agentd",
    version,
    about = "Local Ghost analysis and agent daemon"
)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:47644")]
    listen: String,
    #[arg(long, default_value = ".ghost/ghost.db")]
    database: PathBuf,
    #[arg(long, default_value = ".ghost/artifacts")]
    artifact_root: PathBuf,
    #[arg(long, value_enum, default_value_t = AgentKind::Mock)]
    agent: AgentKind,
    #[arg(long, default_value = "codex")]
    codex_binary: String,
    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentKind {
    Mock,
    Codex,
}

struct AppState {
    database: Mutex<GhostDatabase>,
    agent: Mutex<Box<dyn MixingAgent>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
enum Request {
    Health,
    Analyze {
        path: PathBuf,
        #[serde(default)]
        config: Option<AnalysisConfig>,
    },
    Propose {
        path: PathBuf,
        intent: Box<UserIntent>,
        #[serde(default)]
        config: Option<AnalysisConfig>,
    },
    Stats,
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    result: Option<Value>,
    error: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ghost_agentd=info".into()),
        )
        .init();
    let args = Args::parse();
    let database = GhostDatabase::open(&args.database, &args.artifact_root)?;
    let agent: Box<dyn MixingAgent> = match args.agent {
        AgentKind::Mock => Box::new(MockMixingAgent),
        AgentKind::Codex => Box::new(CodexAppServerAgent::spawn(&args.codex_binary, args.model)?),
    };
    let state = Arc::new(AppState {
        database: Mutex::new(database),
        agent: Mutex::new(agent),
    });
    let listener = TcpListener::bind(&args.listen)
        .with_context(|| format!("failed to bind {}", args.listen))?;
    tracing::info!(address = %args.listen, "Ghost agent daemon ready");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(error) = handle_client(stream, state) {
                        tracing::warn!(%error, "client connection failed");
                    }
                });
            }
            Err(error) => tracing::warn!(%error, "accept failed"),
        }
    }
    Ok(())
}

fn handle_client(mut stream: TcpStream, state: Arc<AppState>) -> Result<()> {
    stream.set_nodelay(true)?;
    let reader_stream = stream.try_clone()?;
    let mut reader = BufReader::new(reader_stream);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let response = handle_line(&line, &state);
        serde_json::to_writer(&mut stream, &response)?;
        stream.write_all(b"\n")?;
        stream.flush()?;
    }
    Ok(())
}

fn handle_line(line: &str, state: &AppState) -> Value {
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            return serde_json::to_value(Response {
                ok: false,
                result: None,
                error: Some(format!("invalid request: {error}")),
            })
            .expect("legacy response serializes");
        }
    };
    if value.get("protocol").is_some() {
        return handle_envelope(value, state);
    }
    let response = match serde_json::from_value::<Request>(value) {
        Ok(request) => match execute(request, state) {
            Ok(result) => Response {
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => Response {
                ok: false,
                result: None,
                error: Some(error.to_string()),
            },
        },
        Err(error) => Response {
            ok: false,
            result: None,
            error: Some(format!("invalid request: {error}")),
        },
    };
    serde_json::to_value(response).expect("legacy response serializes")
}

fn handle_envelope(value: Value, state: &AppState) -> Value {
    let envelope: RequestEnvelope = match serde_json::from_value(value) {
        Ok(envelope) => envelope,
        Err(error) => return json!({"error": format!("invalid envelope: {error}")}),
    };
    let result = if envelope.protocol != PROTOCOL_VERSION {
        Err(anyhow::anyhow!(
            "unsupported protocol {}",
            envelope.protocol
        ))
    } else {
        envelope_request(&envelope).and_then(|request| execute(request, state))
    };
    let response = match result {
        Ok(payload) => ResponseEnvelope {
            protocol: PROTOCOL_VERSION.into(),
            request_id: envelope.request_id,
            status: ResponseStatus::Complete,
            payload,
            error: None,
        },
        Err(error) => ResponseEnvelope {
            protocol: PROTOCOL_VERSION.into(),
            request_id: envelope.request_id,
            status: ResponseStatus::Failed,
            payload: Value::Null,
            error: Some(ProtocolError {
                code: "request_failed".into(),
                message: error.to_string(),
                retryable: false,
            }),
        },
    };
    serde_json::to_value(response).expect("protocol response serializes")
}

fn envelope_request(envelope: &RequestEnvelope) -> Result<Request> {
    match envelope.operation.as_str() {
        "health" => Ok(Request::Health),
        "stats" => Ok(Request::Stats),
        "analyze" => {
            #[derive(Deserialize)]
            struct Payload {
                path: PathBuf,
                #[serde(default)]
                config: Option<AnalysisConfig>,
            }
            let payload: Payload = serde_json::from_value(envelope.payload.clone())?;
            Ok(Request::Analyze {
                path: payload.path,
                config: payload.config,
            })
        }
        "propose" => {
            #[derive(Deserialize)]
            struct Payload {
                path: PathBuf,
                intent: UserIntent,
                #[serde(default)]
                config: Option<AnalysisConfig>,
            }
            let payload: Payload = serde_json::from_value(envelope.payload.clone())?;
            Ok(Request::Propose {
                path: payload.path,
                intent: Box::new(payload.intent),
                config: payload.config,
            })
        }
        operation => Err(anyhow::anyhow!("unknown operation `{operation}`")),
    }
}

fn execute(request: Request, state: &AppState) -> Result<Value> {
    match request {
        Request::Health => Ok(json!({
            "status": "ready",
            "protocol": "ghost.agentd-jsonl/1"
        })),
        Request::Stats => {
            let database = state
                .database
                .lock()
                .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
            Ok(serde_json::to_value(database.counts()?)?)
        }
        Request::Analyze { path, config } => {
            let analysis = analyze_path(&path, &config.unwrap_or_default(), &NoProgress)?;
            let database = state
                .database
                .lock()
                .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
            let run_id = database.store_analysis(&analysis)?;
            Ok(json!({ "analysis_run_id": run_id, "analysis": analysis }))
        }
        Request::Propose {
            path,
            intent,
            config,
        } => {
            let intent = *intent;
            let analysis = analyze_path(&path, &config.unwrap_or_default(), &NoProgress)?;
            let system_prompt = std::fs::read_to_string("prompts/system.md")?;
            let capabilities = default_capabilities();
            let prompt =
                build_prompt_bundle(system_prompt, intent.clone(), &analysis, &capabilities)?;
            let (analysis_run_id, request_id) = {
                let database = state
                    .database
                    .lock()
                    .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
                let analysis_run_id = database.store_analysis(&analysis)?;
                let request_id = database.store_mix_request(analysis_run_id, &intent, &prompt)?;
                (analysis_run_id, request_id)
            };

            let (agent_run_id, plan) = {
                let mut agent = state
                    .agent
                    .lock()
                    .map_err(|_| anyhow::anyhow!("agent lock poisoned"))?;
                let agent_run_id = {
                    let database = state
                        .database
                        .lock()
                        .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
                    database.begin_agent_run(request_id, agent.backend_name(), None)?
                };
                let plan = agent.propose(&prompt)?;
                (agent_run_id, plan)
            };

            validate_mix_plan(&plan)?;
            let encoded = serde_json::to_string_pretty(&plan)?;
            let plan_id = {
                let database = state
                    .database
                    .lock()
                    .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
                database.complete_agent_run(agent_run_id, &encoded)?;
                database.store_mix_plan(agent_run_id, &plan, "valid", json!({ "ok": true }))?
            };
            Ok(json!({
                "analysis_run_id": analysis_run_id,
                "mix_plan_id": plan_id,
                "analysis": analysis,
                "plan": plan
            }))
        }
    }
}

fn default_capabilities() -> Vec<PluginCapabilitySummary> {
    vec![
        PluginCapabilitySummary {
            plugin: "Equalizer role".into(),
            version: "scanned-public-interface".into(),
            supported_operations: vec!["equalizer.band".into()],
            safety_notes: vec!["Map semantic values to scanned parameters".into()],
            public_parameters: Vec::new(),
        },
        PluginCapabilitySummary {
            plugin: "Compressor role".into(),
            version: "scanned-public-interface".into(),
            supported_operations: vec!["compressor.settings".into()],
            safety_notes: vec!["Validate capabilities before apply".into()],
            public_parameters: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn protocol_envelope_preserves_health_request_correlation() {
        let root = std::env::temp_dir().join(format!("ghost-agentd-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let state = AppState {
            database: Mutex::new(
                GhostDatabase::open(root.join("ghost.db"), root.join("artifacts")).unwrap(),
            ),
            agent: Mutex::new(Box::new(MockMixingAgent)),
        };
        let request_id = Uuid::new_v4();
        let response = handle_line(
            &serde_json::to_string(&RequestEnvelope {
                protocol: PROTOCOL_VERSION.into(),
                request_id,
                operation: "health".into(),
                payload: Value::Null,
            })
            .unwrap(),
            &state,
        );
        assert_eq!(response["request_id"], json!(request_id));
        assert_eq!(response["status"], "complete");
        let _ = std::fs::remove_dir_all(root);
    }
}
