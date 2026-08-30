use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use ghost_fl_studio::{FlStudioAdapterConfig, GopherNativeAdapter};
use serde_json::{json, Value};

const DEFAULT_PLUGINS: &[&str] = &[
    "Fruity Parametric EQ 2",
    "Fruity Reeverb 2",
    "Fruity Delay 3",
    "Fruity Compressor",
    "Fruity Multiband Compressor",
    "Fruity Soft Clipper",
    "Soundgoodizer",
    "Fruity Filter",
    "Fruity Chorus",
    "Fruity Phaser",
    "Fruity Flanger",
    "Spreader",
    "Patcher",
];

const SAMPLE_POINTS: &[f64] = &[0.0, 0.25, 0.5, 0.75, 1.0];

#[derive(Debug, Clone, Args)]
pub struct ExportArgs {
    /// Destination directory. Each plugin is written to its own slug-named folder.
    #[arg(long)]
    pub output: PathBuf,

    /// Empty mixer insert used only for temporary plugin instances.
    #[arg(long, default_value_t = 47)]
    pub probe_track: u32,

    /// Empty visual mixer slot used only for temporary plugin instances.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u8).range(1..=10))]
    pub probe_slot: u8,

    /// Installed FL Plugin Database root used when Gopher browser enumeration fails.
    #[arg(long)]
    pub plugin_database: PathBuf,

    /// Explicit plugin names. When omitted, exports Ghost's stock-effect set.
    #[arg(long = "plugin")]
    pub plugins: Vec<String>,

    /// Permit exact-name verification from local .fst files if live browser enumeration fails.
    #[arg(long)]
    pub allow_local_plugin_database_fallback: bool,

    /// Skip the in-process Browser call after an external preflight already failed.
    #[arg(long)]
    pub skip_live_browser_enumeration: bool,

    /// Overwrite known artifact files in an existing output directory without deleting it.
    #[arg(long)]
    pub overwrite: bool,

    /// FL version label from the request's point-in-time context; not a live Gopher observation.
    #[arg(long)]
    pub context_fl_version: Option<String>,
}

#[derive(Debug, Clone)]
struct ParameterDefinition {
    index: u32,
    name: String,
}

#[derive(Debug, Clone)]
struct ParameterValue {
    normalized: f64,
    display: Option<String>,
    raw_text: String,
}

#[derive(Debug)]
struct PluginExport {
    artifact: Value,
    readme: String,
    raw_parameter_list: String,
    parameter_count: usize,
    validation_ok: bool,
}

