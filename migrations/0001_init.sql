PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS plugin_binaries (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    name TEXT NOT NULL,
    vendor TEXT NOT NULL,
    version TEXT NOT NULL,
    format TEXT NOT NULL,
    path TEXT NOT NULL,
    binary_hash TEXT NOT NULL,
    scan_status TEXT NOT NULL,
    last_scanned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(format, path, binary_hash)
);

CREATE TABLE IF NOT EXISTS plugin_manifests (
    id TEXT PRIMARY KEY,
    plugin_binary_id TEXT NOT NULL REFERENCES plugin_binaries(id) ON DELETE CASCADE,
    manifest_schema_version TEXT NOT NULL,
    adapter_version TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    validated INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS ghost_instances (
    id TEXT PRIMARY KEY,
    daw_session_fingerprint TEXT,
    track_name TEXT,
    track_role TEXT,
    sample_rate REAL NOT NULL,
    channel_count INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS artifacts (
    hash TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    encoding TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS captures (
    id TEXT PRIMARY KEY,
    instance_id TEXT REFERENCES ghost_instances(id) ON DELETE SET NULL,
    source_name TEXT NOT NULL,
    sample_rate INTEGER NOT NULL,
    channels INTEGER NOT NULL,
    frames INTEGER NOT NULL,
    duration_seconds REAL NOT NULL,
    content_hash TEXT NOT NULL,
    audio_artifact_hash TEXT REFERENCES artifacts(hash),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS analysis_runs (
    id TEXT PRIMARY KEY,
    capture_id TEXT REFERENCES captures(id) ON DELETE SET NULL,
    analyzer_version TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    profile TEXT NOT NULL,
    status TEXT NOT NULL,
    analysis_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS prompt_packages (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    content_hash TEXT NOT NULL UNIQUE,
    prompt_text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS mix_requests (
    id TEXT PRIMARY KEY,
    analysis_run_id TEXT REFERENCES analysis_runs(id) ON DELETE SET NULL,
    mode TEXT NOT NULL,
    intent_json TEXT NOT NULL,
    prompt_bundle_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY,
    mix_request_id TEXT NOT NULL REFERENCES mix_requests(id) ON DELETE CASCADE,
    backend TEXT NOT NULL,
    model TEXT,
    thread_id TEXT,
    status TEXT NOT NULL,
    output_text TEXT,
    error_json TEXT,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS mix_plans (
    id TEXT PRIMARY KEY,
    agent_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    schema_version TEXT NOT NULL,
    plan_json TEXT NOT NULL,
    confidence REAL NOT NULL,
    validation_status TEXT NOT NULL,
    validation_report_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS state_snapshots (
    id TEXT PRIMARY KEY,
    instance_id TEXT REFERENCES ghost_instances(id) ON DELETE SET NULL,
    reason TEXT NOT NULL,
    outer_state_artifact_hash TEXT REFERENCES artifacts(hash),
    pro_q_state_artifact_hash TEXT REFERENCES artifacts(hash),
    pro_c_state_artifact_hash TEXT REFERENCES artifacts(hash),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS plan_applications (
    id TEXT PRIMARY KEY,
    mix_plan_id TEXT NOT NULL REFERENCES mix_plans(id) ON DELETE CASCADE,
    instance_id TEXT REFERENCES ghost_instances(id) ON DELETE SET NULL,
    pre_state_snapshot_id TEXT REFERENCES state_snapshots(id),
    post_state_snapshot_id TEXT REFERENCES state_snapshots(id),
    status TEXT NOT NULL,
    applied_at TEXT,
    reverted_at TEXT
);

CREATE TABLE IF NOT EXISTS user_decisions (
    id TEXT PRIMARY KEY,
    plan_application_id TEXT NOT NULL REFERENCES plan_applications(id) ON DELETE CASCADE,
    decision TEXT NOT NULL,
    rating INTEGER,
    reason TEXT,
    manual_changes_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_analysis_capture ON analysis_runs(capture_id);
CREATE INDEX IF NOT EXISTS idx_agent_request ON agent_runs(mix_request_id);
CREATE INDEX IF NOT EXISTS idx_plan_agent_run ON mix_plans(agent_run_id);
CREATE INDEX IF NOT EXISTS idx_decision_application ON user_decisions(plan_application_id);
