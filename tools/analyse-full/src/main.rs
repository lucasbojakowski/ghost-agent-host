use std::fmt::Write as _;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_audio::{analyze_audio, read_audio, AnalysisConfig};
use ghost_tap::{
    discover_live_taps, find_live_tap, request_capture, wait_for_capture, TapCaptureCommand,
    TapStatus,
};
use serde_json::Value;

const DEFAULT_CAPTURE_SECONDS: f64 = 10.0;
const SIGNAL_WAIT_SECONDS: f64 = 30.0;

#[derive(Debug, Parser)]
#[command(
    name = "analyse-full",
    about = "Capture a live FL Studio Ghost Tap and print a full maximum-quality Markdown analysis",
    after_help = "Markdown is written to stdout; capture progress is written to stderr."
)]
struct Cli {
    /// Capture length in seconds (0.05 through 20).
    #[arg(
        long = "length",
        visible_alias = "duration",
        default_value_t = DEFAULT_CAPTURE_SECONDS,
        value_parser = parse_capture_length
    )]
    capture_seconds: f64,

    /// Ghost Tap instance ID to capture (auto-selected when exactly one is live).
    #[arg(long)]
    tap_instance: Option<u32>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let tap = select_tap(cli.tap_instance)?;

    eprintln!(
        "[analyse-full] Found Tap {} in FL Studio process {} (sample rate {}).",
        tap.instance_id,
        tap.process_id,
        tap.sample_rate
            .map(|rate| format!("{rate:.0} Hz"))
            .unwrap_or_else(|| "not reported".into())
    );

    let command = TapCaptureCommand::new(cli.capture_seconds)?;
    request_capture(&tap, &command).context("failed to arm Ghost Tap")?;
    eprintln!(
        "[analyse-full] Armed {:.3}s capture. Play audio through the Tap now.",
        command.duration_seconds
    );

    let timeout = Duration::from_secs_f64(cli.capture_seconds + SIGNAL_WAIT_SECONDS);
    let artifact = wait_for_capture(&tap, command.request_id, timeout).with_context(|| {
        let tap_error = find_live_tap(tap.instance_id)
            .ok()
            .and_then(|status| status.last_error)
            .map(|error| format!(" Tap reported: {error}"))
            .unwrap_or_default();
        format!(
            "capture did not complete within {:.1}s; make sure audible signal is passing through the Tap.{tap_error}",
            timeout.as_secs_f64()
        )
    })?;

    eprintln!(
        "[analyse-full] Captured {} frames at {} Hz to {}. Analysing at maximum quality...",
        artifact.frames,
        artifact.sample_rate,
        artifact.wav_path.display()
    );

    let audio = read_audio(&artifact.wav_path)
        .with_context(|| format!("failed to decode {}", artifact.wav_path.display()))?;
    let mut analysis = analyze_audio(
        artifact.wav_path.display().to_string(),
        &audio,
        &AnalysisConfig::maximum(),
    )
    .context("maximum-quality audio analysis failed")?;
    analysis.capture.transport_bpm = artifact.transport.tempo_bpm;
    analysis.capture.transport_start_samples = artifact
        .transport
        .steady_sample_time
        .and_then(|samples| i64::try_from(samples).ok());

    let provenance = serde_json::to_value(&artifact)?;
    let display_spectrum = serde_json::to_value(&analysis.signal.spectrum.display_spectrum)?;
    let mut analysis = serde_json::to_value(&analysis)?;
    if let Some(spectrum) = analysis
        .pointer_mut("/signal/spectrum")
        .and_then(Value::as_object_mut)
    {
        spectrum.insert("display_spectrum".into(), display_spectrum);
    }
    print!("{}", render_report(&provenance, &analysis));
    Ok(())
}