pub fn run(adapter_config: FlStudioAdapterConfig, args: ExportArgs) -> Result<()> {
    if args.output.join("manifest.json").exists() && !args.overwrite {
        bail!(
            "{} already contains manifest.json; pass --overwrite to replace known artifacts",
            args.output.display()
        );
    }
    if !args.plugin_database.is_dir() {
        bail!(
            "plugin database root does not exist: {}",
            args.plugin_database.display()
        );
    }

    let plugins: Vec<String> = if args.plugins.is_empty() {
        DEFAULT_PLUGINS
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    } else {
        args.plugins.clone()
    };
    validate_plugin_request(&plugins)?;
    fs::create_dir_all(&args.output)
        .with_context(|| format!("failed to create {}", args.output.display()))?;

    let started_unix_ms = unix_ms();
    // Browser enumeration currently fails natively on some FL builds. Isolate that optional
    // call in a throwaway adapter session so its callback state cannot poison the export session.
    let browser_result = if args.skip_live_browser_enumeration {
        Err(anyhow!(
            "skipped in-process because an external get_browser_names preflight already failed"
        ))
    } else {
        let browser_adapter = GopherNativeAdapter::connect(adapter_config.clone())
            .context("failed to create isolated Browser-enumeration session")?;
        let result = call_text(
            &browser_adapter,
            "get_browser_names",
            json!({"name": "Plugin database", "fullRecursive": 1}),
        );
        drop(browser_adapter);
        result
    };
    let browser_attempt = match (&browser_result, args.skip_live_browser_enumeration) {
        (Ok(text), _) => json!({
            "status": "succeeded",
            "returned_character_count": text.len(),
            "retention": "full browser payload filtered out after exact-name resolution"
        }),
        (Err(error), true) => json!({
            "status": "skipped_after_external_failure",
            "error": error.to_string(),
            "required_external_call": {"name": "Plugin database", "fullRecursive": 1}
        }),
        (Err(error), false) => json!({"status": "failed", "error": error.to_string()}),
    };
    // Keep preflight transport mutation isolated as well. The durable session below begins
    // with the structural read that guards the probe slot.
    let stop_adapter = GopherNativeAdapter::connect(adapter_config.clone())
        .context("failed to create isolated transport-stop session")?;
    let stop_text =
        call_text(&stop_adapter, "stop", json!({})).context("failed to stop FL transport")?;
    drop(stop_adapter);

    // Establish the durable export session only after optional/preflight calls.
    let adapter = GopherNativeAdapter::connect(adapter_config)
        .context("failed to connect export session after Browser enumeration")?;
    let manifest = adapter.manifest()?;
    require_live_tools(
        &manifest
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<BTreeSet<_>>(),
    )?;
    let local_presets = discover_presets(&args.plugin_database)?;
    let requested_resolution = resolve_plugins(
        &plugins,
        browser_result.as_ref().ok().map(String::as_str),
        &local_presets,
        args.allow_local_plugin_database_fallback,
    )?;

    ensure_probe_slot_empty(&adapter, args.probe_track, args.probe_slot)?;

    write_static_artifacts(&args.output, &args, &plugins)?;
    let mut summaries = Vec::new();
    let mut cleanup_failures = Vec::new();

    for (ordinal, plugin) in plugins.iter().enumerate() {
        println!("[{}/{}] probing {plugin}", ordinal + 1, plugins.len());
        ensure_probe_slot_empty(&adapter, args.probe_track, args.probe_slot)?;
        let resolution = requested_resolution
            .get(plugin)
            .cloned()
            .unwrap_or_else(|| json!({"status": "unresolved"}));
        let plugin_dir = args.output.join(slug(plugin));
        fs::create_dir_all(&plugin_dir)?;

        let add_result = call_text(
            &adapter,
            "add_effect",
            json!({
                "plugin": plugin,
                "slot_number": args.probe_slot,
                "target_tracks": args.probe_track.to_string(),
            }),
        );

        let export_result = match add_result {
            Ok(add_text) => export_loaded_plugin(
                &adapter,
                plugin,
                args.probe_track,
                args.probe_slot,
                resolution,
                add_text,
            ),
            Err(error) => Err(error.context(format!("failed to add {plugin}"))),
        };

        let cleanup = call_text(
            &adapter,
            "remove_effect",
            json!({
                "slot_numbers": args.probe_slot.to_string(),
                "target_tracks": args.probe_track.to_string(),
            }),
        );
        let cleanup_verified = match cleanup {
            Ok(text) => {
                match ensure_probe_slot_empty(&adapter, args.probe_track, args.probe_slot) {
                    Ok(()) => json!({"status": "succeeded", "verified_empty": true, "text": text}),
                    Err(error) => {
                        cleanup_failures.push(format!("{plugin}: {error:#}"));
                        json!({"status": "failed", "verified_empty": false, "error": error.to_string(), "text": text})
                    }
                }
            }
            Err(error) => {
                cleanup_failures.push(format!("{plugin}: {error:#}"));
                json!({"status": "failed", "verified_empty": false, "error": error.to_string()})
            }
        };

        match export_result {
            Ok(mut exported) => {
                exported.artifact["cleanup"] = cleanup_verified.clone();
                write_json(&plugin_dir.join("parameter-space.json"), &exported.artifact)?;
                fs::write(plugin_dir.join("README.md"), exported.readme)?;
                fs::write(
                    plugin_dir.join("raw-parameter-list.txt"),
                    exported.raw_parameter_list,
                )?;
                summaries.push(json!({
                    "plugin": plugin,
                    "slug": slug(plugin),
                    "status": if cleanup_verified["status"] == "succeeded" { "complete" } else { "cleanup_failed" },
                    "parameter_count": exported.parameter_count,
                    "validation_ok": exported.validation_ok && cleanup_verified["status"] == "succeeded",
                }));
            }
            Err(error) => {
                let failure = json!({
                    "schema_version": 1,
                    "plugin": {"requested_name": plugin, "slug": slug(plugin)},
                    "status": "failed",
                    "error": format!("{error:#}"),
                    "cleanup": cleanup_verified,
                });
                write_json(&plugin_dir.join("parameter-space.json"), &failure)?;
                fs::write(
                    plugin_dir.join("README.md"),
                    format!("# {plugin}\n\nExport failed: `{error:#}`\n"),
                )?;
                summaries.push(json!({
                    "plugin": plugin,
                    "slug": slug(plugin),
                    "status": "failed",
                    "parameter_count": 0,
                    "validation_ok": false,
                    "error": format!("{error:#}"),
                }));
            }
        }
    }

    let completed_unix_ms = unix_ms();
    let all_ok = summaries.iter().all(|item| {
        item["status"] == "complete" && item["validation_ok"].as_bool().unwrap_or(false)
    });
    let run_manifest = json!({
        "schema_version": 1,
        "status": if all_ok { "complete" } else { "partial" },
        "started_unix_ms": started_unix_ms,
        "completed_unix_ms": completed_unix_ms,
        "duration_ms": completed_unix_ms.saturating_sub(started_unix_ms),
        "output_directory": args.output,
        "live_adapter": {
            "adapter": manifest.adapter,
            "target_title": manifest.target_title,
            "target_kind": manifest.target_kind,
            "tool_count": manifest.tools.len(),
        },
        "context_snapshot": {
            "fl_version": args.context_fl_version,
            "provenance": "request_context_snapshot; not re-observed by this Gopher exporter",
        },
        "transport": {"stop_call": stop_text},
        "probe": {"mixer_track": args.probe_track, "visual_slot": args.probe_slot},
        "browser_enumeration": browser_attempt,
        "plugin_resolution": requested_resolution,
        "sampling": {
            "normalized_domain": [0.0, 1.0],
            "anchor_points": SAMPLE_POINTS,
            "method": "fresh default instance; read default; set/read five anchors one parameter at a time; restore/read default; remove instance",
        },
        "plugins": summaries,
        "cleanup_failures": cleanup_failures,
        "validation": {
            "requested_plugin_count": plugins.len(),
            "artifact_plugin_count": summaries.len(),
            "all_plugins_complete": all_ok,
            "probe_slot_empty_after_run": ensure_probe_slot_empty(&adapter, args.probe_track, args.probe_slot).is_ok(),
        },
    });
    write_json(&args.output.join("manifest.json"), &run_manifest)?;
    write_root_readme(&args.output, &run_manifest)?;
    println!("wrote {}", args.output.display());

    if !cleanup_failures.is_empty() {
        bail!(
            "one or more temporary instances could not be verified as removed: {}",
            cleanup_failures.join("; ")
        );
    }
    Ok(())
}

