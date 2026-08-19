use std::collections::BTreeMap;

use ghost_fl_scripting::FlScriptingAdapter;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSnapshot {
    connected: bool,
    values: BTreeMap<String, Value>,
    errors: BTreeMap<String, String>,
}

pub(crate) fn capture_workspace_snapshot(scripting: &FlScriptingAdapter) -> WorkspaceSnapshot {
    let status = scripting.status();
    let mut snapshot = WorkspaceSnapshot {
        connected: status.connected,
        values: BTreeMap::new(),
        errors: BTreeMap::new(),
    };

    if !status.connected {
        snapshot.errors.insert(
            "connection".into(),
            status
                .last_error
                .unwrap_or_else(|| format!("waiting for FL scripting device at {}", status.bind)),
        );
        return snapshot;
    }

    for (key, module, function, args) in [
        ("scriptingApiVersion", "general", "getVersion", vec![]),
        ("flVersion", "ui", "getVersion", vec![json!(5)]),
        ("projectTitle", "general", "getProjectTitle", vec![]),
        ("projectChangedFlag", "general", "getChangedFlag", vec![]),
        ("safeToEdit", "general", "safeToEdit", vec![]),
        ("selectedChannel", "channels", "channelNumber", vec![]),
        ("selectedMixerTrack", "mixer", "trackNumber", vec![]),
        ("mixerTrackCount", "mixer", "trackCount", vec![]),
        ("currentPattern", "patterns", "patternNumber", vec![]),
        ("patternCount", "patterns", "patternCount", vec![]),
        (
            "arrangementSelectionStart",
            "arrangement",
            "selectionStart",
            vec![],
        ),
        (
            "arrangementSelectionEnd",
            "arrangement",
            "selectionEnd",
            vec![],
        ),
        ("focusedPluginName", "ui", "getFocusedPluginName", vec![]),
        (
            "focusedWindowCaption",
            "ui",
            "getFocusedFormCaption",
            vec![],
        ),
        ("songPosition", "transport", "getSongPos", vec![]),
        ("songPositionHint", "transport", "getSongPosHint", vec![]),
        ("loopMode", "transport", "getLoopMode", vec![]),
        ("isPlaying", "transport", "isPlaying", vec![]),
    ] {
        observe_snapshot(scripting, &mut snapshot, key, module, function, args);
    }

    if let Some(pattern) = snapshot
        .values
        .get("currentPattern")
        .and_then(Value::as_i64)
    {
        observe_snapshot(
            scripting,
            &mut snapshot,
            "currentPatternName",
            "patterns",
            "getPatternName",
            vec![json!(pattern)],
        );
    }

    if let (Some(start), Some(end)) = (
        snapshot
            .values
            .get("arrangementSelectionStart")
            .and_then(Value::as_i64),
        snapshot
            .values
            .get("arrangementSelectionEnd")
            .and_then(Value::as_i64),
    ) {
        snapshot
            .values
            .insert("arrangementSelectionActive".into(), json!(start != end));
    }

    snapshot
}

fn observe_snapshot(
    scripting: &FlScriptingAdapter,
    snapshot: &mut WorkspaceSnapshot,
    key: &str,
    module: &str,
    function: &str,
    args: Vec<Value>,
) {
    match scripting.call(module, function, args) {
        Ok(value) => {
            snapshot.values.insert(key.into(), value);
        }
        Err(error) => {
            snapshot.errors.insert(key.into(), error.to_string());
        }
    }
}
