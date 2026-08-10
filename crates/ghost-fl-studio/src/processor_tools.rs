use std::cmp::Ordering;
use std::sync::Arc;

use ghost_codex::{ToolDefinition, ToolError, ToolRegistry};
use serde_json::{json, Value};

use crate::adapter::{AdapterError, GopherNativeAdapter, NativeToolResult};
use crate::codex_tools::{FlAgentToolPolicy, FlPluginWriteScope};

const MAX_PARAMETER_SEARCH_MATCHES: usize = 64;
const DISPLAY_TUNE_STEPS: usize = 18;
const DISPLAY_PROBE_POINTS: [f64; 9] = [0.0, 0.0625, 0.125, 0.25, 0.5, 0.75, 0.875, 0.9375, 1.0];

/// Public registration boundary used by Ghost applications.
///
/// Non-processor policies are delegated unchanged to the original Codex tool registrar. Processor
/// policies deliberately bypass its session-context-dependent processor tools and install the
/// purpose-built slot-probe implementation below instead. FL's Gopher `get_session_context` result
/// is a large human-oriented payload and is not guaranteed to be JSON, so mixer write safety must
/// not depend on parsing that response.
pub fn register_codex_tools(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    policy: FlAgentToolPolicy,
) -> Result<(), ToolError> {
    let processor_scope = policy.plugin_write_scope.clone();
    let mut base_policy = policy;
    base_policy.plugin_write_scope = None;

    crate::codex_tools::register_codex_tools(
        registry,
        Arc::clone(&adapter),
        base_policy,
    )?;

    if let Some(scope) = processor_scope {
        register_processor_tools(registry, adapter, scope)?;
    }
    Ok(())
}

