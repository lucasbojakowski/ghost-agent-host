use std::io::Write;
use std::net::TcpStream;

use anyhow::Result;
use ghost_codex::AgentEvent;
use serde::Serialize;
use serde_json::{json, Value};

pub(crate) fn agent_event_value(event: &AgentEvent) -> Option<Value> {
    match event {
        AgentEvent::TurnStarted { .. } => Some(json!({
            "type": "status",
            "label": "Thinking"
        })),
        AgentEvent::ItemStarted { item }
            if item.get("type").and_then(Value::as_str) == Some("reasoning") =>
        {
            Some(json!({
                "type": "status",
                "label": "Reasoning"
            }))
        }
        AgentEvent::ItemCompleted { item }
            if item.get("type").and_then(Value::as_str) == Some("reasoning") =>
        {
            completed_reasoning_summary(item).map(|summary| {
                json!({
                    "type": "reasoning_complete",
                    "text": summary
                })
            })
        }
        AgentEvent::TurnCompleted { .. } => Some(json!({
            "type": "status",
            "label": "Finishing"
        })),
        AgentEvent::Other {
            method: Some(method),
            payload,
        } if method == "item/reasoning/summaryTextDelta" => payload
            .pointer("/params/delta")
            .and_then(Value::as_str)
            .map(|delta| {
                json!({
                    "type": "reasoning_delta",
                    "delta": delta,
                    "summaryIndex": payload.pointer("/params/summaryIndex").cloned()
                })
            }),
        AgentEvent::Other {
            method: Some(method),
            payload,
        } if method == "item/reasoning/summaryPartAdded" => Some(json!({
            "type": "reasoning_section",
            "summaryIndex": payload.pointer("/params/summaryIndex").cloned()
        })),
        // Raw reasoning text is intentionally not exposed to the workspace UI. The readable
        // reasoning-summary stream above is the presentation contract.
        AgentEvent::Other { .. } | AgentEvent::ItemStarted { .. } | AgentEvent::ItemCompleted { .. } => {
            None
        }
    }
}

pub(crate) fn start_chunked_json(stream: &mut TcpStream) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson; charset=utf-8\r\nTransfer-Encoding: chunked\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n"
    )?;
    stream.flush()?;
    Ok(())
}

pub(crate) fn send_chunked_json<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<()> {
    let mut body = serde_json::to_vec(value)?;
    body.push(b'\n');
    write!(stream, "{:X}\r\n", body.len())?;
    stream.write_all(&body)?;
    stream.write_all(b"\r\n")?;
    stream.flush()?;
    Ok(())
}

pub(crate) fn finish_chunked_json(stream: &mut TcpStream) -> Result<()> {
    stream.write_all(b"0\r\n\r\n")?;
    stream.flush()?;
    Ok(())
}

fn completed_reasoning_summary(item: &Value) -> Option<String> {
    let parts = item
        .get("summary")?
        .as_array()?
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_reasoning_summary_delta_is_streamed() {
        let event = AgentEvent::Other {
            method: Some("item/reasoning/summaryTextDelta".into()),
            payload: json!({
                "params": {
                    "delta": "Inspecting the live mixer state",
                    "summaryIndex": 2
                }
            }),
        };
        let value = agent_event_value(&event).unwrap();
        assert_eq!(value["type"], "reasoning_delta");
        assert_eq!(value["delta"], "Inspecting the live mixer state");
        assert_eq!(value["summaryIndex"], 2);
    }

    #[test]
    fn raw_reasoning_text_is_not_streamed() {
        let event = AgentEvent::Other {
            method: Some("item/reasoning/textDelta".into()),
            payload: json!({"params": {"delta": "private reasoning"}}),
        };
        assert!(agent_event_value(&event).is_none());
    }

    #[test]
    fn completed_summary_is_available_as_fallback() {
        let event = AgentEvent::ItemCompleted {
            item: json!({
                "type": "reasoning",
                "summary": [
                    {"type": "summary_text", "text": "First"},
                    {"type": "summary_text", "text": "Second"}
                ]
            }),
        };
        let value = agent_event_value(&event).unwrap();
        assert_eq!(value["type"], "reasoning_complete");
        assert_eq!(value["text"], "First\n\nSecond");
    }
}