fn export_loaded_plugin(
    adapter: &GopherNativeAdapter,
    plugin: &str,
    track: u32,
    slot: u8,
    resolution: Value,
    add_text: String,
) -> Result<PluginExport> {
    verify_loaded_plugin(adapter, track, slot, plugin)?;
    let raw_list = call_text(
        adapter,
        "get_plugin_parameter_list",
        json!({"target": track.to_string(), "slot_number": slot}),
    )?;
    let definitions = parse_parameter_list(&raw_list)?;
    if definitions.len() >= 4096
        && definitions
            .iter()
            .all(|parameter| parameter.name.is_empty())
    {
        let probe_indices = [1, definitions.len() as u32 / 2, definitions.len() as u32];
        let probes: Vec<Value> = probe_indices
            .into_iter()
            .map(|index| {
                get_parameter_value(adapter, track, slot, index)
                    .map(|value| {
                        json!({
                            "index": index,
                            "name": null,
                            "normalized": value.normalized,
                            "display": value.display,
                            "raw_text": value.raw_text,
                        })
                    })
                    .unwrap_or_else(|error| json!({"index": index, "error": format!("{error:#}")}))
            })
            .collect();
        let artifact = json!({
            "schema_version": 1,
            "plugin": {"requested_name": plugin, "loaded_name": plugin, "slug": slug(plugin)},
            "status": "complete_structural_surface",
            "evidence": {
                "kind": "live FL Studio Gopher observations from a fresh temporary effect instance",
                "plugin_resolution": resolution,
                "add_effect_result": add_text,
                "parameter_list_result": raw_list,
            },
            "probe": {"mixer_track": track, "visual_slot": slot},
            "parameter_count": definitions.len(),
            "parameter_space": {
                "kind": "compressed_host_placeholder_bank",
                "index_minimum": 1,
                "index_maximum": definitions.len(),
                "all_names_blank": true,
                "semantics": "A fresh empty Patcher publishes a fixed host automation bank. These slots are not meaningful named controls until a Patcher map publishes parameters.",
                "representative_read_probes": probes,
                "sampling_skipped": "Avoided 20,480 meaningless set/read anchor operations on unnamed empty-map placeholders."
            },
            "parameters": [],
            "validation": {
                "ok": true,
                "unique_indices": true,
                "compressed_placeholder_count_matches_list": definitions.len() == 4096,
                "errors": [],
            },
            "limitations": [
                "This is the parameter space of a fresh empty Patcher instance, not a specific populated Patcher map.",
                "Meaningful Patcher parameter names and mappings are map-dependent and must be exported from that populated instance.",
            ],
        });
        let readme = format!(
            "# {plugin}\n\nA fresh empty Patcher instance exposes **{} unnamed host automation slots** (indices 1–{}). This is a reserved, map-dependent publication surface—not 4,096 meaningful stock controls.\n\nThe JSON stores this range compactly and includes representative live read probes. Five-anchor mutation sampling was intentionally skipped because an empty map has no published semantic controls. Export a populated Patcher instance separately to document its named, map-specific parameters.\n",
            definitions.len(),
            definitions.len()
        );
        return Ok(PluginExport {
            artifact,
            readme,
            raw_parameter_list: raw_list,
            parameter_count: definitions.len(),
            validation_ok: true,
        });
    }
    let mut parameters = Vec::new();
    let mut validation_errors = Vec::new();

    for (ordinal, definition) in definitions.iter().enumerate() {
        if ordinal % 10 == 0 {
            println!(
                "  parameter {}/{}: {}",
                ordinal + 1,
                definitions.len(),
                definition.name
            );
        }
        let default = get_parameter_value(adapter, track, slot, definition.index)?;
        let mut samples = Vec::new();
        let mut sample_errors = Vec::new();
        for requested in SAMPLE_POINTS {
            let set_result = call_text(
                adapter,
                "set_plugin_parameter_value",
                json!({
                    "target": track.to_string(),
                    "param_identifier": definition.index.to_string(),
                    "value": requested,
                    "slot_number": slot,
                }),
            );
            match set_result {
                Ok(set_text) => {
                    thread::sleep(Duration::from_millis(20));
                    match get_parameter_value(adapter, track, slot, definition.index) {
                        Ok(observed) => samples.push(json!({
                            "requested_normalized": requested,
                            "observed_normalized": observed.normalized,
                            "display": observed.display,
                            "readback_within_0_001": (observed.normalized - requested).abs() <= 0.001,
                            "set_text": set_text,
                            "read_text": observed.raw_text,
                        })),
                        Err(error) => sample_errors.push(format!(
                            "anchor {requested:.2}: readback failed: {error:#}"
                        )),
                    }
                }
                Err(error) => {
                    sample_errors.push(format!("anchor {requested:.2}: set failed: {error:#}"))
                }
            }
        }

        let restore_set = call_text(
            adapter,
            "set_plugin_parameter_value",
            json!({
                "target": track.to_string(),
                "param_identifier": definition.index.to_string(),
                "value": default.normalized,
                "slot_number": slot,
            }),
        );
        thread::sleep(Duration::from_millis(20));
        let restore_read = get_parameter_value(adapter, track, slot, definition.index);
        let restore_ok = restore_read
            .as_ref()
            .map(|value| (value.normalized - default.normalized).abs() <= 0.001)
            .unwrap_or(false)
            && restore_set.is_ok();
        if !restore_ok {
            validation_errors.push(format!(
                "parameter {} ({}) did not verify restored",
                definition.index, definition.name
            ));
        }
        if !sample_errors.is_empty() {
            validation_errors.push(format!(
                "parameter {} ({}) had {} sample error(s)",
                definition.index,
                definition.name,
                sample_errors.len()
            ));
        }
        let distinct_displays = samples
            .iter()
            .filter_map(|sample| sample["display"].as_str())
            .collect::<BTreeSet<_>>()
            .len();
        parameters.push(json!({
            "index": definition.index,
            "name": definition.name,
            "normalized_domain": {"minimum": 0.0, "maximum": 1.0},
            "default_observation": {
                "normalized": default.normalized,
                "display": default.display,
                "raw_text": default.raw_text,
            },
            "samples": samples,
            "sample_errors": sample_errors,
            "observed_display_cardinality_at_anchors": distinct_displays,
            "restore": {
                "verified": restore_ok,
                "set_text": restore_set.ok(),
                "observed_normalized": restore_read.as_ref().ok().map(|value| value.normalized),
                "observed_display": restore_read.as_ref().ok().and_then(|value| value.display.clone()),
                "error": restore_read.err().map(|error| format!("{error:#}")),
            },
        }));
    }

    let indices: BTreeSet<u64> = parameters
        .iter()
        .filter_map(|parameter| parameter["index"].as_u64())
        .collect();
    if indices.len() != parameters.len() {
        validation_errors.push("duplicate parameter indices".to_owned());
    }
    let validation_ok = validation_errors.is_empty();
    let artifact = json!({
        "schema_version": 1,
        "plugin": {"requested_name": plugin, "loaded_name": plugin, "slug": slug(plugin)},
        "status": if validation_ok { "complete" } else { "complete_with_validation_errors" },
        "evidence": {
            "kind": "live FL Studio Gopher observations from a fresh temporary effect instance",
            "plugin_resolution": resolution,
            "add_effect_result": add_text,
            "parameter_list_result": raw_list,
        },
        "probe": {"mixer_track": track, "visual_slot": slot},
        "parameter_space": {
            "authoritative_automation_domain": "normalized 0.0..1.0",
            "display_mapping": "empirical five-anchor sampling; not a proof of every intermediate or context-dependent display value",
            "anchor_points": SAMPLE_POINTS,
        },
        "parameter_count": parameters.len(),
        "parameters": parameters,
        "validation": {
            "ok": validation_ok,
            "unique_indices": indices.len() == definitions.len(),
            "all_parameters_restored": validation_errors.iter().all(|error| !error.contains("did not verify restored")),
            "errors": validation_errors,
        },
        "limitations": [
            "Continuous domains cannot be exhaustively enumerated; five normalized anchors were measured.",
            "Discrete controls may contain states between sampled anchors.",
            "Display strings can be tempo-, mode-, or neighboring-parameter-dependent.",
            "Patcher exposes only parameters currently published by a fresh empty Patcher instance.",
        ],
    });
    let readme = plugin_readme(plugin, &artifact);
    Ok(PluginExport {
        artifact,
        readme,
        raw_parameter_list: raw_list,
        parameter_count: definitions.len(),
        validation_ok,
    })
}