fn select_tap(instance_id: Option<u32>) -> Result<TapStatus> {
    if let Some(instance_id) = instance_id {
        return find_live_tap(instance_id).with_context(|| {
            format!(
                "no active Ghost Tap instance {instance_id} was found; load Ghost Tap in FL Studio and make sure its mixer path is processing audio"
            )
        });
    }

    let mut taps = discover_live_taps().context("failed to discover active Ghost Tap instances")?;
    match taps.len() {
        0 => bail!(
            "no active Ghost Tap was found; load Ghost Tap in FL Studio and make sure its mixer path is processing audio"
        ),
        1 => Ok(taps.remove(0)),
        _ => {
            let ids = taps
                .iter()
                .map(|tap| tap.instance_id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            bail!("multiple Ghost Taps are live ({ids}); select one with --tap-instance <ID>")
        }
    }
}

fn parse_capture_length(value: &str) -> std::result::Result<f64, String> {
    let seconds = value
        .parse::<f64>()
        .map_err(|_| "length must be a number of seconds".to_owned())?;
    if seconds.is_finite() && (0.05..=20.0).contains(&seconds) {
        Ok(seconds)
    } else {
        Err("length must be between 0.05 and 20 seconds".into())
    }
}

fn render_report(provenance: &Value, analysis: &Value) -> String {
    let mut output = String::from(
        "# Ghost Tap Full Audio Analysis\n\n\
         Maximum-quality deterministic analysis of a lossless floating-point capture from FL Studio.\n\n\
         ## Capture provenance\n\n",
    );
    render_value(&mut output, provenance, 3);
    output.push_str("## Full analysis\n\n");
    render_value(&mut output, analysis, 3);
    output
}

fn render_value(output: &mut String, value: &Value, heading_level: usize) {
    match value {
        Value::Object(object) => render_object(output, object, heading_level),
        Value::Array(array) => render_array(output, array, heading_level),
        scalar => {
            let _ = writeln!(output, "{}\n", scalar_text(scalar));
        }
    }
}

fn render_object(
    output: &mut String,
    object: &serde_json::Map<String, Value>,
    heading_level: usize,
) {
    let scalars: Vec<_> = object
        .iter()
        .filter(|(_, value)| is_scalar(value))
        .collect();
    if !scalars.is_empty() {
        output.push_str("| Field | Value |\n|---|---|\n");
        for (key, value) in scalars {
            let _ = writeln!(
                output,
                "| {} | {} |",
                escape_table(&humanize(key)),
                escape_table(&scalar_text(value))
            );
        }
        output.push('\n');
    }

    for (key, value) in object.iter().filter(|(_, value)| !is_scalar(value)) {
        heading(output, heading_level, &humanize(key));
        render_value(output, value, heading_level + 1);
    }
}

fn render_array(output: &mut String, array: &[Value], heading_level: usize) {
    if array.is_empty() {
        output.push_str("_None._\n\n");
        return;
    }

    if array.iter().all(is_scalar) {
        output.push_str("| Index | Value |\n|---:|---:|\n");
        for (index, value) in array.iter().enumerate() {
            let _ = writeln!(
                output,
                "| {index} | {} |",
                escape_table(&scalar_text(value))
            );
        }
        output.push('\n');
        return;
    }

    if let Some(columns) = scalar_object_columns(array) {
        output.push('|');
        for column in &columns {
            let _ = write!(output, " {} |", escape_table(&humanize(column)));
        }
        output.push_str("\n|");
        for _ in &columns {
            output.push_str("---|");
        }
        output.push('\n');
        for item in array {
            let object = item.as_object().expect("validated object array");
            output.push('|');
            for column in &columns {
                let value = object.get(column).unwrap_or(&Value::Null);
                let _ = write!(output, " {} |", escape_table(&scalar_text(value)));
            }
            output.push('\n');
        }
        output.push('\n');
        return;
    }

    for (index, value) in array.iter().enumerate() {
        heading(output, heading_level, &format!("Item {index}"));
        render_value(output, value, heading_level + 1);
    }
}

fn scalar_object_columns(array: &[Value]) -> Option<Vec<String>> {
    let first = array.first()?.as_object()?;
    if first.values().any(|value| !is_scalar(value)) {
        return None;
    }
    let columns: Vec<String> = first.keys().cloned().collect();
    let same_shape = array.iter().all(|item| {
        item.as_object().is_some_and(|object| {
            object.len() == columns.len()
                && columns
                    .iter()
                    .all(|key| object.get(key).is_some_and(is_scalar))
        })
    });
    same_shape.then_some(columns)
}

fn is_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        _ => unreachable!("scalar_text called for a compound JSON value"),
    }
}

fn humanize(key: &str) -> String {
    let words = key.replace('_', " ");
    let mut characters = words.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

fn escape_table(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace("\r\n", "<br>")
        .replace(['\r', '\n'], "<br>")
}

fn heading(output: &mut String, level: usize, text: &str) {
    let level = level.clamp(1, 6);
    let _ = writeln!(output, "{} {text}\n", "#".repeat(level));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cli_defaults_to_ten_seconds_and_auto_selected_tap() {
        let cli = Cli::try_parse_from(["analyse-full"]).unwrap();
        assert_eq!(cli.capture_seconds, 10.0);
        assert_eq!(cli.tap_instance, None);
    }

    #[test]
    fn capture_length_is_bounded() {
        assert!(parse_capture_length("0.05").is_ok());
        assert!(parse_capture_length("20").is_ok());
        assert!(parse_capture_length("0").is_err());
        assert!(parse_capture_length("21").is_err());
        assert!(parse_capture_length("NaN").is_err());
    }

    #[test]
    fn report_renders_nested_values_and_full_arrays() {
        let report = render_report(
            &json!({"wav_path": "C:\\audio|tap.wav"}),
            &json!({
                "configuration": {"profile": "maximum"},
                "signal": {
                    "frame_centroid_hz": [100.0, 200.0],
                    "flags": [{"code": "test", "message": "a|b"}]
                }
            }),
        );
        assert!(report.starts_with("# Ghost Tap Full Audio Analysis"));
        assert!(report.contains("C:\\\\audio\\|tap.wav"));
        assert!(report.contains("| 0 | 100.0 |"));
        assert!(report.contains("| 1 | 200.0 |"));
        assert!(report.contains("| test | a\\|b |"));
        assert!(report.contains("| Profile | maximum |"));
    }
}
