use std::{
    cmp::Ordering,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use ghost_codex::{ToolDefinition, ToolError, ToolRegistry};
use serde_json::{json, Value};

use crate::adapter::{AdapterError, GopherNativeAdapter, NativeToolResult};
use crate::codex_tools::{FlAgentToolPolicy, FlPluginWriteScope};

const DISPLAY_TUNE_STEPS: usize = 14;
const DISPLAY_PROBE_POINTS: [f64; 9] = [0.0, 0.0625, 0.125, 0.25, 0.5, 0.75, 0.875, 0.9375, 1.0];
const PARAMETER_SETTLE_TIMEOUT: Duration = Duration::from_millis(300);
const PARAMETER_SETTLE_FLOOR: Duration = Duration::from_millis(45);
const PARAMETER_POLL_INTERVAL: Duration = Duration::from_millis(12);
const NORMALIZED_EPSILON: f64 = 0.0025;

/// Product-facing FL tool registration.
///
/// The base registrar owns the proven tempo/transport/parameter primitives. Product processor
/// workflows replace the operations that need stricter runtime semantics:
/// - compact context and slot safety use direct plugin-slot probes;
/// - display-domain continuous tuning waits for FL/plugin value-string propagation before sampling;
/// - normalized writes are restricted to explicit boolean/discrete controls so the agent cannot
///   fall back to arbitrary 0..1 guesses when a continuous mapping is unclear.
pub fn register_codex_tools(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    policy: FlAgentToolPolicy,
) -> Result<(), ToolError> {
    let processor_scope = policy.plugin_write_scope.clone();
    crate::codex_tools::register_codex_tools(
        registry,
        Arc::clone(&adapter),
        policy,
    )?;

    if let Some(scope) = processor_scope {
        replace_processor_context_tool(registry, Arc::clone(&adapter), scope.clone())?;
        replace_add_effect_tool(registry, Arc::clone(&adapter), scope.clone())?;
        replace_display_value_tool(registry, Arc::clone(&adapter), scope.clone())?;
        replace_normalized_parameter_tool(registry, adapter, scope)?;
    }
    Ok(())
}

fn replace_processor_context_tool(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    let description = format!(
        "Read a compact live view of mixer track {} across effect slots {}..={}. Ghost probes each slot directly through FL's plugin target resolver; it does not parse the large Gopher session-context text. This tool does not modify the project.",
        scope.target_track, scope.slot_start, scope.slot_end
    );
    registry.replace(
        ToolDefinition {
            name: "fl_get_target_track_context".into(),
            description,
            input_schema: empty_schema(),
        },
        move |_| scoped_track_context(&adapter, &scope),
    )
}

fn replace_add_effect_tool(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    let allowed = scope.allowed_plugins.join(", ");
    let description = format!(
        "Insert one effect on FL Studio mixer track {}. Writes are restricted to slots {}..={} and these exact plugin names: {}. Ghost probes the requested slot immediately before insertion and refuses to overwrite any effect that already resolves there.",
        scope.target_track, scope.slot_start, scope.slot_end, allowed
    );
    let input_schema = json!({
        "type": "object",
        "properties": {
            "plugin": {"type": "string", "description": format!("One of: {allowed}")},
            "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end}
        },
        "required": ["plugin", "slot_number"],
        "additionalProperties": false
    });

    registry.replace(
        ToolDefinition {
            name: "fl_add_effect".into(),
            description,
            input_schema,
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&scope, slot)?;
            let requested = required_str(&arguments, "plugin")?;
            let plugin = canonical_plugin(&scope, requested).ok_or_else(|| {
                ToolError(format!(
                    "plugin `{requested}` is outside the allowed processor scope"
                ))
            })?;

            match probe_effect_slot(&adapter, &scope, slot)? {
                EffectSlotProbe::Empty => {}
                EffectSlotProbe::Occupied { name } => {
                    return Err(ToolError(format!(
                        "refusing to insert `{plugin}` into mixer track {}, slot {slot}: `{name}` already resolves there",
                        scope.target_track
                    )));
                }
            }

            let result = adapter
                .add_effect_verified(plugin, &scope.target_track, slot)
                .map_err(|error| ToolError(error.to_string()))?;
            serde_json::to_value(result).map_err(|error| ToolError(error.to_string()))
        },
    )
}

