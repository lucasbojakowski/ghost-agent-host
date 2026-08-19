use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) const MAX_FRAME_BYTES: usize = 64 * 1024;
pub(crate) const MAX_RECEIVE_BUFFER_BYTES: usize = 2 * MAX_FRAME_BYTES;
pub(crate) const IO_CHUNK_BYTES: usize = 4096;

#[derive(Debug, Deserialize)]
pub(crate) struct HelloMessage {
    pub protocol: u32,
    pub bridge: String,
    #[serde(default)]
    pub fl_version: Option<String>,
    #[serde(default)]
    pub scripting_api_version: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResultMessage {
    pub id: u64,
    pub ok: bool,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub error: Option<WireError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WireError {
    #[serde(default)]
    pub kind: Option<String>,
    pub message: String,
}

#[derive(Debug)]
pub(crate) enum IncomingMessage {
    Hello(HelloMessage),
    Result(ResultMessage),
}

pub(crate) fn encode_call(
    id: u64,
    module: &str,
    function: &str,
    args: &[Value],
) -> Result<Vec<u8>, String> {
    let mut frame = serde_json::to_vec(&json!({
        "type": "call",
        "id": id,
        "module": module,
        "function": function,
        "args": args,
    }))
    .map_err(|error| format!("failed to encode FL scripting call: {error}"))?;
    if frame.len().saturating_add(1) > MAX_FRAME_BYTES {
        return Err(format!(
            "FL scripting call exceeded {MAX_FRAME_BYTES} bytes"
        ));
    }
    frame.push(b'\n');
    Ok(frame)
}

pub(crate) fn parse_incoming(frame: &[u8]) -> Result<IncomingMessage, String> {
    let value: Value = serde_json::from_slice(frame)
        .map_err(|error| format!("invalid FL scripting JSON frame: {error}"))?;
    let message_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "FL scripting frame is missing string 'type'".to_owned())?;
    match message_type {
        "hello" => serde_json::from_value(value)
            .map(IncomingMessage::Hello)
            .map_err(|error| format!("invalid FL scripting hello: {error}")),
        "result" => serde_json::from_value(value)
            .map(IncomingMessage::Result)
            .map_err(|error| format!("invalid FL scripting result: {error}")),
        other => Err(format!("unsupported FL scripting frame type '{other}'")),
    }
}

pub(crate) fn decode_result(result: ResultMessage) -> Result<Value, String> {
    if result.ok {
        return Ok(result.value.unwrap_or(Value::Null));
    }
    let error = result.error.map_or_else(
        || "FL scripting call failed without an error payload".into(),
        |error| match error.kind {
            Some(kind) => format!("{kind}: {}", error.message),
            None => error.message,
        },
    );
    Err(error)
}

pub(crate) fn take_frame(buffer: &mut Vec<u8>) -> std::io::Result<Option<Vec<u8>>> {
    let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') else {
        if buffer.len() > MAX_FRAME_BYTES {
            buffer.clear();
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "FL scripting frame exceeded maximum size",
            ));
        }
        return Ok(None);
    };
    if newline > MAX_FRAME_BYTES {
        let _ = buffer.drain(..=newline);
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "FL scripting frame exceeded maximum size",
        ));
    }
    let mut frame: Vec<u8> = buffer.drain(..=newline).collect();
    let _ = frame.pop();
    if frame.last() == Some(&b'\r') {
        let _ = frame.pop();
    }
    if frame.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "FL scripting frame was empty",
        ));
    }
    Ok(Some(frame))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn call_encoding_is_ndjson_and_preserves_id() {
        let encoded = encode_call(17, "patterns", "getPatternName", &[json!(4)]).unwrap();
        assert_eq!(encoded.last(), Some(&b'\n'));
        let decoded: Value = serde_json::from_slice(&encoded[..encoded.len() - 1]).unwrap();
        assert_eq!(decoded["id"], 17);
        assert_eq!(decoded["module"], "patterns");
        assert_eq!(decoded["function"], "getPatternName");
        assert_eq!(decoded["args"], json!([4]));
    }

    #[test]
    fn malformed_json_is_reported_without_panicking() {
        let error = parse_incoming(br#"{"type":"result","id":1,"ok":true"#).unwrap_err();
        assert!(error.contains("invalid FL scripting JSON frame"));
    }

    #[test]
    fn request_id_mismatch_is_detectable() {
        let message =
            parse_incoming(br#"{"type":"result","id":18,"ok":true,"value":"x"}"#).unwrap();
        let IncomingMessage::Result(result) = message else {
            panic!("expected result");
        };
        assert_ne!(result.id, 17);
    }

    #[test]
    fn frame_buffering_is_bounded_and_preserves_partial_frames() {
        let mut partial = br#"{"type":"result""#.to_vec();
        assert!(take_frame(&mut partial).unwrap().is_none());
        partial.extend_from_slice(b"}\n");
        assert_eq!(
            take_frame(&mut partial).unwrap().unwrap(),
            br#"{"type":"result"}"#
        );
        let mut oversized = vec![b'x'; MAX_FRAME_BYTES + 1];
        assert!(take_frame(&mut oversized).is_err());
        assert!(oversized.is_empty());
    }
}