fn register_processor_tools(
    registry: &mut ToolRegistry,
    adapter: Arc<GopherNativeAdapter>,
    scope: FlPluginWriteScope,
) -> Result<(), ToolError> {
    validate_scope(&scope)?;

    let context_scope = scope.clone();
    let context_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_get_target_track_context".into(),
            description: format!(
                "Read a compact live view of mixer track {} across writable effect slots {}..={}. Ghost probes each slot directly through FL's plugin target resolver instead of parsing the large Gopher session-context text. This tool does not modify the project.",
                scope.target_track, scope.slot_start, scope.slot_end
            ),
            input_schema: empty_schema(),
        },
        move |_| scoped_track_context(&context_adapter, &context_scope),
    )?;

    let target = scope.target_track.clone();
    let allowed = scope.allowed_plugins.join(", ");
    let add_scope = scope.clone();
    let add_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_add_effect".into(),
            description: format!(
                "Insert one effect on FL Studio mixer track {}. Writes are restricted to slots {}..={} and these exact plugin names: {}. Ghost directly probes the requested plugin slot immediately before insertion and refuses to overwrite any effect that already resolves there.",
                target, scope.slot_start, scope.slot_end, allowed
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "plugin": {"type": "string", "description": format!("One of: {allowed}")},
                    "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end}
                },
                "required": ["plugin", "slot_number"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&add_scope, slot)?;
            let requested = required_str(&arguments, "plugin")?;
            let plugin = add_scope
                .canonical_plugin(requested)
                .ok_or_else(|| ToolError(format!(
                    "plugin `{requested}` is outside the allowed processor scope"
                )))?;

            match probe_effect_slot(&add_adapter, &add_scope, slot)? {
                EffectSlotProbe::Empty => {}
                EffectSlotProbe::Occupied { name } => {
                    return Err(ToolError(format!(
                        "refusing to insert `{plugin}` into mixer track {}, slot {slot}: `{name}` already resolves there",
                        add_scope.target_track
                    )));
                }
            }

            let result = add_adapter
                .add_effect_verified(plugin, &add_scope.target_track, slot)
                .map_err(|error| ToolError(error.to_string()))?;
            serde_json::to_value(result).map_err(|error| ToolError(error.to_string()))
        },
    )?;

    let search_scope = scope.clone();
    let search_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_find_plugin_parameters".into(),
            description: format!(
                "Search the published parameter manifest of the plugin on mixer track {}, restricted to processor slots {}..={}. Space-separated terms are ORed, so `threshold ratio attack release` finds any of those controls. MIDI CC mappings are hidden unless `midi` is explicitly requested. Returns at most {} structured matches.",
                scope.target_track,
                scope.slot_start,
                scope.slot_end,
                MAX_PARAMETER_SEARCH_MATCHES
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end},
                    "query": {"type": "string", "minLength": 1}
                },
                "required": ["slot_number", "query"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&search_scope, slot)?;
            let query = required_str(&arguments, "query")?;
            let manifest = plugin_manifest(&search_adapter, &search_scope, slot)?;
            Ok(search_parameter_manifest(&manifest, query))
        },
    )?;

    let read_scope = scope.clone();
    let read_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_get_plugin_parameter_value".into(),
            description: "Read one exact published plugin parameter. Returns both the normalized 0..1 value and the plugin's native human display string when FL exposes it (for example -18.00 dB, 20.00 ms, 3.00:1, or 350 Hz). Discover the exact identifier with fl_find_plugin_parameters.".into(),
            input_schema: parameter_schema(&scope, false),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&read_scope, slot)?;
            let parameter = required_str(&arguments, "param_identifier")?;
            let reading = parameter_reading(
                &read_adapter,
                &read_scope.target_track,
                parameter,
                slot,
            )?;
            Ok(reading_json(&reading))
        },
    )?;

    let tune_scope = scope.clone();
    let tune_adapter = Arc::clone(&adapter);
    registry.register(
        ToolDefinition {
            name: "fl_set_plugin_parameter_display_value".into(),
            description: "Preferred tool for continuous numeric controls. Set an exact published parameter to a human display-domain target such as `-24 dB`, `3:1`, `15 ms`, `120 ms`, `350 Hz`, `1.2`, or `35%`. Ghost probes the stopped plugin's normalized mapping, restores the original setting after calibration, then performs one journaled native write/readback to the chosen display target. It restores on calibration failure. Use this instead of guessing tiny normalized deltas.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "slot_number": {"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end},
                    "param_identifier": {"type": "string"},
                    "target_display": {"type": "string", "description": "Human target including units when applicable, e.g. -24 dB, 3:1, 20 ms, 350 Hz"}
                },
                "required": ["slot_number", "param_identifier", "target_display"],
                "additionalProperties": false
            }),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&tune_scope, slot)?;
            let parameter = required_str(&arguments, "param_identifier")?;
            let target_display = required_str(&arguments, "target_display")?;
            tune_parameter_display(
                &tune_adapter,
                &tune_scope,
                slot,
                parameter,
                target_display,
            )
        },
    )?;

    let write_scope = scope;
    let write_adapter = adapter;
    registry.register(
        ToolDefinition {
            name: "fl_set_plugin_parameter_value".into(),
            description: "Low-level normalized 0..1 parameter write with native readback. Prefer fl_set_plugin_parameter_display_value for continuous numeric controls. Use this normalized tool only when a discrete/boolean/enum mapping is already semantically clear; do not use it for token 0.02-0.03 gestures whose audible meaning is unknown.".into(),
            input_schema: parameter_schema(&write_scope, true),
        },
        move |arguments| {
            let slot = required_u32(&arguments, "slot_number")?;
            ensure_slot(&write_scope, slot)?;
            let parameter = required_str(&arguments, "param_identifier")?;
            let value = arguments
                .get("value")
                .and_then(Value::as_f64)
                .ok_or_else(|| ToolError("missing or invalid number `value`".into()))?;
            if !(0.0..=1.0).contains(&value) {
                return Err(ToolError("`value` must be normalized to 0..=1".into()));
            }
            let mutation = write_adapter
                .set_plugin_parameter_verified(
                    &write_scope.target_track,
                    parameter,
                    value,
                    slot,
                )
                .map_err(|error| ToolError(error.to_string()))?;
            let after = parameter_reading(
                &write_adapter,
                &write_scope.target_track,
                parameter,
                slot,
            )?;
            Ok(json!({
                "mutation": mutation,
                "after": reading_json(&after)
            }))
        },
    )?;

    Ok(())
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum EffectSlotProbe {
    Empty,
    Occupied { name: String },
}

