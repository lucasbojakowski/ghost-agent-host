use std::fs;
use std::path::{Path, PathBuf};

use ghost_core::{AnalysisBundle, MixPlan, PromptBundle, UserIntent};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

const MIGRATION_1: &str = include_str!("../../../migrations/0001_init.sql");

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct GhostDatabase {
    connection: Connection,
    artifact_root: PathBuf,
}

impl GhostDatabase {
    pub fn open(path: impl AsRef<Path>, artifact_root: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(artifact_root.as_ref())?;
        let connection = Connection::open(path)?;
        Self::configure(&connection)?;
        let database = Self {
            connection,
            artifact_root: artifact_root.as_ref().to_path_buf(),
        };
        database.migrate()?;
        Ok(database)
    }

    pub fn in_memory() -> Result<Self, DatabaseError> {
        let root = std::env::temp_dir().join(format!("ghost-db-{}", Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let connection = Connection::open_in_memory()?;
        Self::configure(&connection)?;
        let database = Self {
            connection,
            artifact_root: root,
        };
        database.migrate()?;
        Ok(database)
    }

    fn configure(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;\nPRAGMA synchronous=NORMAL;\nPRAGMA foreign_keys=ON;",
        )?;
        Ok(())
    }

    fn migrate(&self) -> Result<(), DatabaseError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);",
        )?;
        let applied: Option<String> = self
            .connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let checksum = blake3::hash(MIGRATION_1.as_bytes()).to_hex().to_string();
        if let Some(existing) = applied {
            if existing != checksum {
                return Err(DatabaseError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "migration checksum mismatch",
                )));
            }
            return Ok(());
        }
        self.connection.execute_batch(MIGRATION_1)?;
        self.connection.execute(
            "INSERT INTO schema_migrations(version, checksum) VALUES(1, ?1)",
            params![checksum],
        )?;
        Ok(())
    }

    pub fn store_analysis(&self, bundle: &AnalysisBundle) -> Result<Uuid, DatabaseError> {
        let capture_id = bundle.capture.capture_id;
        self.connection.execute(
            "INSERT OR REPLACE INTO captures(id, source_name, sample_rate, channels, frames, duration_seconds, content_hash) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                capture_id.to_string(),
                &bundle.capture.source_name,
                bundle.capture.sample_rate,
                bundle.capture.channels as i64,
                bundle.capture.frames as i64,
                bundle.capture.duration_seconds,
                &bundle.capture.content_hash,
            ],
        )?;
        let run_id = Uuid::new_v4();
        self.connection.execute(
            "INSERT INTO analysis_runs(id, capture_id, analyzer_version, schema_version, profile, status, analysis_json) VALUES(?1, ?2, ?3, ?4, ?5, 'complete', ?6)",
            params![
                run_id.to_string(),
                capture_id.to_string(),
                &bundle.analyzer_version,
                &bundle.schema_version,
                format!("{:?}", bundle.configuration.profile).to_lowercase(),
                serde_json::to_string(bundle)?,
            ],
        )?;
        Ok(run_id)
    }

    pub fn store_mix_request(
        &self,
        analysis_run_id: Uuid,
        intent: &UserIntent,
        bundle: &PromptBundle,
    ) -> Result<Uuid, DatabaseError> {
        let request_id = Uuid::new_v4();
        let mode = match intent {
            UserIntent::Freeform { .. } => "freeform",
            UserIntent::Structured { .. } => "structured",
        };
        self.connection.execute(
            "INSERT INTO mix_requests(id, analysis_run_id, mode, intent_json, prompt_bundle_json) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![
                request_id.to_string(),
                analysis_run_id.to_string(),
                mode,
                serde_json::to_string(intent)?,
                serde_json::to_string(bundle)?,
            ],
        )?;
        Ok(request_id)
    }

    pub fn begin_agent_run(
        &self,
        request_id: Uuid,
        backend: &str,
        model: Option<&str>,
    ) -> Result<Uuid, DatabaseError> {
        let id = Uuid::new_v4();
        self.connection.execute(
            "INSERT INTO agent_runs(id, mix_request_id, backend, model, status) VALUES(?1, ?2, ?3, ?4, 'running')",
            params![id.to_string(), request_id.to_string(), backend, model],
        )?;
        Ok(id)
    }

    pub fn complete_agent_run(
        &self,
        run_id: Uuid,
        output: &str,
    ) -> Result<(), DatabaseError> {
        self.connection.execute(
            "UPDATE agent_runs SET status='complete', output_text=?2, completed_at=CURRENT_TIMESTAMP WHERE id=?1",
            params![run_id.to_string(), output],
        )?;
        Ok(())
    }

    pub fn store_mix_plan(
        &self,
        agent_run_id: Uuid,
        plan: &MixPlan,
        validation_status: &str,
        validation_report: impl Serialize,
    ) -> Result<Uuid, DatabaseError> {
        let id = Uuid::new_v4();
        self.connection.execute(
            "INSERT INTO mix_plans(id, agent_run_id, schema_version, plan_json, confidence, validation_status, validation_report_json) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.to_string(),
                agent_run_id.to_string(),
                &plan.schema_version,
                serde_json::to_string(plan)?,
                plan.confidence,
                validation_status,
                serde_json::to_string(&validation_report)?,
            ],
        )?;
        Ok(id)
    }

    pub fn put_artifact(
        &self,
        kind: &str,
        bytes: &[u8],
        extension: &str,
    ) -> Result<String, DatabaseError> {
        let hash = blake3::hash(bytes).to_hex().to_string();
        let shard = &hash[..2];
        let directory = self.artifact_root.join(kind).join(shard);
        fs::create_dir_all(&directory)?;
        let relative = PathBuf::from(kind).join(shard).join(format!("{hash}.{extension}"));
        let target = self.artifact_root.join(&relative);
        if !target.exists() {
            let temporary = target.with_extension(format!("{extension}.tmp"));
            fs::write(&temporary, bytes)?;
            fs::rename(temporary, &target)?;
        }
        self.connection.execute(
            "INSERT OR IGNORE INTO artifacts(hash, kind, relative_path, size_bytes, encoding) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![&hash, kind, relative.to_string_lossy(), bytes.len() as i64, extension],
        )?;
        Ok(hash)
    }

    pub fn counts(&self) -> Result<DatabaseCounts, DatabaseError> {
        Ok(DatabaseCounts {
            captures: self.table_count("captures")?,
            analysis_runs: self.table_count("analysis_runs")?,
            mix_requests: self.table_count("mix_requests")?,
            agent_runs: self.table_count("agent_runs")?,
            mix_plans: self.table_count("mix_plans")?,
        })
    }

    fn table_count(&self, table: &str) -> Result<i64, DatabaseError> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        Ok(self.connection.query_row(&sql, [], |row| row.get(0))?)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseCounts {
    pub captures: i64,
    pub analysis_runs: i64,
    pub mix_requests: i64,
    pub agent_runs: i64,
    pub mix_plans: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_idempotent() {
        let database = GhostDatabase::in_memory().unwrap();
        database.migrate().unwrap();
        assert_eq!(database.counts().unwrap().captures, 0);
    }

    #[test]
    fn artifacts_are_content_addressed() {
        let database = GhostDatabase::in_memory().unwrap();
        let first = database.put_artifact("test", b"same", "bin").unwrap();
        let second = database.put_artifact("test", b"same", "bin").unwrap();
        assert_eq!(first, second);
    }
}