fn get_parameter_value(
    adapter: &GopherNativeAdapter,
    track: u32,
    slot: u8,
    index: u32,
) -> Result<ParameterValue> {
    let text = call_text(
        adapter,
        "get_plugin_parameter_value",
        json!({
            "target": track.to_string(),
            "param_identifier": index.to_string(),
            "slot_number": slot,
        }),
    )?;
    parse_parameter_value(&text)
}

fn parse_parameter_list(text: &str) -> Result<Vec<ParameterDefinition>> {
    let mut definitions = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("Index ") else {
            continue;
        };
        let (index, name) = rest
            .split_once(':')
            .ok_or_else(|| anyhow!("malformed parameter-list line: {line}"))?;
        definitions.push(ParameterDefinition {
            index: index.trim().parse()?,
            name: name.trim().to_owned(),
        });
    }
    if definitions.is_empty() {
        // Empty Patcher instances can legitimately expose no published controls.
        if text.contains("Patcher") || text.contains("Plugin has no parameters") {
            return Ok(definitions);
        }
        bail!("no parameters parsed from: {text}");
    }
    Ok(definitions)
}

fn parse_parameter_value(text: &str) -> Result<ParameterValue> {
    let normalized_marker = "Normalized Value: ";
    let normalized_start = text
        .find(normalized_marker)
        .ok_or_else(|| anyhow!("normalized marker missing from: {text}"))?
        + normalized_marker.len();
    let remainder = &text[normalized_start..];
    let display_marker = ", String Value: '";
    let (normalized_text, display) = match remainder.find(display_marker) {
        Some(display_offset) => {
            let display_start = normalized_start + display_offset + display_marker.len();
            let display = text[display_start..]
                .strip_suffix('\'')
                .unwrap_or(&text[display_start..])
                .to_owned();
            (&remainder[..display_offset], Some(display))
        }
        None => (remainder, None),
    };
    let normalized: f64 = normalized_text.trim().parse()?;
    Ok(ParameterValue {
        normalized,
        display,
        raw_text: text.to_owned(),
    })
}

