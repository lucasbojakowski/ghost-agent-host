use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ghost_fl_studio::{FlStudioAdapterConfig, GopherNativeAdapter};
use serde_json::Value;

mod parameter_export;

#[derive(Debug, Parser)]
#[command(
    name = "fl-gopher-probe",
    about = "Inspect and invoke the live FL Studio/Gopher interface through ghost-fl-studio"
)]
struct Cli {
    #[arg(long, default_value_t = 9222)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the current Gopher target and complete live MCP tool catalog.
    Catalog,
    /// Invoke one raw live Gopher tool with a JSON object of named arguments.
    Call {
        tool: String,
        #[arg(long, default_value = "{}")]
        arguments: String,
    },
    /// Empirically export normalized/display parameter-space samples for mixer effects.
    ExportParameterSpaces(parameter_export::ExportArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let adapter_config = FlStudioAdapterConfig {
        debug_port: cli.debug_port,
        target_match: cli.target_match,
        ..Default::default()
    };
    match cli.command {
        Command::Catalog => {
            let adapter = GopherNativeAdapter::connect(adapter_config)
                .context("failed to connect to the FL Studio Gopher target")?;
            println!("{}", serde_json::to_string_pretty(&adapter.manifest()?)?);
        }
        Command::Call { tool, arguments } => {
            let adapter = GopherNativeAdapter::connect(adapter_config)
                .context("failed to connect to the FL Studio Gopher target")?;
            let arguments: Value = serde_json::from_str(&arguments)
                .context("--arguments must be valid JSON, normally an object")?;
            let result = adapter.call_native(&tool, arguments)?;
            println!("{}", serde_json::to_string_pretty(&result.raw)?);
        }
        Command::ExportParameterSpaces(args) => {
            parameter_export::run(adapter_config, args)?;
        }
    }
    Ok(())
}
