use super::*;
use std::sync::{Arc, Mutex};

#[test]
fn codex_output_schema_is_strict() {
    let mut schema = serde_json::to_value(schemars::schema_for!(MixPlan)).unwrap();
    normalize_output_schema(&mut schema);
    assert_strict_objects(&schema);
    assert!(!contains_key(&schema, "$schema"));
    assert!(!contains_key(&schema, "format"));
    assert!(!contains_key(&schema, "oneOf"));
}

#[cfg(target_os = "windows")]
#[test]
fn native_windows_binary_is_preferred_over_command_shims() {
    let candidates = vec![
        PathBuf::from(r"C:\tools\codex.cmd"),
        PathBuf::from(r"C:\apps\codex.exe"),
    ];
    assert_eq!(
        preferred_windows_candidate(&candidates),
        Some(PathBuf::from(r"C:\apps\codex.exe"))
    );
}

fn assert_strict_objects(value: &Value) {
    match value {
        Value::Object(object) => {
            if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                assert_eq!(
                    object.get("additionalProperties"),
                    Some(&Value::Bool(false))
                );
                let required = object
                    .get("required")
                    .and_then(Value::as_array)
                    .expect("objects with properties must list required fields");
                assert_eq!(required.len(), properties.len());
            }
            object.values().for_each(assert_strict_objects);
        }
        Value::Array(items) => items.iter().for_each(assert_strict_objects),
        _ => {}
    }
}

fn contains_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key(key) || object.values().any(|child| contains_key(child, key))
        }
        Value::Array(items) => items.iter().any(|child| contains_key(child, key)),
        _ => false,
    }
}

struct QueueTransport {
    incoming: VecDeque<Value>,
    sent: Arc<Mutex<Vec<Value>>>,
}

impl RpcTransport for QueueTransport {
    fn send(&mut self, message: &Value) -> Result<(), AgentError> {
        self.sent.lock().unwrap().push(message.clone());
        Ok(())
    }

    fn receive(&mut self) -> Result<Value, AgentError> {
        self.incoming
            .pop_front()
            .ok_or_else(|| AgentError::Protocol("script exhausted".into()))
    }

    fn shutdown(&mut self) {}
}

#[test]
fn scripted_transport_covers_thread_events_and_dynamic_output_schema() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let transport = QueueTransport {
        incoming: VecDeque::from([
            json!({"id": 1, "result": {}}),
            json!({"id": 2, "result": {"thread": {"id": "thread-test"}}}),
            json!({"id": 3, "result": {"turn": {"id": "turn-test"}}}),
            json!({"method": "turn/started", "params": {"turn": {"id": "turn-test"}}}),
            json!({"method": "item/completed", "params": {"item": {
                "type": "agentMessage", "text": "{\"answer\":\"ok\"}"
            }}}),
            json!({"method": "turn/completed", "params": {"turn": {
                "id": "turn-test", "status": "completed"
            }}}),
        ]),
        sent: Arc::clone(&sent),
    };
    let mut agent = CodexAppServerAgent::from_transport(
        Box::new(transport),
        "test-model",
        ToolRegistry::default(),
    )
    .unwrap();
    let context = ghost_context::CompiledContext {
        schema_version: ghost_context::CompiledContext::SCHEMA.into(),
        messages: vec![ghost_context::ContextMessage {
            role: ghost_context::MessageRole::User,
            content: "Respond".into(),
        }],
        output: ghost_context::OutputContract::Json {
            schema_name: "answer".into(),
            schema: json!({
                "type": "object",
                "properties": {"answer": {"type": "string"}}
            }),
        },
        metadata: Value::Null,
    };
    let mut events = Vec::new();
    let output = agent
        .run_turn(&context, &TurnOptions::default(), &mut |event| {
            events.push(event)
        })
        .unwrap();
    assert_eq!(output.structured, Some(json!({"answer": "ok"})));
    assert!(events
        .iter()
        .any(|event| matches!(event, AgentEvent::TurnStarted { .. })));
    let sent = sent.lock().unwrap();
    assert_eq!(
        sent[3]["params"]["outputSchema"]["additionalProperties"],
        false
    );
}
