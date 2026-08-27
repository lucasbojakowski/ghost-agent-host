use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

static ASSET_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectAsset {
    pub id: String,
    pub label: String,
    pub role: String,
    pub path: String,
    pub analysis_id: Option<String>,
    pub created_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectContext {
    pub schema_version: String,
    pub thread_id: String,
    pub title: String,
    pub description: String,
    pub tempo_bpm: Option<f64>,
    pub time_signature: String,
    pub assets: Vec<ProjectAsset>,
    pub production_plan: Value,
    pub updated_unix_ms: u64,
}

impl ProjectContext {
    fn empty(thread_id: impl Into<String>) -> Self {
        Self {
            schema_version: "ghost.workspace-project/1".into(),
            thread_id: thread_id.into(),
            title: "Untitled production".into(),
            description: String::new(),
            tempo_bpm: None,
            time_signature: "4/4".into(),
            assets: Vec::new(),
            production_plan: empty_plan(),
            updated_unix_ms: unix_ms(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tempo_bpm: Option<Option<f64>>,
    pub time_signature: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssetRequest {
    pub path: String,
    pub label: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssetIdRequest {
    pub asset_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanUpdate {
    pub plan: Value,
}

#[derive(Debug)]
pub(crate) struct WorkspaceProjectHub {
    root: PathBuf,
    active_thread_id: Option<String>,
}

impl WorkspaceProjectHub {
    pub(crate) fn open_default() -> Result<Self> {
        let root = workspace_state_root()?.join("projects");
        fs::create_dir_all(&root)
            .with_context(|| format!("failed to create workspace project directory {}", root.display()))?;
        Ok(Self {
            root,
            active_thread_id: None,
        })
    }

    pub(crate) fn analysis_root(&self) -> PathBuf {
        self.root
            .parent()
            .unwrap_or(&self.root)
            .join("analysis")
    }

    pub(crate) fn activate(&mut self, thread_id: impl Into<String>) -> Result<ProjectContext> {
        let thread_id = thread_id.into();
        self.active_thread_id = Some(thread_id.clone());
        self.load_or_create(&thread_id)
    }

    pub(crate) fn active_thread_id(&self) -> Option<&str> {
        self.active_thread_id.as_deref()
    }

    pub(crate) fn current(&self) -> Result<ProjectContext> {
        let thread_id = self
            .active_thread_id
            .as_deref()
            .context("no workspace thread is active")?;
        self.load_or_create(thread_id)
    }

    pub(crate) fn update(&self, update: ProjectUpdate) -> Result<ProjectContext> {
        let mut project = self.current()?;
        if let Some(title) = update.title {
            let title = title.trim();
            if !title.is_empty() {
                project.title = title.to_owned();
            }
        }
        if let Some(description) = update.description {
            project.description = description.trim().to_owned();
        }
        if let Some(tempo_bpm) = update.tempo_bpm {
            if let Some(value) = tempo_bpm {
                if !value.is_finite() || !(20.0..=400.0).contains(&value) {
                    bail!("tempoBpm must be between 20 and 400 BPM");
                }
            }
            project.tempo_bpm = tempo_bpm;
        }
        if let Some(time_signature) = update.time_signature {
            let time_signature = time_signature.trim();
            if time_signature.is_empty() {
                bail!("timeSignature must not be empty");
            }
            project.time_signature = time_signature.to_owned();
        }
        self.save(project)
    }

    pub(crate) fn add_asset(&self, request: AssetRequest) -> Result<ProjectContext> {
        let mut project = self.current()?;
        let path = normalize_path(&request.path)?;
        let path_text = path.to_string_lossy().into_owned();
        if let Some(existing) = project.assets.iter_mut().find(|asset| asset.path == path_text) {
            if let Some(label) = request.label.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                existing.label = label.to_owned();
            }
            if let Some(role) = request.role.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                existing.role = role.to_owned();
            }
            return self.save(project);
        }

        let label = request
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| path.file_name().and_then(|value| value.to_str()).map(str::to_owned))
            .unwrap_or_else(|| "Audio asset".into());
        let role = request
            .role
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("other")
            .to_owned();

        project.assets.push(ProjectAsset {
            id: next_asset_id(),
            label,
            role,
            path: path_text,
            analysis_id: None,
            created_unix_ms: unix_ms(),
        });
        self.save(project)
    }

    pub(crate) fn remove_asset(&self, asset_id: &str) -> Result<ProjectContext> {
        let mut project = self.current()?;
        let before = project.assets.len();
        project.assets.retain(|asset| asset.id != asset_id);
        if project.assets.len() == before {
            bail!("unknown project asset `{asset_id}`");
        }
        self.save(project)
    }

    pub(crate) fn ensure_asset(
        &self,
        path: &Path,
        label: Option<&str>,
        role: Option<&str>,
    ) -> Result<ProjectAsset> {
        let path_text = path.to_string_lossy().into_owned();
        let mut project = self.current()?;
        if let Some(index) = project.assets.iter().position(|asset| asset.path == path_text) {
            if let Some(label) = label.map(str::trim).filter(|value| !value.is_empty()) {
                project.assets[index].label = label.to_owned();
            }
            if let Some(role) = role.map(str::trim).filter(|value| !value.is_empty()) {
                project.assets[index].role = role.to_owned();
            }
            let asset = project.assets[index].clone();
            self.save(project)?;
            return Ok(asset);
        }

        let asset = ProjectAsset {
            id: next_asset_id(),
            label: label
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .or_else(|| path.file_name().and_then(|value| value.to_str()).map(str::to_owned))
                .unwrap_or_else(|| "Audio asset".into()),
            role: role
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("other")
                .to_owned(),
            path: path_text,
            analysis_id: None,
            created_unix_ms: unix_ms(),
        };
        project.assets.push(asset.clone());
        self.save(project)?;
        Ok(asset)
    }

    pub(crate) fn set_asset_analysis(&self, asset_id: &str, analysis_id: &str) -> Result<()> {
        let mut project = self.current()?;
        let asset = project
            .assets
            .iter_mut()
            .find(|asset| asset.id == asset_id)
            .with_context(|| format!("unknown project asset `{asset_id}`"))?;
        asset.analysis_id = Some(analysis_id.to_owned());
        self.save(project)?;
        Ok(())
    }

    pub(crate) fn set_plan(&self, plan: Value) -> Result<ProjectContext> {
        if !plan.is_object() {
            bail!("production plan must be a JSON object");
        }
        let mut project = self.current()?;
        project.production_plan = plan;
        self.save(project)
    }

    pub(crate) fn compact_prompt_context(&self) -> Result<String> {
        let project = self.current()?;
        let assets = project
            .assets
            .iter()
            .map(|asset| {
                json!({
                    "id": asset.id,
                    "label": asset.label,
                    "role": asset.role,
                    "path": asset.path,
                    "analysisId": asset.analysis_id
                })
            })
            .collect::<Vec<_>>();
        let plan_summary = json!({
            "title": project.production_plan.get("title"),
            "sections": project.production_plan.get("sections").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "channels": project.production_plan.get("channels").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "playlistTracks": project.production_plan.get("playlistTracks").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "mixerInserts": project.production_plan.get("mixerInserts").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
        });
        Ok(serde_json::to_string_pretty(&json!({
            "title": project.title,
            "description": project.description,
            "tempoBpm": project.tempo_bpm,
            "timeSignature": project.time_signature,
            "assets": assets,
            "productionPlanSummary": plan_summary
        }))?)
    }

    fn load_or_create(&self, thread_id: &str) -> Result<ProjectContext> {
        let path = self.project_path(thread_id);
        if !path.exists() {
            let project = ProjectContext::empty(thread_id);
            return self.save(project);
        }
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read workspace project {}", path.display()))?;
        let mut project: ProjectContext = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid workspace project JSON {}", path.display()))?;
        if project.production_plan.is_null() {
            project.production_plan = empty_plan();
        }
        Ok(project)
    }

    fn save(&self, mut project: ProjectContext) -> Result<ProjectContext> {
        project.updated_unix_ms = unix_ms();
        let path = self.project_path(&project.thread_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_vec_pretty(&project)?)
            .with_context(|| format!("failed to persist workspace project {}", path.display()))?;
        Ok(project)
    }

    fn project_path(&self, thread_id: &str) -> PathBuf {
        let safe = thread_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        self.root.join(format!("{safe}.json"))
    }
}

pub(crate) fn normalize_path(path: &str) -> Result<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        bail!("audio path must not be empty");
    }
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("audio file does not exist or cannot be resolved: {path}"))?;
    if !canonical.is_file() {
        bail!("audio path is not a file: {}", canonical.display());
    }
    Ok(canonical)
}

fn workspace_state_root() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Konko")
        .join("Ghost")
        .join("workspace");

    #[cfg(not(target_os = "windows"))]
    let root = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".ghost-workspace"))
        .join("ghost-and-guild")
        .join("workspace");

    fs::create_dir_all(&root)?;
    Ok(root)
}

fn empty_plan() -> Value {
    json!({
        "schemaVersion": "ghost.production-plan/1",
        "title": "",
        "channels": [],
        "playlistTracks": [],
        "mixerInserts": [],
        "sections": [],
        "markers": [],
        "timbres": [],
        "nextSteps": []
    })
}

fn next_asset_id() -> String {
    format!(
        "asset-{:x}-{:x}",
        unix_ms(),
        ASSET_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_project_has_semantic_plan_shape() {
        let project = ProjectContext::empty("thread-test");
        assert_eq!(project.production_plan["schemaVersion"], "ghost.production-plan/1");
        assert!(project.production_plan["sections"].is_array());
    }

    #[test]
    fn asset_ids_are_distinct() {
        assert_ne!(next_asset_id(), next_asset_id());
    }
}
