use std::sync::Arc;

use anyhow::{bail, Result};
use ghost_codex::{ToolDefinition, ToolError, ToolRegistry};
use ghost_fl_scripting::{FlScriptingAdapter, FlScriptingCatalog, FlScriptingFunction};
use ghost_fl_studio::{FlStudioManifest, GopherNativeAdapter, NativeToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub(crate) const SCRIPTING_TOOL_NAMES: [&str; 3] = [
    "fl_scripting_search",
    "fl_scripting_describe",
    "fl_scripting_call",
];

#[derive(Debug, Deserialize)]
struct ScriptingSearchArgs {
    query: String,
    #[serde(default)]
    module: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScriptingDescribeArgs {
    module: String,
    function: String,
}

#[derive(Debug, Deserialize)]
struct ScriptingCallArgs {
    module: String,
    function: String,
    args: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptingSearchMatch {
    score: u8,
    module: String,
    function: String,
    signature: Option<String>,
    returns: Option<String>,
    description: Option<String>,
    minimum_api_version: Option<u32>,
    bridge_callable: bool,
    unsupported_reason: Option<String>,
}

pub(crate) fn build_workspace_registry(
    manifest: &FlStudioManifest,
    gopher: Arc<GopherNativeAdapter>,
    scripting: Arc<FlScriptingAdapter>,
) -> Result<ToolRegistry> {
    for gateway_name in SCRIPTING_TOOL_NAMES {
        if manifest.tools.iter().any(|tool| tool.name == gateway_name) {
            bail!("live Gopher catalog collides with workspace gateway tool `{gateway_name}`");
        }
    }

    let definitions = workspace_tool_definitions(manifest);
    let gopher_count = manifest.tools.len();
    let mut registry = ToolRegistry::default();

    for (index, definition) in definitions.into_iter().enumerate() {
        if index < gopher_count {
            let handler_name = definition.name.clone();
            let adapter = Arc::clone(&gopher);
            registry.register(definition, move |arguments| {
                adapter
                    .call_native(&handler_name, arguments)
                    .map(|result| result.raw)
                    .map_err(|error| ToolError(error.to_string()))
            })?;
            continue;
        }

        register_scripting_gateway(&mut registry, definition, &scripting)?;
    }

    Ok(registry)
}

fn register_scripting_gateway(
    registry: &mut ToolRegistry,
    definition: ToolDefinition,
    scripting: &Arc<FlScriptingAdapter>,
) -> Result<()> {
    match definition.name.as_str() {
        "fl_scripting_search" => {
            let catalog = scripting.catalog();
            registry.register(definition, move |arguments| {
                let request: ScriptingSearchArgs = serde_json::from_value(arguments).map_err(
                    |error| ToolError(format!("invalid scripting search arguments: {error}")),
                )?;
                search_scripting_catalog(&catalog, &request.query, request.module.as_deref())
                    .map_err(ToolError)
            })?;
        }
        "fl_scripting_describe" => {
            let catalog = scripting.catalog();
            registry.register(definition, move |arguments| {
                let request: ScriptingDescribeArgs = serde_json::from_value(arguments).map_err(
                    |error| ToolError(format!("invalid scripting describe arguments: {error}")),
                )?;
                describe_scripting_function(&catalog, &request.module, &request.function)
                    .map_err(ToolError)
            })?;
        }
        "fl_scripting_call" => {
            let adapter = Arc::clone(scripting);
            registry.register(definition, move |arguments| {
                let request: ScriptingCallArgs = serde_json::from_value(arguments).map_err(
                    |error| ToolError(format!("invalid scripting call arguments: {error}")),
                )?;
                adapter
                    .call(&request.module, &request.function, request.args)
                    .map_err(|error| ToolError(error.to_string()))
            })?;
        }
        other => bail!("unexpected workspace tool definition `{other}`"),
    }
    Ok(())
}

fn workspace_tool_definitions(manifest: &FlStudioManifest) -> Vec<ToolDefinition> {
    let mut definitions: Vec<ToolDefinition> = manifest
        .tools
        .iter()
        .map(gopher_tool_definition)
        .collect();
    definitions.extend(scripting_gateway_definitions());
    definitions
}

fn gopher_tool_definition(tool: &NativeToolDefinition) -> ToolDefinition {
    ToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
    }
}

fn scripting_gateway_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "fl_scripting_search".into(),
            description: "Search the checked-in FL MIDI Scripting runtime catalog. Use this before scripting calls when the module/function is not already established.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Function/module/signature/description terms"
                    },
                    "module": {
                        "type": "string",
                        "description": "Optional exact FL scripting module filter"
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "fl_scripting_describe".into(),
            description: "Return the evidence-backed FL MIDI Scripting metadata for one exact module/function, including overloads and bridge support.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "module": {"type": "string"},
                    "function": {"type": "string"}
                },
                "required": ["module", "function"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "fl_scripting_call".into(),
            description: "Invoke one explicitly cataloged FL MIDI Scripting primitive with positional JSON arguments through the live loopback bridge.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "module": {"type": "string"},
                    "function": {"type": "string"},
                    "args": {"type": "array", "items": {}}
                },
                "required": ["module", "function", "args"],
                "additionalProperties": false
            }),
        },
    ]
}