fn call_text(adapter: &GopherNativeAdapter, tool: &str, arguments: Value) -> Result<String> {
    let result = adapter.call_native(tool, arguments)?;
    if let Some(text) = result.primary_text() {
        return Ok(text.to_owned());
    }
    extract_primary_text(&result.raw).ok_or_else(|| {
        anyhow!(
            "{tool} returned no decodable text content; raw={}",
            result.raw
        )
    })
}

fn extract_primary_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|decoded| extract_primary_text(&decoded))
            .or_else(|| extract_text_from_malformed_wrapper(text)),
        Value::Object(object) => {
            if let Some(text) = object
                .get("content")
                .and_then(Value::as_array)
                .and_then(|content| content.first())
                .and_then(|item| item.get("text"))
                .and_then(Value::as_str)
            {
                return Some(text.to_owned());
            }
            object.get("result").and_then(extract_primary_text)
        }
        _ => None,
    }
}

fn extract_text_from_malformed_wrapper(wrapper: &str) -> Option<String> {
    // Observed FL 26.1.3 quirk: get_session_context can return a JSON-string wrapper whose
    // nested content text has escaped newlines but unescaped JSON quotes. The wrapper is not
    // valid JSON, although the nested tool text is recoverable without guessing any DAW state.
    let marker = "\"text\": \"";
    let start = wrapper.find(marker)? + marker.len();
    let end_marker = "\"\n      }\n    ],\n    \"isError\"";
    let end = wrapper[start..].rfind(end_marker)? + start;
    Some(
        wrapper[start..end]
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
            .replace("\\\"", "\""),
    )
}

