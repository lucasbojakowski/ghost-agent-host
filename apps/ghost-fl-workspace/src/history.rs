use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

const USER_REQUEST_MARKER: &str = "\n\nUSER REQUEST:\n";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadHistoryResponse {
    pub thread_id: Option<String>,
    pub messages: Vec<HistoryMessage>,
}

impl ThreadHistoryResponse {
    pub fn empty() -> Self {
        Self {
            thread_id: None,
            messages: Vec::new(),
        }
    }

    pub fn from_thread_read(thread_id: &str, result: &Value) -> Result<Self> {
        let turns = result
            .pointer("/thread/turns")
            .and_then(Value::as_array)
            .context("thread/read with includeTurns did not return thread.turns")?;
        let mut messages = Vec::new();

        for turn in turns {
            let Some(items) = turn.get("items").and_then(Value::as_array) else {
                continue;
            };
            let mut trace = Vec::new();
            let first_message_index = messages.len();

            for item in items {
                match item.get("type").and_then(Value::as_str) {
                    Some("userMessage") => {
                        if let Some(text) = user_message_text(item) {
                            messages.push(HistoryMessage {
                                role: "user",
                                text: visible_user_text(&text).to_owned(),
                                trace: Vec::new(),
                            });
                        }
                    }
                    Some("agentMessage") => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            if !text.trim().is_empty() {
                                messages.push(HistoryMessage {
                                    role: "assistant",
                                    text: text.to_owned(),
                                    trace: Vec::new(),
                                });
                            }
                        }
                    }
                    Some("dynamicToolCall") => {
                        trace.extend(history_trace(item));
                    }
                    _ => {}
                }
            }

            if !trace.is_empty() {
                if let Some(message) = messages[first_message_index..]
                    .iter_mut()
                    .rev()
                    .find(|message| message.role == "assistant")
                {
                    message.trace = trace;
                }
            }
        }

        Ok(Self {
            thread_id: Some(thread_id.to_owned()),
            messages,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMessage {
    pub role: &'static str,
    pub text: String,
    pub trace: Vec<HistoryTraceEvent>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryTraceEvent {
    pub kind: &'static str,
    pub tool: String,
    pub arguments: Option<Value>,
    pub success: Option<bool>,
    pub duration_ms: Option<u64>,
}

fn user_message_text(item: &Value) -> Option<String> {
    let content = item.get("content")?.as_array()?;
    let text = content
        .iter()
        .filter(|input| input.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|input| input.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn visible_user_text(text: &str) -> &str {
    text.rsplit_once(USER_REQUEST_MARKER)
        .map(|(_, request)| request)
        .unwrap_or(text)
}

fn history_trace(item: &Value) -> Vec<HistoryTraceEvent> {
    let tool = item
        .get("tool")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>")
        .to_owned();
    let arguments = item.get("arguments").cloned();
    vec![
        HistoryTraceEvent {
            kind: "tool_started",
            tool: tool.clone(),
            arguments: arguments.clone(),
            success: None,
            duration_ms: None,
        },
        HistoryTraceEvent {
            kind: "tool_completed",
            tool,
            arguments,
            success: item.get("success").and_then(Value::as_bool),
            duration_ms: item
                .get("durationMs")
                .and_then(Value::as_i64)
                .and_then(|duration| u64::try_from(duration).ok()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rehydrates_visible_messages_and_hides_workspace_envelope() {
        let result = json!({
            "thread": {
                "turns": [{
                    "id": "turn-a",
                    "items": [
                        {
                            "type": "userMessage",
                            "id": "user-a",
                            "content": [{
                                "type": "text",
                                "text": "SYSTEM\n\nPOINT-IN-TIME FL MIDI SCRIPTING SNAPSHOT:\n{}\n\nUSER REQUEST:\nMake the kick louder"
                            }]
                        },
                        {
                            "type": "dynamicToolCall",
                            "id": "tool-a",
                            "tool": "fl_scripting_call",
                            "arguments": {"module": "mixer"},
                            "success": true,
                            "durationMs": 4
                        },
                        {
                            "type": "agentMessage",
                            "id": "agent-a",
                            "text": "Done."
                        }
                    ],
                    "status": "completed"
                }]
            }
        });

        let history = ThreadHistoryResponse::from_thread_read("thread-a", &result).unwrap();
        assert_eq!(history.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(history.messages.len(), 2);
        assert_eq!(history.messages[0].role, "user");
        assert_eq!(history.messages[0].text, "Make the kick louder");
        assert_eq!(history.messages[1].role, "assistant");
        assert_eq!(history.messages[1].text, "Done.");
        assert_eq!(history.messages[1].trace.len(), 2);
        assert_eq!(history.messages[1].trace[0].kind, "tool_started");
        assert_eq!(history.messages[1].trace[1].success, Some(true));
    }

    #[test]
    fn preserves_plain_user_messages_from_other_clients() {
        let result = json!({
            "thread": {
                "turns": [{
                    "items": [{
                        "type": "userMessage",
                        "content": [{"type": "text", "text": "plain message"}]
                    }]
                }]
            }
        });
        let history = ThreadHistoryResponse::from_thread_read("thread-a", &result).unwrap();
        assert_eq!(history.messages[0].text, "plain message");
    }
}
