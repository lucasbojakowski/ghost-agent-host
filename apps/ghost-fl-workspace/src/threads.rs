use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceThreadRecord {
    pub id: String,
    pub name: Option<String>,
    pub forked_from_id: Option<String>,
    pub has_turns: bool,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceThreadState {
    selected_thread_id: Option<String>,
    threads: Vec<WorkspaceThreadRecord>,
}

pub struct WorkspaceThreadStore {
    path: PathBuf,
    state: WorkspaceThreadState,
}

impl WorkspaceThreadStore {
    pub fn open_default() -> Result<Self> {
        Self::open(default_store_path())
    }

    fn open(path: PathBuf) -> Result<Self> {
        let state = if path.is_file() {
            let bytes = fs::read(&path)
                .with_context(|| format!("failed to read workspace thread state {}", path.display()))?;
            serde_json::from_slice(&bytes).with_context(|| {
                format!("workspace thread state was invalid JSON at {}", path.display())
            })?
        } else {
            WorkspaceThreadState::default()
        };
        Ok(Self { path, state })
    }

    pub fn selected_id(&self) -> Option<&str> {
        self.state.selected_thread_id.as_deref()
    }

    pub fn selected_record(&self) -> Option<&WorkspaceThreadRecord> {
        self.selected_id().and_then(|id| self.record(id))
    }

    pub fn record(&self, id: &str) -> Option<&WorkspaceThreadRecord> {
        self.state.threads.iter().find(|thread| thread.id == id)
    }

    pub fn list(&self) -> Vec<WorkspaceThreadRecord> {
        let mut threads = self.state.threads.clone();
        threads.sort_by(|left, right| {
            right
                .updated_at_unix_ms
                .cmp(&left.updated_at_unix_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        threads
    }

    pub fn len(&self) -> usize {
        self.state.threads.len()
    }

    pub fn register(
        &mut self,
        id: String,
        forked_from_id: Option<String>,
        has_turns: bool,
    ) -> Result<WorkspaceThreadRecord> {
        let now = unix_ms();
        if let Some(existing) = self.state.threads.iter_mut().find(|thread| thread.id == id) {
            existing.updated_at_unix_ms = now;
            existing.has_turns |= has_turns;
            if existing.forked_from_id.is_none() {
                existing.forked_from_id = forked_from_id;
            }
        } else {
            self.state.threads.push(WorkspaceThreadRecord {
                id: id.clone(),
                name: None,
                forked_from_id,
                has_turns,
                created_at_unix_ms: now,
                updated_at_unix_ms: now,
            });
        }
        self.state.selected_thread_id = Some(id.clone());
        self.persist()?;
        self.record(&id)
            .cloned()
            .context("registered workspace thread disappeared from state")
    }

    pub fn select(&mut self, id: &str) -> Result<WorkspaceThreadRecord> {
        if self.record(id).is_none() {
            bail!("unknown workspace thread `{id}`");
        }
        self.state.selected_thread_id = Some(id.to_owned());
        self.persist()?;
        self.record(id)
            .cloned()
            .context("selected workspace thread disappeared from state")
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<WorkspaceThreadRecord> {
        let name = name.trim();
        if name.is_empty() {
            bail!("thread name must not be empty");
        }
        let now = unix_ms();
        let thread = self
            .state
            .threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .with_context(|| format!("unknown workspace thread `{id}`"))?;
        thread.name = Some(name.to_owned());
        thread.updated_at_unix_ms = now;
        let record = thread.clone();
        self.persist()?;
        Ok(record)
    }

    pub fn mark_turn(&mut self, id: &str) -> Result<WorkspaceThreadRecord> {
        let now = unix_ms();
        let thread = self
            .state
            .threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .with_context(|| format!("unknown workspace thread `{id}`"))?;
        thread.has_turns = true;
        thread.updated_at_unix_ms = now;
        let record = thread.clone();
        self.persist()?;
        Ok(record)
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create workspace state directory {}", parent.display())
            })?;
        }
        let bytes = serde_json::to_vec_pretty(&self.state)?;
        fs::write(&self.path, bytes)
            .with_context(|| format!("failed to write workspace thread state {}", self.path.display()))
    }
}

fn default_store_path() -> PathBuf {
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("Konko")
            .join("Ghost")
            .join("workspace")
            .join("threads.json");
    }
    if let Some(xdg_state) = env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(xdg_state)
            .join("ghost-and-guild")
            .join("workspace")
            .join("threads.json");
    }
    env::current_dir()
        .unwrap_or_else(|_| Path::new(".").to_path_buf())
        .join(".ghost-workspace")
        .join("threads.json")
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> WorkspaceThreadStore {
        let path = env::temp_dir().join(format!(
            "ghost-workspace-threads-{name}-{}-{}.json",
            std::process::id(),
            unix_ms()
        ));
        WorkspaceThreadStore::open(path).unwrap()
    }

    #[test]
    fn registers_selects_and_persists_named_threads() {
        let mut store = temp_store("lifecycle");
        let path = store.path.clone();
        let first = store.register("thread-a".into(), None, false).unwrap();
        assert!(!first.has_turns);
        store.rename("thread-a", "Mix pass").unwrap();
        store.mark_turn("thread-a").unwrap();
        store
            .register("thread-b".into(), Some("thread-a".into()), true)
            .unwrap();
        store.select("thread-a").unwrap();

        let reopened = WorkspaceThreadStore::open(path.clone()).unwrap();
        assert_eq!(reopened.selected_id(), Some("thread-a"));
        assert_eq!(reopened.len(), 2);
        let first = reopened.record("thread-a").unwrap();
        assert_eq!(first.name.as_deref(), Some("Mix pass"));
        assert!(first.has_turns);
        assert_eq!(
            reopened.record("thread-b").unwrap().forked_from_id.as_deref(),
            Some("thread-a")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_empty_names_and_unknown_selection() {
        let mut store = temp_store("validation");
        let path = store.path.clone();
        store.register("thread-a".into(), None, false).unwrap();
        assert!(store.rename("thread-a", "   ").is_err());
        assert!(store.select("missing").is_err());
        let _ = fs::remove_file(path);
    }
}