fn ensure_probe_slot_empty(adapter: &GopherNativeAdapter, track: u32, slot: u8) -> Result<()> {
    let session_text = call_text(adapter, "get_session_context", json!({}))?;
    let session: Value = serde_json::from_str(&session_text)
        .with_context(|| "get_session_context text was not JSON")?;
    let occupied = session["active_mixer_tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["index"].as_u64() == Some(track as u64))
        .flat_map(|item| item["effect_plugins"].as_array().into_iter().flatten())
        .find(|effect| effect["slot"].as_u64() == Some(slot as u64));
    if let Some(effect) = occupied {
        bail!(
            "probe mixer track {track} visual slot {slot} is occupied by {}",
            effect["name"].as_str().unwrap_or("an unknown effect")
        );
    }
    Ok(())
}

fn verify_loaded_plugin(
    adapter: &GopherNativeAdapter,
    track: u32,
    slot: u8,
    expected: &str,
) -> Result<()> {
    let session_text = call_text(adapter, "get_session_context", json!({}))?;
    let session: Value = serde_json::from_str(&session_text)?;
    let observed = session["active_mixer_tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["index"].as_u64() == Some(track as u64))
        .flat_map(|item| item["effect_plugins"].as_array().into_iter().flatten())
        .find(|effect| effect["slot"].as_u64() == Some(slot as u64))
        .and_then(|effect| effect["name"].as_str());
    match observed {
        Some(name) if name.eq_ignore_ascii_case(expected) => Ok(()),
        Some(name) => bail!("loaded effect mismatch: expected {expected}, observed {name}"),
        None => bail!("no effect observed on mixer track {track}, visual slot {slot}"),
    }
}

fn discover_presets(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut stack = vec![root.to_path_buf()];
    let mut presets = Vec::new();
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("failed to read {}", directory.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("fst"))
            {
                if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    presets.push((stem.to_owned(), path));
                }
            }
        }
    }
    Ok(presets)
}

fn resolve_plugins(
    plugins: &[String],
    browser_text: Option<&str>,
    local_presets: &[(String, PathBuf)],
    allow_fallback: bool,
) -> Result<Value> {
    let mut result = serde_json::Map::new();
    for plugin in plugins {
        if browser_text.is_some_and(|text| text.contains(plugin)) {
            result.insert(
                plugin.clone(),
                json!({"status": "resolved", "source": "live_get_browser_names"}),
            );
            continue;
        }
        let paths: Vec<String> = local_presets
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(plugin))
            .map(|(_, path)| path.display().to_string())
            .collect();
        if allow_fallback && !paths.is_empty() {
            result.insert(
                plugin.clone(),
                json!({"status": "resolved", "source": "installed_plugin_database_fst", "paths": paths}),
            );
        } else {
            bail!(
                "could not resolve exact installed plugin name {plugin:?}; live browser returned={}, local matches={}",
                browser_text.is_some(),
                paths.len()
            );
        }
    }
    Ok(Value::Object(result))
}

