//! Filesystem control plane for the lightweight Ghost Tap CLAP plugin.
//!
//! The audio callback never touches this module. A non-realtime worker publishes tap status,
//! consumes capture commands, and commits completed WAV captures. External Ghost processes use the
//! same protocol to discover tap instances and request captures without opening a socket inside the
//! DAW process.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{DawTransportSnapshot, RealtimeCaptureState};

pub const TAP_PROTOCOL: &str = "ghost.tap/1";
pub const TAP_PLUGIN_ID: &str = "ai.konko.ghost-tap";
pub const TAP_STATUS_STALE_AFTER: Duration = Duration::from_secs(5);

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum TapProtocolError {
    #[error("tap filesystem I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("tap JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no live Ghost Tap matched instance {0}")]
    TapNotFound(u32),
    #[error("Ghost Tap capture request timed out")]
    Timeout,
    #[error("invalid Ghost Tap capture duration {0}")]
    InvalidDuration(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TapCaptureCommand {
    pub protocol: String,
    pub request_id: u64,
    pub duration_seconds: f64,
    pub threshold_dbfs: f32,
    pub persistence_blocks: u32,
    pub pre_roll_ms: u32,
}

impl TapCaptureCommand {
    pub fn new(duration_seconds: f64) -> Result<Self, TapProtocolError> {
        if !duration_seconds.is_finite() || !(0.05..=20.0).contains(&duration_seconds) {
            return Err(TapProtocolError::InvalidDuration(duration_seconds));
        }
        Ok(Self {
            protocol: TAP_PROTOCOL.into(),
            request_id: next_request_id(),
            duration_seconds,
            threshold_dbfs: -50.0,
            persistence_blocks: 2,
            pre_roll_ms: 75,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TapStatus {
    pub protocol: String,
    pub plugin_id: String,
    pub process_id: u32,
    pub instance_id: u32,
    pub sample_rate: Option<f64>,
    pub maximum_block_frames: Option<u32>,
    pub capture_state: RealtimeCaptureState,
    pub active_request_id: Option<u64>,
    pub command_path: PathBuf,
    pub artifact_path: PathBuf,
    pub updated_unix_ms: u64,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl TapStatus {
    pub fn is_fresh(&self, now_unix_ms: u64) -> bool {
        now_unix_ms.saturating_sub(self.updated_unix_ms)
            <= TAP_STATUS_STALE_AFTER.as_millis() as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TapCaptureArtifact {
    pub protocol: String,
    pub request_id: u64,
    pub process_id: u32,
    pub instance_id: u32,
    pub sample_rate: u32,
    pub frames: usize,
    pub duration_seconds: f64,
    pub wav_path: PathBuf,
    pub transport: DawTransportSnapshot,
    pub completed_unix_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TapPaths {
    pub root: PathBuf,
    pub status: PathBuf,
    pub command: PathBuf,
    pub artifact: PathBuf,
}

impl TapPaths {
    pub fn for_instance(process_id: u32, instance_id: u32) -> Result<Self, TapProtocolError> {
        let root = tap_root()?;
        let stem = format!("ghost-tap-{process_id}-{instance_id}");
        Ok(Self {
            status: root.join(format!("{stem}.status.json")),
            command: root.join(format!("{stem}.command.json")),
            artifact: root.join(format!("{stem}.capture.json")),
            root,
        })
    }

    pub fn wav_for_request(&self, request_id: u64) -> PathBuf {
        self.root.join(format!("ghost-tap-{request_id}.wav"))
    }
}

pub fn tap_root() -> Result<PathBuf, TapProtocolError> {
    #[cfg(target_os = "windows")]
    let root = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("Konko")
        .join("Ghost")
        .join("taps");

    #[cfg(not(target_os = "windows"))]
    let root = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("konko-ghost")
        .join("taps");

    fs::create_dir_all(&root)?;
    Ok(root)
}

pub fn discover_live_taps() -> Result<Vec<TapStatus>, TapProtocolError> {
    let root = tap_root()?;
    let now = unix_ms();
    let mut taps = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("ghost-tap-") || !name.ends_with(".status.json") {
            continue;
        }
        let status: TapStatus = match read_json(entry.path()) {
            Ok(status) => status,
            Err(_) => continue,
        };
        if status.protocol == TAP_PROTOCOL
            && status.plugin_id == TAP_PLUGIN_ID
            && status.is_fresh(now)
        {
            taps.push(status);
        }
    }
    taps.sort_by_key(|tap| (tap.instance_id, std::cmp::Reverse(tap.updated_unix_ms)));
    Ok(taps)
}

pub fn find_live_tap(instance_id: u32) -> Result<TapStatus, TapProtocolError> {
    discover_live_taps()?
        .into_iter()
        .find(|tap| tap.instance_id == instance_id)
        .ok_or(TapProtocolError::TapNotFound(instance_id))
}

pub fn request_capture(
    status: &TapStatus,
    command: &TapCaptureCommand,
) -> Result<(), TapProtocolError> {
    write_json_atomic(&status.command_path, command)
}

pub fn wait_for_capture(
    status: &TapStatus,
    request_id: u64,
    timeout: Duration,
) -> Result<TapCaptureArtifact, TapProtocolError> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(artifact) = read_json::<TapCaptureArtifact>(&status.artifact_path) {
            if artifact.protocol == TAP_PROTOCOL && artifact.request_id == request_id {
                return Ok(artifact);
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(TapProtocolError::Timeout)
}

pub fn read_capture_command(path: impl AsRef<Path>) -> Result<TapCaptureCommand, TapProtocolError> {
    read_json(path)
}

pub fn publish_tap_status(
    path: impl AsRef<Path>,
    status: &TapStatus,
) -> Result<(), TapProtocolError> {
    write_json_atomic(path.as_ref(), status)
}

pub fn publish_capture_artifact(
    path: impl AsRef<Path>,
    artifact: &TapCaptureArtifact,
) -> Result<(), TapProtocolError> {
    write_json_atomic(path.as_ref(), artifact)
}

pub fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, TapProtocolError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_json_atomic<T: Serialize>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<(), TapProtocolError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp, path)?;
    Ok(())
}

pub fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn next_request_id() -> u64 {
    let time = unix_ms();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed) & 0xffff;
    (time << 16) ^ (u64::from(std::process::id()) << 8) ^ sequence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_command_rejects_unbounded_duration() {
        assert!(TapCaptureCommand::new(4.0).is_ok());
        assert!(TapCaptureCommand::new(0.0).is_err());
        assert!(TapCaptureCommand::new(60.0).is_err());
    }

    #[test]
    fn freshness_is_bounded() {
        let now = 10_000;
        let status = TapStatus {
            protocol: TAP_PROTOCOL.into(),
            plugin_id: TAP_PLUGIN_ID.into(),
            process_id: 1,
            instance_id: 0,
            sample_rate: Some(48_000.0),
            maximum_block_frames: Some(512),
            capture_state: RealtimeCaptureState::Idle,
            active_request_id: None,
            command_path: PathBuf::from("command.json"),
            artifact_path: PathBuf::from("artifact.json"),
            updated_unix_ms: now,
            last_error: None,
        };
        assert!(status.is_fresh(now + 1_000));
        assert!(!status.is_fresh(now + 10_000));
    }
}
