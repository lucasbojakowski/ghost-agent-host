use std::sync::Mutex;

use serde_json::json;

use super::*;

fn fixture_manifest() -> FlStudioManifest {
    FlStudioManifest {
        adapter: "gopher_native".into(),
        target_title: "fixture".into(),
        target_kind: "page".into(),
        target_id: "fixture-id".into(),
        tools: vec![
            NativeToolDefinition {
                name: "zeta_tool".into(),
                description: "zeta description".into(),
                input_schema: json!({
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "type": "object",
                    "properties": {"value": {"type": "number"}},
                    "required": ["value"]
                }),
            },
            NativeToolDefinition {
                name: "alpha_tool".into(),
                description: "alpha description".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {"target": {"type": "string"}}
                }),
            },
        ],
    }
}

#[derive(Clone)]
struct RecordingCaller {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    result: NativeToolResult,
}

impl NativeToolCaller for RecordingCaller {
    fn call_native(&self, tool: &str, arguments: Value) -> Result<NativeToolResult, AdapterError> {
        self.calls
            .lock()
            .expect("recording caller lock")
            .push((tool.to_owned(), arguments));
        Ok(self.result.clone())
    }
}

#[test]
fn manifest_conversion_is_sorted_and_lossless() {
    let manifest = fixture_manifest();
    let tools = mcp_tools_from_manifest(&manifest).unwrap();
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>(),
        vec!["alpha_tool", "zeta_tool"]
    );

    for definition in &manifest.tools {
        let tool = tools
            .iter()
            .find(|tool| tool.name.as_ref() == definition.name)
            .unwrap();
        assert_eq!(
            tool.description.as_deref(),
            Some(definition.description.as_str())
        );
        assert_eq!(tool.schema_as_json_value(), definition.input_schema);
    }
}

#[test]
fn rejects_non_object_input_schema_instead_of_rewriting_it() {
    let mut manifest = fixture_manifest();
    manifest.tools[0].input_schema = json!(true);
    assert!(matches!(
        mcp_tools_from_manifest(&manifest),
        Err(McpEdgeError::InvalidToolSchema { .. })
    ));
}

#[tokio::test]
async fn dispatches_exact_tool_name_and_arguments() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let caller = Arc::new(RecordingCaller {
        calls: Arc::clone(&calls),
        result: NativeToolResult {
            tool: "alpha_tool".into(),
            raw: json!({"result": {"content": [{"type": "text", "text": "ok"}]}}),
            content_text: vec!["ok".into()],
        },
    });
    let server = FlMcpServer::new_with_caller(&fixture_manifest(), caller).unwrap();
    let arguments = json!({"target": "Kick"}).as_object().unwrap().clone();

    server
        .dispatch("alpha_tool".into(), arguments)
        .await
        .unwrap();

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.as_slice(),
        &[("alpha_tool".into(), json!({"target": "Kick"}))]
    );
}

#[tokio::test]
async fn unknown_tool_never_reaches_adapter() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let caller = Arc::new(RecordingCaller {
        calls: Arc::clone(&calls),
        result: NativeToolResult {
            tool: "alpha_tool".into(),
            raw: json!({}),
            content_text: Vec::new(),
        },
    });
    let server = FlMcpServer::new_with_caller(&fixture_manifest(), caller).unwrap();

    assert!(matches!(
        server.dispatch("missing_tool".into(), JsonObject::new()).await,
        Err(DispatchError::UnknownTool(name)) if name == "missing_tool"
    ));
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn native_result_preserves_text_and_raw_structured_content() {
    let mapped = native_result_to_mcp(NativeToolResult {
        tool: "alpha_tool".into(),
        raw: json!({
            "result": {
                "content": [{"type": "text", "text": "native"}],
                "value": 42
            }
        }),
        content_text: vec!["native".into()],
    });
    let value = serde_json::to_value(mapped).unwrap();

    assert_eq!(value.pointer("/content/0/text"), Some(&json!("native")));
    assert_eq!(
        value.pointer("/structuredContent/result/value"),
        Some(&json!(42))
    );
    assert_eq!(value.get("isError"), Some(&json!(false)));
}

#[test]
fn adapter_failures_become_visible_mcp_tool_errors() {
    for error in [
        AdapterError::InvalidArguments("bad args".into()),
        AdapterError::Transport("cdp unavailable".into()),
        AdapterError::NativeTool("Flapi Error: failed".into()),
    ] {
        let value = serde_json::to_value(adapter_error_to_mcp(error)).unwrap();
        assert_eq!(value.get("isError"), Some(&json!(true)));
        assert!(value
            .pointer("/content/0/text")
            .and_then(Value::as_str)
            .is_some());
    }
}

#[test]
fn app_manifest_has_no_codex_or_application_dependency() {
    let cargo_toml = include_str!("../../Cargo.toml");
    assert!(!cargo_toml.contains("ghost-codex"));
    assert!(!cargo_toml.contains("ghost-application"));
}