fn replace_display_value_tool(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    let description = format!(
        "Set a continuous parameter on mixer track {} to a human display-domain target such as -5 dB, 350 Hz, 0.9, 3:1, 18 ms, or 90%. Ghost calibrates the plugin's normalized 0..1 mapping with temporary UNJOURNALED probes, waits for FL/plugin display text to settle after every probe, restores the original value on failure, and commits exactly one journaled normalized write only after the target mapping has been proven. FL only exposes value strings for plugins that support getParamValueString; unsupported mappings fail closed.",
        scope.target_track
    );
    let input_schema = json!({
        "type": "object",
        "properties": {
            "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end},
            "param_identifier": {"type": "string", "minLength": 1},
            "target_display": {"type": "string", "minLength": 1, "description": "Human target including units when applicable, e.g. -5 dB, 350 Hz, 18 ms, 3:1, 90%"}
        },
        "required": ["slot_number", "param_identifier", "target_display"],
        "additionalProperties": false
    });

    registry.replace(
        ToolDefinition {
            name: "fl_set_plugin_parameter_display_value".into(),
            description,
            input_schema,
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&scope, slot)?;
            let parameter = required_str(&arguments, "param_identifier")?;
            if parameter.trim().chars().all(|ch| ch.is_ascii_digit()) {
                return Err(ToolError(
                    "display-domain tuning requires the exact published parameter NAME, not a numeric index; use fl_find_plugin_parameters first".into(),
                ));
            }
            let target_display = required_str(&arguments, "target_display")?;
            tune_parameter_display(&adapter, &scope, slot, parameter, target_display)
        },
    )
}