fn scoped_track_context(
    adapter: &GopherNativeAdapter,
    scope: &FlPluginWriteScope,
) -> Result<Value, ToolError> {
    let tempo = adapter
        .get_tempo()
        .map_err(|error| ToolError(error.to_string()))?;
    let mut slots = Vec::new();
    let mut effects = Vec::new();

    for slot in scope.slot_start..=scope.slot_end {
        match probe_effect_slot(adapter, scope, slot)? {
            EffectSlotProbe::Empty => {
                slots.push(json!({"slot": slot, "occupied": false}));
            }
            EffectSlotProbe::Occupied { name } => {
                slots.push(json!({"slot": slot, "occupied": true, "name": name}));
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
        "effectSlots": slots,
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

fn plugin_manifest(
    adapter: &GopherNativeAdapter,
    scope: &FlPluginWriteScope,
    slot: u32,
) -> Result<String, ToolError> {
    ensure_slot(scope, slot)?;
    match adapter.plugin_parameter_list(&scope.target_track, slot) {
        Ok(result) => Ok(native_content_text(&result).join("\n")),
        Err(AdapterError::NativeTool(message)) if is_empty_plugin_target_error(&message) => {
            Err(ToolError(format!(
                "mixer track {}, slot {slot} is empty; insert a processor there before searching parameters",
                scope.target_track
            )))
        }
        Err(error) => Err(ToolError(error.to_string())),
    }
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

fn search_parameter_manifest(text: &str, query: &str) -> Value {
    let terms = search_terms(query);
    let include_midi = terms.iter().any(|term| term.contains("midi"));
    let mut matches = Vec::new();
    let mut total_matches = 0_usize;
    for line in text.lines() {
        let Some((index, name)) = parse_parameter_line(line) else {
            continue;
        };
        let lower = name.to_lowercase();
        if !include_midi && lower.starts_with("midi cc") {
            continue;
        }
        if terms.iter().any(|term| lower.contains(term)) {
            total_matches += 1;
            if matches.len() < MAX_PARAMETER_SEARCH_MATCHES {
                matches.push(json!({"index": index, "name": name}));
            }
        }
    }
    json!({
        "query": query,
        "terms": terms,
        "matchMode": "any_term",
        "matchCount": total_matches,
        "returned": matches.len(),
        "truncated": total_matches > matches.len(),
        "matches": matches
    })
}

fn search_terms(query: &str) -> Vec<String> {
    let mut terms = query
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | '|' | '/'))
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms
}

fn parse_parameter_line(line: &str) -> Option<(u32, String)> {
    let rest = line.trim().strip_prefix("Index ")?;
    let (index, name) = rest.split_once(':')?;
    let index = index.trim().parse().ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| (index, name.to_owned()))
}

fn parameter_reading(
    adapter: &GopherNativeAdapter,
    target: &str,
    param_identifier: &str,
    slot_number: u32,
) -> Result<ParameterReading, ToolError> {
    let result = adapter
        .call_native(
            "get_plugin_parameter_value",
            json!({
                "target": target,
                "param_identifier": param_identifier,
                "slot_number": slot_number
            }),
        )
        .map_err(|error| ToolError(error.to_string()))?;
    let text = native_content_text(&result).join("\n");
    let normalized = marker_number(&text, "Normalized Value:")
        .or_else(|| first_number(&text))
        .ok_or_else(|| {
            ToolError(format!(
                "could not parse normalized value for parameter `{param_identifier}` from native response"
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

    if before
        .display
        .as_deref()
        .and_then(parse_display_value)
        .is_some_and(|value| value.unit == target.unit && display_error(value, target) <= tolerance)
    {
        return Err(ToolError(format!(
            "parameter `{parameter}` is already at or within tolerance of `{target_display}`; no mutation was performed"
        )));
    }

    let mut samples = Vec::new();
    for normalized in DISPLAY_PROBE_POINTS {
        if let Err(error) = raw_set_parameter(adapter, &scope.target_track, parameter, normalized, slot)
        {
            restore_parameter_raw(adapter, &scope.target_track, parameter, before.normalized, slot);
            return Err(error);
        }
        let reading = match parameter_reading(adapter, &scope.target_track, parameter, slot) {
            Ok(reading) => reading,
            Err(error) => {
                restore_parameter_raw(adapter, &scope.target_track, parameter, before.normalized, slot);
                return Err(error);
            }
        };
        let Some(display) = reading.display.clone() else {
            continue;
        };
        let Some(canonical) = parse_display_value(&display) else {
            continue;
        };
        if canonical.unit == target.unit {
            samples.push(DisplaySample {
                normalized,
                canonical,
                display,
            });
        }
    }

    if samples.is_empty() {
        restore_parameter_raw(adapter, &scope.target_track, parameter, before.normalized, slot);
        return Err(ToolError(format!(
            "parameter `{parameter}` did not expose a numeric {} display mapping; original value restored",
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
            restore_parameter_raw(adapter, &scope.target_track, parameter, before.normalized, slot);
            return Err(ToolError(format!(
                "target `{target_display}` was outside the numeric display range Ghost could safely bracket for `{parameter}` ({sampled}); original value restored"
            )));
        };

        for _ in 0..DISPLAY_TUNE_STEPS {
            let normalized = (low.normalized + high.normalized) * 0.5;
            if let Err(error) = raw_set_parameter(
                adapter,
                &scope.target_track,
                parameter,
                normalized,
                slot,
            ) {
                restore_parameter_raw(adapter, &scope.target_track, parameter, before.normalized, slot);
                return Err(error);
            }
            let reading = match parameter_reading(adapter, &scope.target_track, parameter, slot) {
                Ok(reading) => reading,
                Err(error) => {
                    restore_parameter_raw(adapter, &scope.target_track, parameter, before.normalized, slot);
                    return Err(error);
                }
            };
            let Some(display) = reading.display.clone() else {
                continue;
            };
            let Some(canonical) = parse_display_value(&display) else {
                continue;
            };
            if canonical.unit != target.unit {
                continue;
            }
            let sample = DisplaySample {
                normalized,
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

    // Calibration is intentionally invisible to the mutation journal. Restore the original value
    // before the one final verified write so the journal's `before` field describes the user's real
    // pre-agent setting, not whichever probe happened to run last.
    raw_set_parameter(
        adapter,
        &scope.target_track,
        parameter,
        before.normalized,
        slot,
    )?;
    let restored = parameter_reading(adapter, &scope.target_track, parameter, slot)?;
    if (restored.normalized - before.normalized).abs() > 0.002 {
        return Err(ToolError(format!(
            "Ghost could not restore `{parameter}` after display calibration; refusing the final write"
        )));
    }

    let mutation = adapter
        .set_plugin_parameter_verified(
            &scope.target_track,
            parameter,
            best.normalized,
            slot,
        )
        .map_err(|error| ToolError(error.to_string()))?;
    let after = parameter_reading(adapter, &scope.target_track, parameter, slot)?;
    let after_canonical = after.display.as_deref().and_then(parse_display_value);
    let display_verified = after_canonical
        .is_some_and(|value| value.unit == target.unit && display_error(value, target) <= tolerance);
    if !display_verified {
        restore_parameter_raw(adapter, &scope.target_track, parameter, before.normalized, slot);
        return Err(ToolError(format!(
            "normalized write verified, but `{parameter}` did not read back near display target `{target_display}`; original value restored"
        )));
    }

    Ok(json!({
        "parameter": parameter,
        "targetDisplay": target_display,
        "tolerance": {"value": tolerance, "unit": target.unit.label()},
        "before": reading_json(&before),
        "after": reading_json(&after),
        "mutation": mutation,
        "displayVerified": true
    }))
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

fn restore_parameter_raw(
    adapter: &GopherNativeAdapter,
    target: &str,
    parameter: &str,
    value: f64,
    slot: u32,
) {
    let _ = raw_set_parameter(adapter, target, parameter, value, slot);
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
        DisplayUnit::Db => 0.15,
        DisplayUnit::Hertz => (target.value.abs() * 0.005).max(1.0),
        DisplayUnit::Milliseconds => (target.value.abs() * 0.02).max(0.5),
        DisplayUnit::Percent => 0.25,
        DisplayUnit::Ratio => (target.value.abs() * 0.02).max(0.05),
        DisplayUnit::Unitless => (target.value.abs() * 0.01).max(0.01),
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

fn native_content_text(result: &NativeToolResult) -> Vec<String> {
    if !result.content_text.is_empty() {
        return result.content_text.clone();
    }
    let value = normalize_native_value(result.raw.clone());
    value
        .pointer("/result/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn normalize_native_value(mut value: Value) -> Value {
    for _ in 0..8 {
        let text = match &value {
            Value::String(text) => text.clone(),
            _ => break,
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
            break;
        };
        value = parsed;
    }
    value
}

fn marker_number(text: &str, marker: &str) -> Option<f64> {
    let (_, tail) = text.split_once(marker)?;
    first_number(tail)
}

fn string_value(text: &str) -> Option<String> {
    let (_, tail) = text.split_once("String Value:")?;
    let tail = tail.trim();
    if let Some(rest) = tail.strip_prefix('\'') {
        return rest.split_once('\'').map(|(value, _)| value.to_owned());
    }
    if let Some(rest) = tail.strip_prefix('"') {
        return rest.split_once('"').map(|(value, _)| value.to_owned());
    }
    tail.split(|ch| ch == ',' || ch == '\n')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn first_number(text: &str) -> Option<f64> {
    let mut token = String::new();
    let mut started = false;
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit()
            || (ch == '.' && started)
            || ((ch == '-' || ch == '+') && !started)
        {
            token.push(ch);
            started = true;
        } else if started {
            if let Ok(value) = token.parse::<f64>() {
                if value.is_finite() {
                    return Some(value);
                }
            }
            token.clear();
            started = false;
        }
    }
    None
}

fn parameter_schema(scope: &FlPluginWriteScope, write: bool) -> Value {
    let mut properties = serde_json::Map::<String, Value>::new();
    properties.insert(
        "slot_number".into(),
        json!({"type": "integer", "minimum": scope.slot_start, "maximum": scope.slot_end}),
    );
    properties.insert("param_identifier".into(), json!({"type": "string"}));
    let mut required = vec![json!("slot_number"), json!("param_identifier")];
    if write {
        properties.insert(
            "value".into(),
            json!({"type": "number", "minimum": 0.0, "maximum": 1.0}),
        );
        required.push(json!("value"));
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn ensure_slot(scope: &FlPluginWriteScope, slot: u32) -> Result<(), ToolError> {
    if scope.allows_slot(slot) {
        Ok(())
    } else {
        Err(ToolError(format!(
            "slot {slot} is outside the allowed range {}..={} for mixer track {}",
            scope.slot_start, scope.slot_end, scope.target_track
        )))
    }
}

fn empty_schema() -> Value {
    json!({"type": "object", "properties": {}, "required": [], "additionalProperties": false})
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
            resolve_plugin_name("Plugin Pro-C 3 parameter list\nIndex 1: Threshold", &allowed),
            Some("Pro-C 3".to_owned())
        );
    }

    #[test]
    fn multi_term_parameter_search_is_or_and_hides_midi_noise() {
        let text = "  Index 2: Threshold\n  Index 5: Ratio\n  Index 8: Attack\n  Index 9: Release\n  Index 4169: MIDI CC #72 (Release time)";
        let result = search_parameter_manifest(text, "threshold ratio attack release");
        assert_eq!(result.get("matchCount").and_then(Value::as_u64), Some(4));
        assert_eq!(result.get("returned").and_then(Value::as_u64), Some(4));
    }

    #[test]
    fn normalizes_common_audio_display_units() {
        assert_eq!(parse_display_value("-18.0 dB").unwrap().unit, DisplayUnit::Db);
        assert_eq!(parse_display_value("1.25 kHz").unwrap().value, 1250.0);
        assert_eq!(
            parse_display_value("20 ms").unwrap().unit,
            DisplayUnit::Milliseconds
        );
        assert_eq!(parse_display_value("3.0:1").unwrap().unit, DisplayUnit::Ratio);
    }
}