fn search_scripting_catalog(
    catalog: &FlScriptingCatalog,
    query: &str,
    module: Option<&str>,
) -> Result<Value, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("scripting search query must not be empty".into());
    }

    let module = module.map(str::trim).filter(|value| !value.is_empty());
    let lowered_query = query.to_ascii_lowercase();
    let terms: Vec<&str> = lowered_query.split_whitespace().collect();
    let mut matches: Vec<(u8, &FlScriptingFunction)> = catalog
        .functions()
        .iter()
        .filter(|entry| module.is_none_or(|name| entry.module.eq_ignore_ascii_case(name)))
        .filter_map(|entry| {
            search_score(entry, &lowered_query, &terms).map(|score| (score, entry))
        })
        .collect();

    matches.sort_by(|(score_a, entry_a), (score_b, entry_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| entry_a.module.cmp(&entry_b.module))
            .then_with(|| entry_a.function.cmp(&entry_b.function))
            .then_with(|| entry_a.signature.cmp(&entry_b.signature))
    });

    let matches: Vec<ScriptingSearchMatch> = matches
        .into_iter()
        .take(25)
        .map(|(score, entry)| ScriptingSearchMatch {
            score,
            module: entry.module.clone(),
            function: entry.function.clone(),
            signature: entry.signature.clone(),
            returns: entry.returns.clone(),
            description: entry.description.clone(),
            minimum_api_version: entry.minimum_api_version,
            bridge_callable: entry.bridge_callable,
            unsupported_reason: entry.unsupported_reason.clone(),
        })
        .collect();

    Ok(json!({
        "query": query,
        "module": module,
        "matches": matches
    }))
}

fn search_score(entry: &FlScriptingFunction, query: &str, terms: &[&str]) -> Option<u8> {
    let signature = entry.signature.as_deref().unwrap_or_default();
    let description = entry.description.as_deref().unwrap_or_default();
    let haystack = format!(
        "{} {} {} {}",
        entry.module, entry.function, signature, description
    )
    .to_ascii_lowercase();

    if !terms.iter().all(|term| haystack.contains(term)) {
        return None;
    }

    let qualified = format!("{}.{}", entry.module, entry.function).to_ascii_lowercase();
    let function = entry.function.to_ascii_lowercase();
    Some(if qualified == query {
        100
    } else if function == query {
        95
    } else if function.starts_with(query) {
        85
    } else if function.contains(query) {
        75
    } else if signature.to_ascii_lowercase().contains(query) {
        65
    } else {
        50
    })
}

fn describe_scripting_function(
    catalog: &FlScriptingCatalog,
    module: &str,
    function: &str,
) -> Result<Value, String> {
    let module = module.trim();
    let function = function.trim();
    let overloads = catalog.describe(module, function);
    if overloads.is_empty() {
        return Err(format!(
            "FL scripting function `{module}.{function}` was not found in the checked-in runtime catalog"
        ));
    }

    Ok(json!({
        "module": module,
        "function": function,
        "overloads": overloads
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn fixture_manifest() -> FlStudioManifest {
        FlStudioManifest {
            adapter: "gopher-native".into(),
            target_title: "FL Studio".into(),
            target_kind: "page".into(),
            target_id: "fixture".into(),
            tools: vec![
                NativeToolDefinition {
                    name: "native_alpha".into(),
                    description: "alpha description".into(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {"value": {"type": "number"}},
                        "required": ["value"]
                    }),
                },
                NativeToolDefinition {
                    name: "native_beta".into(),
                    description: "beta description".into(),
                    input_schema: json!({"type": "object", "properties": {}}),
                },
            ],
        }
    }

    #[test]
    fn preserves_all_gopher_definitions_then_adds_three_gateways() {
        let manifest = fixture_manifest();
        let definitions = workspace_tool_definitions(&manifest);
        assert_eq!(definitions.len(), manifest.tools.len() + 3);

        for (definition, native) in definitions.iter().zip(&manifest.tools) {
            assert_eq!(definition.name, native.name);
            assert_eq!(definition.description, native.description);
            assert_eq!(definition.input_schema, native.input_schema);
        }

        let gateway_names: Vec<&str> = definitions[manifest.tools.len()..]
            .iter()
            .map(|definition| definition.name.as_str())
            .collect();
        assert_eq!(gateway_names, SCRIPTING_TOOL_NAMES);
    }

    #[test]
    fn gateway_names_do_not_expand_into_per_function_tools() {
        let definitions = scripting_gateway_definitions();
        let names: BTreeSet<&str> = definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect();
        assert_eq!(names.len(), 3);
        assert_eq!(names, SCRIPTING_TOOL_NAMES.into_iter().collect());
    }

    #[test]
    fn scripting_search_is_deterministic_and_module_filterable() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        let first = search_scripting_catalog(&catalog, "pattern name", Some("patterns")).unwrap();
        let second = search_scripting_catalog(&catalog, "pattern name", Some("patterns")).unwrap();
        assert_eq!(first, second);

        let matches = first["matches"].as_array().unwrap();
        assert!(matches.iter().any(|entry| {
            entry["module"] == "patterns" && entry["function"] == "getPatternName"
        }));
        assert!(matches.iter().all(|entry| entry["module"] == "patterns"));
    }

    #[test]
    fn scripting_describe_preserves_overloads() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        let described = describe_scripting_function(&catalog, "device", "midiOutMsg").unwrap();
        assert_eq!(described["overloads"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn empty_scripting_search_is_rejected() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        assert!(search_scripting_catalog(&catalog, "  ", None).is_err());
    }
}