fn require_live_tools(tools: &BTreeSet<&str>) -> Result<()> {
    for required in [
        "stop",
        "get_session_context",
        "get_browser_names",
        "add_effect",
        "remove_effect",
        "get_plugin_parameter_list",
        "get_plugin_parameter_value",
        "set_plugin_parameter_value",
    ] {
        if !tools.contains(required) {
            bail!("live Gopher catalog does not expose required tool {required}");
        }
    }
    Ok(())
}

fn validate_plugin_request(plugins: &[String]) -> Result<()> {
    if plugins.is_empty() {
        bail!("plugin list is empty");
    }
    let unique: BTreeSet<String> = plugins.iter().map(|name| name.to_lowercase()).collect();
    if unique.len() != plugins.len() {
        bail!("plugin list contains duplicates");
    }
    Ok(())
}

fn write_static_artifacts(output: &Path, args: &ExportArgs, plugins: &[String]) -> Result<()> {
    let plan = format!(
        "# Parameter-space export plan\n\n1. Attach to the live Gopher catalog and require all read/write primitives used by the exporter.\n2. Stop transport and verify mixer Insert {}, visual slot {}, is empty.\n3. Resolve every requested plugin by exact name through live Browser enumeration or an explicit local `.fst` fallback.\n4. For each plugin, add a fresh temporary instance, verify the loaded name, enumerate parameters, read defaults, sample normalized anchors 0/0.25/0.5/0.75/1, restore every default, and remove the instance.\n5. Write JSON plus natural-language artifacts per plugin.\n6. Validate unique indices, readbacks/restoration, plugin-folder completeness, and final slot cleanup.\n\nRequested plugins: {}\n",
        args.probe_track,
        args.probe_slot,
        plugins.join(", ")
    );
    fs::write(output.join("PLAN.md"), plan)?;
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Ghost FL plugin parameter-space artifact (compact contract)",
        "type": "object",
        "required": ["schema_version", "plugin", "status"],
        "properties": {
            "schema_version": {"const": 1},
            "plugin": {"type": "object"},
            "status": {"type": "string"},
            "parameter_count": {"type": "integer", "minimum": 0},
            "parameters": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["index", "name", "default_observation", "samples", "restore"]
                }
            }
        }
    });
    write_json(&output.join("schema.json"), &schema)?;
    Ok(())
}

fn plugin_readme(plugin: &str, artifact: &Value) -> String {
    if artifact["parameter_count"].as_u64() == Some(0) {
        return format!(
            "# {plugin}\n\nA fresh temporary `{plugin}` instance returned **`Plugin has no parameters.`** through the live Gopher automation surface. The empirical host-exposed parameter count is therefore **0** for this build and wrapper.\n\nThis does not claim that the plugin UI has no controls; it means those controls were not published through `get_plugin_parameter_list` in this observation. See `parameter-space.json` and `raw-parameter-list.txt` for the exact evidence and provenance.\n"
        );
    }
    let mut text = format!(
        "# {plugin}\n\nThis is an empirical automation-parameter map from a fresh temporary FL Studio instance. FL's normalized 0–1 domain is authoritative; display values below are sampled observations, not an exhaustive mathematical model.\n\nParameters: **{}**. Validation: **{}**.\n\n| # | Parameter | Default | 0 | 0.25 | 0.5 | 0.75 | 1 | Restored |\n|---:|---|---|---|---|---|---|---|:---:|\n",
        artifact["parameter_count"],
        if artifact["validation"]["ok"].as_bool().unwrap_or(false) { "passed" } else { "has errors" }
    );
    if let Some(parameters) = artifact["parameters"].as_array() {
        for parameter in parameters {
            let default = format!(
                "{} ({})",
                parameter["default_observation"]["display"]
                    .as_str()
                    .unwrap_or("?"),
                parameter["default_observation"]["normalized"]
                    .as_f64()
                    .unwrap_or_default()
            );
            let samples: Vec<String> = parameter["samples"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|sample| escape_markdown(sample["display"].as_str().unwrap_or("?")))
                .collect();
            let sample = |index: usize| samples.get(index).cloned().unwrap_or_else(|| "—".into());
            text.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                parameter["index"],
                escape_markdown(parameter["name"].as_str().unwrap_or("?")),
                escape_markdown(&default),
                sample(0),
                sample(1),
                sample(2),
                sample(3),
                sample(4),
                if parameter["restore"]["verified"].as_bool().unwrap_or(false) {
                    "yes"
                } else {
                    "no"
                }
            ));
        }
    }
    text.push_str("\nCaveats: continuous controls are sampled at five anchors; discrete states can exist between anchors; display text may depend on tempo, mode, or other controls. See `parameter-space.json` for raw readback evidence and validation details.\n");
    text
}

