use std::time::Duration;

use anyhow::{bail, Result};
use ghost_fl_scripting::{FlScriptingAdapter, FlScriptingConfig, FlScriptingStatus};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct ScriptingBridge {
    adapter: FlScriptingAdapter,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptingProbe {
    pub status: FlScriptingStatus,
    pub observations: Vec<ProbeEntry>,
    pub reversible_mutation: ReversibleMutationProbe,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeEntry {
    pub label: String,
    pub module: String,
    pub function: String,
    pub args: Vec<Value>,
    pub ok: bool,
    pub value: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReversibleMutationProbe {
    pub operation: &'static str,
    pub attempted: bool,
    pub original_track: Option<i64>,
    pub temporary_track: Option<i64>,
    pub changed: bool,
    pub restored: bool,
    pub error: Option<String>,
}

impl ScriptingBridge {
    pub fn start(bind: &str, timeout: Duration) -> Result<Self> {
        Ok(Self {
            adapter: FlScriptingAdapter::start(FlScriptingConfig {
                bind: bind.to_owned(),
                call_timeout: timeout,
            })?,
        })
    }

    pub fn status(&self) -> FlScriptingStatus {
        self.adapter.status()
    }

    pub fn run_probe(&self) -> Result<ScriptingProbe> {
        run_scripting_probe(&self.adapter)
    }
}

fn run_scripting_probe(scripting: &FlScriptingAdapter) -> Result<ScriptingProbe> {
    let initial_status = scripting.status();
    if !initial_status.connected {
        bail!(
            "FL scripting adapter is not ready; waiting for device_Ghost.py handshake at {}",
            initial_status.bind
        );
    }

    let mut observations = vec![
        probe(
            scripting,
            "scriptingApiVersion",
            "general",
            "getVersion",
            vec![],
        ),
        probe(
            scripting,
            "flVersion",
            "ui",
            "getVersion",
            vec![json!(5)],
        ),
        probe(
            scripting,
            "projectTitle",
            "general",
            "getProjectTitle",
            vec![],
        ),
        probe(
            scripting,
            "projectChangedFlag",
            "general",
            "getChangedFlag",
            vec![],
        ),
        probe(
            scripting,
            "safeToEdit",
            "general",
            "safeToEdit",
            vec![],
        ),
        probe(
            scripting,
            "selectedChannel",
            "channels",
            "channelNumber",
            vec![],
        ),
        probe(
            scripting,
            "selectedMixerTrack",
            "mixer",
            "trackNumber",
            vec![],
        ),
        probe(
            scripting,
            "mixerTrackCount",
            "mixer",
            "trackCount",
            vec![],
        ),
        probe(
            scripting,
            "currentPattern",
            "patterns",
            "patternNumber",
            vec![],
        ),
        probe(
            scripting,
            "patternCount",
            "patterns",
            "patternCount",
            vec![],
        ),
    ];

    if let Some(pattern) = observation_i64(&observations, "currentPattern") {
        observations.push(probe(
            scripting,
            "currentPatternName",
            "patterns",
            "getPatternName",
            vec![json!(pattern)],
        ));
    }

    observations.extend([
        probe(
            scripting,
            "arrangementSelectionStart",
            "arrangement",
            "selectionStart",
            vec![],
        ),
        probe(
            scripting,
            "arrangementSelectionEnd",
            "arrangement",
            "selectionEnd",
            vec![],
        ),
        probe(
            scripting,
            "focusedPluginName",
            "ui",
            "getFocusedPluginName",
            vec![],
        ),
        probe(
            scripting,
            "focusedWindowCaption",
            "ui",
            "getFocusedFormCaption",
            vec![],
        ),
        probe(
            scripting,
            "songPosition",
            "transport",
            "getSongPos",
            vec![],
        ),
        probe(
            scripting,
            "songPositionHint",
            "transport",
            "getSongPosHint",
            vec![],
        ),
        probe(
            scripting,
            "loopMode",
            "transport",
            "getLoopMode",
            vec![],
        ),
        probe(
            scripting,
            "isPlaying",
            "transport",
            "isPlaying",
            vec![],
        ),
    ]);

    if let (Some(start), Some(end)) = (
        observation_i64(&observations, "arrangementSelectionStart"),
        observation_i64(&observations, "arrangementSelectionEnd"),
    ) {
        observations.push(ProbeEntry {
            label: "arrangementSelectionActive".into(),
            module: "derived".into(),
            function: "selectionStart!=selectionEnd".into(),
            args: vec![],
            ok: true,
            value: Some(json!(start != end)),
            error: None,
        });
    }

    let reversible_mutation = reversible_mixer_selection_probe(
        scripting,
        observation_i64(&observations, "safeToEdit"),
        observation_i64(&observations, "selectedMixerTrack"),
        observation_i64(&observations, "mixerTrackCount"),
    );
    Ok(ScriptingProbe {
        status: scripting.status(),
        observations,
        reversible_mutation,
    })
}

fn probe(
    scripting: &FlScriptingAdapter,
    label: &str,
    module: &str,
    function: &str,
    args: Vec<Value>,
) -> ProbeEntry {
    match scripting.call(module, function, args.clone()) {
        Ok(value) => ProbeEntry {
            label: label.into(),
            module: module.into(),
            function: function.into(),
            args,
            ok: true,
            value: Some(value),
            error: None,
        },
        Err(error) => ProbeEntry {
            label: label.into(),
            module: module.into(),
            function: function.into(),
            args,
            ok: false,
            value: None,
            error: Some(error.to_string()),
        },
    }
}

fn reversible_mixer_selection_probe(
    scripting: &FlScriptingAdapter,
    safe_to_edit: Option<i64>,
    original_track: Option<i64>,
    track_count: Option<i64>,
) -> ReversibleMutationProbe {
    let mut report = ReversibleMutationProbe {
        operation: "mixer.setTrackNumber temporary selection + restore",
        attempted: false,
        original_track,
        temporary_track: None,
        changed: false,
        restored: false,
        error: None,
    };
    if safe_to_edit != Some(1) {
        report.error = Some("skipped because general.safeToEdit() did not return 1".into());
        return report;
    }
    let Some(original) = original_track else {
        report.error = Some("skipped because current mixer track was unavailable".into());
        return report;
    };
    if original < 0 || track_count.unwrap_or(0) < 2 {
        report.error = Some("skipped because no alternate mixer track was available".into());
        return report;
    }

    let temporary = if original == 0 { 1 } else { 0 };
    report.temporary_track = Some(temporary);
    report.attempted = true;
    if let Err(error) = scripting.call("mixer", "setTrackNumber", vec![json!(temporary)]) {
        report.error = Some(format!("temporary selection failed: {error}"));
        return report;
    }
    match scripting.call("mixer", "trackNumber", vec![]) {
        Ok(value) if value.as_i64() == Some(temporary) => report.changed = true,
        Ok(value) => {
            report.error = Some(format!(
                "temporary mixer selection read back as {value}, expected {temporary}"
            ));
        }
        Err(error) => report.error = Some(format!("temporary readback failed: {error}")),
    }

    if let Err(error) = scripting.call("mixer", "setTrackNumber", vec![json!(original)]) {
        report.error = Some(append_error(
            report.error.take(),
            format!("restore call failed: {error}"),
        ));
        return report;
    }
    match scripting.call("mixer", "trackNumber", vec![]) {
        Ok(value) if value.as_i64() == Some(original) => report.restored = true,
        Ok(value) => {
            report.error = Some(append_error(
                report.error.take(),
                format!("restore read back as {value}, expected {original}"),
            ));
        }
        Err(error) => {
            report.error = Some(append_error(
                report.error.take(),
                format!("restore readback failed: {error}"),
            ));
        }
    }
    report
}

fn observation_i64(observations: &[ProbeEntry], label: &str) -> Option<i64> {
    observations
        .iter()
        .find(|entry| entry.label == label && entry.ok)
        .and_then(|entry| entry.value.as_ref())
        .and_then(Value::as_i64)
}

fn append_error(existing: Option<String>, next: String) -> String {
    match existing {
        Some(existing) => format!("{existing}; {next}"),
        None => next,
    }
}