fn replace_normalized_parameter_tool(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    let description = format!(
        "Set an explicit boolean/discrete plugin control on mixer track {}. This product-facing normalized writer is intentionally restricted to exact published boolean-like parameter names (for example `Band 1 Used`, `Band 1 Enabled`, `Bypass`) and values exactly 0 or 1. Continuous parameters such as frequency, gain, Q, threshold, ratio, attack, release, mix, range, or output level MUST use fl_set_plugin_parameter_display_value instead. Numeric parameter indices are rejected.",
        scope.target_track
    );
    let input_schema = json!({
        "type": "object",
        "properties": {
            "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end},
            "param_identifier": {"type": "string", "minLength": 1},
            "value": {"type": "number", "enum": [0, 1]}
        },
        "required": ["slot_number", "param_identifier", "value"],
        "additionalProperties": false
    });

    registry.replace(
        ToolDefinition {
            name: "fl_set_plugin_parameter_value".into(),
            description,
            input_schema,
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&scope, slot)?;
            let parameter = required_str(&arguments, "param_identifier")?;
            if parameter.trim().chars().all(|ch| ch.is_ascii_digit()) {
                return Err(ToolError(
                    "normalized product writes require an exact published parameter NAME; numeric indices are rejected".into(),
                ));
            }
            if !is_safe_binary_parameter(parameter) {
                return Err(ToolError(format!(
                    "`{parameter}` is not a recognized boolean/discrete control; use fl_set_plugin_parameter_display_value for continuous controls"
                )));
            }
            let value = arguments
                .get("value")
                .and_then(Value::as_f64)
                .ok_or_else(|| ToolError("missing or invalid number `value`".into()))?;
            if value != 0.0 && value != 1.0 {
                return Err(ToolError(
                    "product-facing normalized writes are restricted to exact boolean values 0 or 1".into(),
                ));
            }
            let mutation = adapter
                .set_plugin_parameter_verified(&scope.target_track, parameter, value, slot)
                .map_err(|error| ToolError(error.to_string()))?;
            serde_json::to_value(mutation).map_err(|error| ToolError(error.to_string()))
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EffectSlotProbe {
    Empty,
    Occupied { name: String },
}

#[derive(Debug, Clone)]
struct ParameterReading {
    normalized: f64,
    display: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayUnit {
    Unitless,
    Db,
    Hertz,
    Milliseconds,
    Percent,
    Ratio,
}

impl DisplayUnit {
    fn label(self) -> &'static str {
        match self {
            Self::Unitless => "unitless",
            Self::Db => "dB",
            Self::Hertz => "Hz",
            Self::Milliseconds => "ms",
            Self::Percent => "%",
            Self::Ratio => "ratio",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CanonicalDisplay {
    value: f64,
    unit: DisplayUnit,
}

#[derive(Debug, Clone)]
struct DisplaySample {
    normalized: f64,
    canonical: CanonicalDisplay,
    display: String,
}

fn scoped_track_context(
    adapter: &GopherNativeAdapter,
    scope: &FlPluginWriteScope,
) -> Result<Value, ToolError> {
    validate_scope(scope)?;
    let tempo = adapter
        .get_tempo()
        .map_err(|error| ToolError(error.to_string()))?;
    let mut slot_states = Vec::new();
    let mut effects = Vec::new();

    for slot in scope.slot_start..=scope.slot_end {
        match probe_effect_slot(adapter, scope, slot)? {
            EffectSlotProbe::Empty => {
                slot_states.push(json!({"slot": slot, "occupied": false}));
            }
            EffectSlotProbe::Occupied { name } => {
                slot_states.push(json!({
                    "slot": slot,
                    "occupied": true,
                    "name": name
                }));
                effects.push(json!({"slot": slot, "name": name}));
            }
        }
    }

    Ok(json!({
        "contextSource": "direct_plugin_slot_probes",
        "tempoBpm": tempo,
        "targetMixerTrack": {
            "target": scope.target_track,
            "effect_plugins": effects
        },
        "effectSlots": slot_states,
        "writeScope": {
            "slotStart": scope.slot_start,
            "slotEnd": scope.slot_end,
            "allowedPlugins": scope.allowed_plugins
        }
    }))
}

fn probe_effect_slot(
    adapter: &GopherNativeAdapter,
    scope: &FlPluginWriteScope,
    slot: u32,
) -> Result<EffectSlotProbe, ToolError> {
    ensure_slot(scope, slot)?;
    match adapter.plugin_parameter_list(&scope.target_track, slot) {
        Ok(result) => {
            let manifest = native_content_text(&result).join("\n");
            let name = resolve_plugin_name(&manifest, &scope.allowed_plugins)
                .unwrap_or_else(|| "existing effect".to_owned());
            Ok(EffectSlotProbe::Occupied { name })
        }
        Err(AdapterError::NativeTool(message)) if is_empty_plugin_target_error(&message) => {
            Ok(EffectSlotProbe::Empty)
        }
        Err(error) => Err(ToolError(error.to_string())),
    }
}

fn tune_parameter_display(
    adapter: &GopherNativeAdapter,
    scope: &FlPluginWriteScope,
    slot: u32,
    parameter: &str,
    target_display: &str,
) -> Result<Value, ToolError> {
    let target = parse_display_value(target_display).ok_or_else(|| {
        ToolError(format!(
            "target_display `{target_display}` is not a supported numeric display value; include units when applicable"
        ))
    })?;
    let before = parameter_reading(adapter, &scope.target_track, parameter, slot)?;
    let tolerance = display_tolerance(target);
    let mut samples = Vec::new();

    for normalized in DISPLAY_PROBE_POINTS {
        match set_and_settle_display(
            adapter,
            &scope.target_track,
            parameter,
            normalized,
            slot,
        ) {
            Ok(reading) => {
                let Some(display) = reading.display else {
                    continue;
                };
                let Some(canonical) = parse_display_value(&display) else {
                    continue;
                };
                if canonical.unit == target.unit {
                    samples.push(DisplaySample {
                        normalized: reading.normalized,
                        canonical,
                        display,
                    });
                }
            }
            Err(error) => {
                let _ = restore_parameter(adapter, &scope.target_track, parameter, before.normalized, slot);
                return Err(error);
            }
        }
    }

    if samples.is_empty() {
        let _ = restore_parameter(adapter, &scope.target_track, parameter, before.normalized, slot);
        return Err(ToolError(format!(
            "parameter `{parameter}` did not expose a stable numeric {} display mapping through FL getParamValueString; original value restored",
            target.unit.label()
        )));
    }

    let mut best = samples
        .iter()
        .min_by(|left, right| {
            display_error(left.canonical, target)
                .partial_cmp(&display_error(right.canonical, target))
                .unwrap_or(Ordering::Equal)
        })
        .cloned()
        .expect("samples is non-empty");

    let mut bracket = samples.windows(2).find_map(|pair| {
        brackets(pair[0].canonical.value, pair[1].canonical.value, target.value)
            .then(|| (pair[0].clone(), pair[1].clone()))
    });

    if display_error(best.canonical, target) > tolerance {
        let Some((mut low, mut high)) = bracket.take() else {
            let sampled = samples
                .iter()
                .map(|sample| format!("{:.4} -> {}", sample.normalized, sample.display))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = restore_parameter(adapter, &scope.target_track, parameter, before.normalized, slot);
            return Err(ToolError(format!(
                "target `{target_display}` was outside the stable numeric display range Ghost could bracket for `{parameter}` ({sampled}); original value restored"
            )));
        };

        for _ in 0..DISPLAY_TUNE_STEPS {
            let normalized = (low.normalized + high.normalized) * 0.5;
            let reading = set_and_settle_display(
                adapter,
                &scope.target_track,
                parameter,
                normalized,
                slot,
            )?;
            let Some(display) = reading.display else {
                continue;
            };
            let Some(canonical) = parse_display_value(&display) else {
                continue;
            };
            if canonical.unit != target.unit {
                continue;
            }
            let sample = DisplaySample {
                normalized: reading.normalized,
                canonical,
                display,
            };
            if display_error(sample.canonical, target) < display_error(best.canonical, target) {
                best = sample.clone();
            }
            if display_error(best.canonical, target) <= tolerance {
                break;
            }
            if brackets(low.canonical.value, sample.canonical.value, target.value) {
                high = sample;
            } else {
                low = sample;
            }
        }
    }

    if display_error(best.canonical, target) > tolerance {
        let _ = restore_parameter(adapter, &scope.target_track, parameter, before.normalized, slot);
        return Err(ToolError(format!(
            "could not converge `{parameter}` to `{target_display}` within tolerance after settled display probing; best stable mapping was {:.4} -> {}; original value restored",
            best.normalized, best.display
        )));
    }

    // Prove the exact selected normalized value with the settled display path once more before
    // creating a durable mutation record. Probe/calibration writes above are intentionally raw and
    // therefore do not pollute the adapter mutation journal.
    let proven = set_and_settle_display(
        adapter,
        &scope.target_track,
        parameter,
        best.normalized,
        slot,
    )?;
    let proven_canonical = proven.display.as_deref().and_then(parse_display_value);
    if !proven_canonical.is_some_and(|value| {
        value.unit == target.unit && display_error(value, target) <= tolerance
    }) {
        let _ = restore_parameter(adapter, &scope.target_track, parameter, before.normalized, slot);
        return Err(ToolError(format!(
            "the selected normalized mapping for `{parameter}` did not remain stable at `{target_display}`; original value restored"
        )));
    }

    restore_parameter(adapter, &scope.target_track, parameter, before.normalized, slot)?;

    // Commit exactly one journaled write after the display-domain mapping has been proven.
    let mutation = adapter
        .set_plugin_parameter_verified(
            &scope.target_track,
            parameter,
            best.normalized,
            slot,
        )
        .map_err(|error| ToolError(error.to_string()))?;

    Ok(json!({
        "parameter": parameter,
        "targetDisplay": target_display,
        "tolerance": {"value": tolerance, "unit": target.unit.label()},
        "before": reading_json(&before),
        "after": {
            "normalizedValue": best.normalized,
            "displayValue": best.display,
            "canonicalDisplay": {
                "value": best.canonical.value,
                "unit": best.canonical.unit.label()
            }
        },
        "mutation": mutation,
        "displayMappingVerified": true,
        "probeWritesJournaled": false
    }))
}

fn set_and_settle_display(
    adapter: &GopherNativeAdapter,
    target: &str,
    parameter: &str,
    value: f64,
    slot: u32,
) -> Result<ParameterReading, ToolError> {
    raw_set_parameter(adapter, target, parameter, value, slot)?;
    wait_for_settled_display(adapter, target, parameter, value, slot)
}

fn wait_for_settled_display(
    adapter: &GopherNativeAdapter,
    target: &str,
    parameter: &str,
    requested_normalized: f64,
    slot: u32,
) -> Result<ParameterReading, ToolError> {
    let start = Instant::now();
    let mut normalized_matched_at: Option<Instant> = None;
    let mut last_display: Option<String> = None;
    let mut stable_display_reads = 0_u8;
    let mut last_reading: Option<ParameterReading> = None;

    loop {
        let reading = parameter_reading(adapter, target, parameter, slot)?;
        let normalized_matches =
            (reading.normalized - requested_normalized).abs() <= NORMALIZED_EPSILON;

        if normalized_matches {
            let matched_at = normalized_matched_at.get_or_insert_with(Instant::now);
            if matched_at.elapsed() >= PARAMETER_SETTLE_FLOOR {
                if let Some(display) = reading.display.as_ref() {
                    if last_display.as_deref() == Some(display.as_str()) {
                        stable_display_reads = stable_display_reads.saturating_add(1);
                    } else {
                        last_display = Some(display.clone());
                        stable_display_reads = 1;
                    }
                    if stable_display_reads >= 2 {
                        return Ok(reading);
                    }
                }
            }
        } else {
            normalized_matched_at = None;
            last_display = None;
            stable_display_reads = 0;
        }

        last_reading = Some(reading);
        if start.elapsed() >= PARAMETER_SETTLE_TIMEOUT {
            let detail = last_reading
                .as_ref()
                .map(|reading| {
                    format!(
                        "last normalized={:.4}, display={}",
                        reading.normalized,
                        reading.display.as_deref().unwrap_or("<none>")
                    )
                })
                .unwrap_or_else(|| "no readable value".into());
            return Err(ToolError(format!(
                "parameter `{parameter}` did not settle after normalized write {:.4} within {} ms ({detail})",
                requested_normalized,
                PARAMETER_SETTLE_TIMEOUT.as_millis()
            )));
        }
        thread::sleep(PARAMETER_POLL_INTERVAL);
    }
}

fn restore_parameter(
    adapter: &GopherNativeAdapter,
    target: &str,
    parameter: &str,
    value: f64,
    slot: u32,
) -> Result<(), ToolError> {
    raw_set_parameter(adapter, target, parameter, value, slot)?;
    wait_for_normalized(adapter, target, parameter, value, slot)
}

fn wait_for_normalized(
    adapter: &GopherNativeAdapter,
    target: &str,
    parameter: &str,
    requested_normalized: f64,
    slot: u32,
) -> Result<(), ToolError> {
    let start = Instant::now();
    loop {
        let reading = parameter_reading(adapter, target, parameter, slot)?;
        if (reading.normalized - requested_normalized).abs() <= NORMALIZED_EPSILON {
            return Ok(());
        }
        if start.elapsed() >= PARAMETER_SETTLE_TIMEOUT {
            return Err(ToolError(format!(
                "could not restore `{parameter}` to normalized value {:.4} within {} ms; last readback was {:.4}",
                requested_normalized,
                PARAMETER_SETTLE_TIMEOUT.as_millis(),
                reading.normalized
            )));
        }
        thread::sleep(PARAMETER_POLL_INTERVAL);
    }
}

fn raw_set_parameter(
    adapter: &GopherNativeAdapter,
    target: &str,
    parameter: &str,
    value: f64,
    slot: u32,
) -> Result<(), ToolError> {
    adapter
        .call_native(
            "set_plugin_parameter_value",
            json!({
                "target": target,
                "param_identifier": parameter,
                "value": value,
                "slot_number": slot
            }),
        )
        .map(|_| ())
        .map_err(|error| ToolError(error.to_string()))
}

fn parameter_reading(
    adapter: &GopherNativeAdapter,
    target: &str,
    parameter: &str,
    slot: u32,
) -> Result<ParameterReading, ToolError> {
    let result = adapter
        .call_native(
            "get_plugin_parameter_value",
            json!({
                "target": target,
                "param_identifier": parameter,
                "slot_number": slot
            }),
        )
        .map_err(|error| ToolError(error.to_string()))?;
    let text = native_content_text(&result).join("\n");
    let normalized = marker_number(&text, "Normalized Value:")
        .or_else(|| first_number(&text))
        .ok_or_else(|| {
            ToolError(format!(
                "could not parse normalized value for parameter `{parameter}` from native response"
            ))
        })?;
    Ok(ParameterReading {
        normalized,
        display: string_value(&text),
    })
}

fn reading_json(reading: &ParameterReading) -> Value {
    let canonical = reading.display.as_deref().and_then(parse_display_value);
    json!({
        "normalizedValue": reading.normalized,
        "displayValue": reading.display,
        "canonicalDisplay": canonical.map(|value| json!({
            "value": value.value,
            "unit": value.unit.label()
        }))
    })
}

fn parse_display_value(text: &str) -> Option<CanonicalDisplay> {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    let mut value = first_number(trimmed)?;
    let unit = if lower.contains("khz") {
        value *= 1000.0;
        DisplayUnit::Hertz
    } else if lower.contains("hz") {
        DisplayUnit::Hertz
    } else if lower.contains("ms") {
        DisplayUnit::Milliseconds
    } else if lower.contains("db") {
        DisplayUnit::Db
    } else if lower.contains('%') {
        DisplayUnit::Percent
    } else if lower.contains(":1") {
        DisplayUnit::Ratio
    } else if lower
        .split_whitespace()
        .skip(1)
        .any(|unit| matches!(unit, "s" | "sec" | "secs" | "second" | "seconds"))
    {
        value *= 1000.0;
        DisplayUnit::Milliseconds
    } else {
        DisplayUnit::Unitless
    };
    value
        .is_finite()
        .then_some(CanonicalDisplay { value, unit })
}

fn display_tolerance(target: CanonicalDisplay) -> f64 {
    match target.unit {
        DisplayUnit::Db => 0.2,
        DisplayUnit::Hertz => (target.value.abs() * 0.0075).max(1.5),
        DisplayUnit::Milliseconds => (target.value.abs() * 0.03).max(0.75),
        DisplayUnit::Percent => 0.5,
        DisplayUnit::Ratio => (target.value.abs() * 0.03).max(0.08),
        DisplayUnit::Unitless => (target.value.abs() * 0.02).max(0.02),
    }
}

fn display_error(value: CanonicalDisplay, target: CanonicalDisplay) -> f64 {
    if value.unit == target.unit {
        (value.value - target.value).abs()
    } else {
        f64::INFINITY
    }
}

fn brackets(left: f64, right: f64, target: f64) -> bool {
    let min = left.min(right);
    let max = left.max(right);
    target >= min && target <= max
}

fn is_safe_binary_parameter(parameter: &str) -> bool {
    let lower = parameter.trim().to_ascii_lowercase();
    lower == "bypass"
        || lower.ends_with(" used")
        || lower.ends_with(" enabled")
        || lower.ends_with(" auto")
        || lower.contains("auto release")
        || lower.ends_with(" external side chain")
        || lower.ends_with(" side chain filtering")
        || lower == "receive midi"
        || lower == "output invert phase"
}

fn is_empty_plugin_target_error(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("could not resolve plugin target")
}

fn resolve_plugin_name(manifest: &str, allowed_plugins: &[String]) -> Option<String> {
    let lower = manifest.to_ascii_lowercase();
    allowed_plugins
        .iter()
        .find(|plugin| lower.contains(&plugin.to_ascii_lowercase()))
        .cloned()
}

fn native_content_text(result: &NativeToolResult) -> Vec<String> {
    if !result.content_text.is_empty() {
        return result.content_text.clone();
    }
    result
        .raw
        .pointer("/result/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn marker_number(text: &str, marker: &str) -> Option<f64> {
    let (_, tail) = text.split_once(marker)?;
    first_number(tail)
}

fn string_value(text: &str) -> Option<String> {
    let marker = "String Value: '";
    let (_, tail) = text.split_once(marker)?;
    let end = tail.find('\'')?;
    Some(tail[..end].to_owned())
}

fn first_number(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len() {
        let first = bytes[start] as char;
        if !(first.is_ascii_digit() || matches!(first, '+' | '-' | '.')) {
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '+' | '-') {
                end += 1;
            } else {
                break;
            }
        }
        if let Ok(value) = text[start..end].parse::<f64>() {
            return value.is_finite().then_some(value);
        }
    }
    None
}

fn validate_scope(scope: &FlPluginWriteScope) -> Result<(), ToolError> {
    if scope.slot_start == 0 || scope.slot_end < scope.slot_start || scope.slot_end > 10 {
        return Err(ToolError(
            "processor write scope must use FL mixer slots 1..=10".into(),
        ));
    }
    if scope.allowed_plugins.is_empty() {
        return Err(ToolError(
            "processor write scope must allow at least one plugin".into(),
        ));
    }
    Ok(())
}

fn ensure_slot(scope: &FlPluginWriteScope, slot: u32) -> Result<(), ToolError> {
    validate_scope(scope)?;
    if slot >= scope.slot_start && slot <= scope.slot_end {
        Ok(())
    } else {
        Err(ToolError(format!(
            "slot {slot} is outside the allowed range {}..={} for mixer track {}",
            scope.slot_start, scope.slot_end, scope.target_track
        )))
    }
}

fn canonical_plugin<'a>(scope: &'a FlPluginWriteScope, requested: &str) -> Option<&'a str> {
    scope
        .allowed_plugins
        .iter()
        .find(|plugin| plugin.eq_ignore_ascii_case(requested))
        .map(String::as_str)
}

fn required_u32(arguments: &Value, key: &str) -> Result<u32, ToolError> {
    let value = arguments
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError(format!("missing or invalid integer `{key}`")))?;
    u32::try_from(value).map_err(|_| ToolError(format!("`{key}` is out of range")))
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolError(format!("missing or invalid string `{key}`")))
}

fn empty_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_plugin_target_is_an_empty_slot_signal() {
        assert!(is_empty_plugin_target_error(
            "Error: Could not resolve plugin target '1' (Slot: 1)."
        ));
        assert!(!is_empty_plugin_target_error("Error: something else"));
    }

    #[test]
    fn resolves_known_plugin_name_from_parameter_manifest() {
        let allowed = vec!["Pro-Q 4".to_owned(), "Pro-C 3".to_owned()];
        assert_eq!(
            resolve_plugin_name(
                "Plugin Pro-C 3 parameter list\nIndex 1: Threshold",
                &allowed
            ),
            Some("Pro-C 3".to_owned())
        );
    }

    #[test]
    fn canonical_plugin_is_case_insensitive_and_scope_is_bounded() {
        let scope = FlPluginWriteScope::new("1", 1, 4, ["Pro-Q 4", "Pro-C 3"]);
        assert_eq!(canonical_plugin(&scope, "pro-q 4"), Some("Pro-Q 4"));
        assert!(ensure_slot(&scope, 1).is_ok());
        assert!(ensure_slot(&scope, 5).is_err());
    }

    #[test]
    fn display_parser_handles_common_audio_units() {
        assert_eq!(parse_display_value("350 Hz").unwrap().unit, DisplayUnit::Hertz);
        assert_eq!(parse_display_value("0.35 kHz").unwrap().value, 350.0);
        assert_eq!(parse_display_value("-5.00 dB").unwrap().value, -5.0);
        assert_eq!(parse_display_value("18 ms").unwrap().unit, DisplayUnit::Milliseconds);
        assert_eq!(parse_display_value("3:1").unwrap().unit, DisplayUnit::Ratio);
        assert_eq!(parse_display_value("90%").unwrap().unit, DisplayUnit::Percent);
    }

    #[test]
    fn normalized_product_writer_is_boolean_only() {
        assert!(is_safe_binary_parameter("Band 1 Used"));
        assert!(is_safe_binary_parameter("Band 2 Enabled"));
        assert!(is_safe_binary_parameter("Bypass"));
        assert!(!is_safe_binary_parameter("Band 1 Frequency"));
        assert!(!is_safe_binary_parameter("Threshold"));
        assert!(!is_safe_binary_parameter("Ratio"));
    }
}