fn write_root_readme(output: &Path, manifest: &Value) -> Result<()> {
    let mut text = format!(
        "# FL Studio plugin parameter spaces\n\nStatus: **{}**. Generated empirically through the live FL/Gopher bridge. Each plugin folder contains `parameter-space.json`, a producer-readable `README.md`, and the raw parameter-list response.\n\n| Plugin | Parameters | Status | Validation |\n|---|---:|---|:---:|\n",
        manifest["status"].as_str().unwrap_or("unknown")
    );
    for plugin in manifest["plugins"].as_array().into_iter().flatten() {
        text.push_str(&format!(
            "| [{}]({}/README.md) | {} | {} | {} |\n",
            plugin["plugin"].as_str().unwrap_or("?"),
            plugin["slug"].as_str().unwrap_or("?"),
            plugin["parameter_count"],
            plugin["status"].as_str().unwrap_or("?"),
            if plugin["validation_ok"].as_bool().unwrap_or(false) {
                "pass"
            } else {
                "fail"
            }
        ));
    }
    text.push_str("\nThe artifacts distinguish live FL observations from the request-context FL version label and from inferred limitations. `PLAN.md` records the workflow; `manifest.json` records run-level evidence and cleanup.\n");
    fs::write(output.join("README.md"), text)?;
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let encoded = serde_json::to_string_pretty(value)?;
    // Reparse before writing so malformed serialization can never become an artifact.
    let _: Value = serde_json::from_str(&encoded)?;
    fs::write(path, format!("{encoded}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn slug(value: &str) -> String {
    let mut result = String::new();
    let mut previous_dash = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            result.push(character);
            previous_dash = false;
        } else if !previous_dash && !result.is_empty() {
            result.push('-');
            previous_dash = true;
        }
    }
    result.trim_end_matches('-').to_owned()
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_parameter_list() {
        let parsed = parse_parameter_list(
            "Parameters for 'Example':\n  Index 1: Low cut\n  Index 2: Wet level",
        )
        .unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].index, 1);
        assert_eq!(parsed[1].name, "Wet level");
    }

    #[test]
    fn parses_parameter_value() {
        let parsed = parse_parameter_value(
            "Value for 'Low cut' (Visual Index 1): Normalized Value: 0.0188, String Value: '75Hz'",
        )
        .unwrap();
        assert!((parsed.normalized - 0.0188).abs() < f64::EPSILON);
        assert_eq!(parsed.display.as_deref(), Some("75Hz"));
    }

    #[test]
    fn parses_parameter_value_without_display_string() {
        let parsed = parse_parameter_value(
            "Value for 'Cutoff frequency' (Visual Index 1): Normalized Value: 1.0000",
        )
        .unwrap();
        assert_eq!(parsed.normalized, 1.0);
        assert_eq!(parsed.display, None);
    }

    #[test]
    fn creates_stable_slug() {
        assert_eq!(slug("Fruity Parametric EQ 2"), "fruity-parametric-eq-2");
    }

    #[test]
    fn extracts_text_from_string_wrapped_mcp_response() {
        let inner = json!({"result": {"content": [{"type": "text", "text": "hello"}]}});
        let wrapped = Value::String(inner.to_string());
        assert_eq!(extract_primary_text(&wrapped).as_deref(), Some("hello"));
    }

    #[test]
    fn extracts_text_from_malformed_string_wrapper() {
        let wrapped = Value::String(
            "{\n  \"result\": {\n    \"content\": [\n      {\n        \"text\": \"{\\n  \"tempo_bpm\": 122.0\\n}\"\n      }\n    ],\n    \"isError\": false\n  }\n}"
                .to_owned(),
        );
        assert_eq!(
            extract_primary_text(&wrapped).as_deref(),
            Some("{\n  \"tempo_bpm\": 122.0\n}")
        );
    }
}
