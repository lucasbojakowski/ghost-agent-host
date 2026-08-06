#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import json
import sqlite3
import sys
import tomllib
import wave
import soundfile as sf
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
errors: list[str] = []
checks: list[dict] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    checks.append({"name": name, "ok": ok, "detail": detail})
    if not ok:
        errors.append(f"{name}: {detail}")


# TOML syntax and workspace paths.
for path in sorted(ROOT.rglob("Cargo.toml")):
    try:
        with path.open("rb") as handle:
            tomllib.load(handle)
        record(f"toml:{path.relative_to(ROOT)}", True)
    except Exception as exc:
        record(f"toml:{path.relative_to(ROOT)}", False, str(exc))

root_cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
for member in root_cargo["workspace"]["members"]:
    record(f"workspace-member:{member}", (ROOT / member / "Cargo.toml").exists(), "Cargo.toml missing")

# SQLite migration executes and foreign keys work.
try:
    con = sqlite3.connect(":memory:")
    con.executescript((ROOT / "migrations/0001_init.sql").read_text())
    tables = {row[0] for row in con.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    required = {"captures", "analysis_runs", "mix_requests", "agent_runs", "mix_plans", "user_decisions"}
    record("sqlite-migration", required.issubset(tables), f"missing {sorted(required - tables)}")
except Exception as exc:
    record("sqlite-migration", False, str(exc))

# JSON schemas.
for schema_path in sorted((ROOT / "schemas").glob("*.schema.json")):
    try:
        schema = json.loads(schema_path.read_text())
        Draft202012Validator.check_schema(schema)
        record(f"schema:{schema_path.name}", True)
    except Exception as exc:
        record(f"schema:{schema_path.name}", False, str(exc))

example_plan_path = ROOT / "artifacts" / "examples" / "mix_plan.json"
if example_plan_path.exists():
    try:
        schema = json.loads((ROOT / "schemas/mix_plan.schema.json").read_text())
        Draft202012Validator(schema).validate(json.loads(example_plan_path.read_text()))
        record("example-mix-plan", True)
    except Exception as exc:
        record("example-mix-plan", False, str(exc))

prompt_path = ROOT / "artifacts" / "examples" / "prompt_bundle.json"
if prompt_path.exists():
    try:
        data = json.loads(prompt_path.read_text())
        schema = json.loads((ROOT / "schemas/prompt_bundle.schema.json").read_text())
        Draft202012Validator(schema).validate(data)
        forbidden = {"plot", "plots", "image", "images", "localImage", "url"}
        found = []
        def walk(value, path=""):
            if isinstance(value, dict):
                for key, child in value.items():
                    if key in forbidden:
                        found.append(f"{path}/{key}")
                    walk(child, f"{path}/{key}")
            elif isinstance(value, list):
                for i, child in enumerate(value):
                    walk(child, f"{path}/{i}")
        walk(data)
        record("prompt-bundle-text-only", not found, f"forbidden keys: {found}")
    except Exception as exc:
        record("prompt-bundle-text-only", False, str(exc))

# Fixtures and numerical expectations.
manifest_path = ROOT / "fixtures/manifest.json"
if manifest_path.exists():
    manifest = json.loads(manifest_path.read_text())
    for name in manifest["fixtures"]:
        path = ROOT / "fixtures" / name
        try:
            info = sf.info(path)
            ok = info.samplerate == manifest["sample_rate"] and abs(info.duration - manifest["duration_seconds"]) < 1e-4
            record(f"fixture:{name}", ok, str(info))
        except Exception as exc:
            record(f"fixture:{name}", False, str(exc))

summary_path = ROOT / "artifacts/reference-analysis/summary.json"
if summary_path.exists():
    summary = json.loads(summary_path.read_text())
    try:
        muddy = summary["muddy_bass.wav"]
        clean = summary["clean_reference.wav"]
        phasey = summary["phasey_wide.wav"]
        crushed = summary["crushed_drums.wav"]
        record(
            "fixture-behavior:muddy-low-mid",
            muddy["spectrum"]["bands"]["low_mid_db"] > clean["spectrum"]["bands"]["low_mid_db"] + 2.0,
            "muddy fixture should have materially more low-mid energy",
        )
        record(
            "fixture-behavior:phase-correlation",
            phasey["stereo"]["high_band_correlation"] < clean["stereo"]["high_band_correlation"] - 0.2,
            "phase fixture should reduce high-band correlation",
        )
        record(
            "fixture-behavior:crushed-crest",
            crushed["loudness_proxy"]["crest_factor_db"] < clean["loudness_proxy"]["crest_factor_db"],
            "crushed fixture should have lower crest",
        )
    except Exception as exc:
        record("fixture-behavior", False, str(exc))

# Independent mock processing expectations.
mock_eval_path = ROOT / "artifacts" / "mock-evaluation" / "evaluation.json"
if mock_eval_path.exists():
    try:
        evaluation = json.loads(mock_eval_path.read_text())
        for key, value in evaluation["expectations"].items():
            record(f"mock-evaluation:{key}", bool(value), json.dumps(evaluation["deltas"]))
    except Exception as exc:
        record("mock-evaluation", False, str(exc))

# Default analysis configuration parses and satisfies the Rust-side invariants.
try:
    config = tomllib.loads((ROOT / "config/default.toml").read_text())["analysis"]
    powers = all(isinstance(value, int) and value > 0 and value & (value - 1) == 0 for value in config["fft_sizes"])
    valid = powers and 0.03125 <= config["hop_ratio"] <= 1.0 and config["maximum_frequency_hz"] > config["minimum_frequency_hz"]
    record("analysis-config", valid, json.dumps(config))
except Exception as exc:
    record("analysis-config", False, str(exc))

# Cargo path dependencies and include_str! assets exist.
for cargo_path in sorted(ROOT.rglob("Cargo.toml")):
    try:
        cargo = tomllib.loads(cargo_path.read_text())
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, spec in cargo.get(section, {}).items():
                if isinstance(spec, dict) and "path" in spec:
                    target = (cargo_path.parent / spec["path"] / "Cargo.toml").resolve()
                    record(f"path-dependency:{cargo_path.relative_to(ROOT)}:{name}", target.exists(), str(target))
    except Exception as exc:
        record(f"path-dependency:{cargo_path.relative_to(ROOT)}", False, str(exc))

import re
for path in sorted(ROOT.rglob("*.rs")):
    text = path.read_text()
    for match in re.finditer(r'include_str!\(\s*"([^"]+)"\s*\)', text):
        target = (path.parent / match.group(1)).resolve()
        record(f"include-str:{path.relative_to(ROOT)}:{match.group(1)}", target.exists(), str(target))

# Lightweight delimiter scanner. It skips Rust comments and ordinary/raw strings.
def balanced_rust(text: str):
    stack = []
    pairs = {')': '(', ']': '[', '}': '{'}
    i = 0
    state = 'code'
    block_depth = 0
    raw_hashes = 0
    while i < len(text):
        c = text[i]
        n = text[i + 1] if i + 1 < len(text) else ''
        if state == 'line':
            if c == '\n': state = 'code'
            i += 1; continue
        if state == 'block':
            if c == '/' and n == '*': block_depth += 1; i += 2; continue
            if c == '*' and n == '/':
                block_depth -= 1; i += 2
                if block_depth == 0: state = 'code'
                continue
            i += 1; continue
        if state == 'string':
            if c == '\\': i += 2; continue
            if c == '"': state = 'code'
            i += 1; continue
        if state == 'raw':
            if c == '"' and text.startswith('#' * raw_hashes, i + 1):
                i += 1 + raw_hashes; state = 'code'
            else: i += 1
            continue
        if c == '/' and n == '/': state = 'line'; i += 2; continue
        if c == '/' and n == '*': state = 'block'; block_depth = 1; i += 2; continue
        # r"...", r#"..."#, br#"..."#
        raw_start = None
        if c == 'r': raw_start = i + 1
        elif c == 'b' and n == 'r': raw_start = i + 2
        if raw_start is not None:
            j = raw_start
            while j < len(text) and text[j] == '#': j += 1
            if j < len(text) and text[j] == '"':
                raw_hashes = j - raw_start; state = 'raw'; i = j + 1; continue
        if c == '"': state = 'string'; i += 1; continue
        if c in '([{': stack.append((c, i))
        elif c in pairs:
            if not stack or stack[-1][0] != pairs[c]: return False, f"mismatch at {i}"
            stack.pop()
        i += 1
    if state in ('string', 'raw', 'block'): return False, f"unterminated {state}"
    return (not stack), (f"unclosed {stack[-1]}" if stack else '')

# Source hygiene checks possible without a Rust compiler.
for path in sorted(ROOT.rglob("*.rs")):
    text = path.read_text()
    bad = [token for token in ["todo!", "unimplemented!", "panic!(\"TODO"] if token in text]
    record(f"source-hygiene:{path.relative_to(ROOT)}", not bad, f"found {bad}")
    ok, detail = balanced_rust(text)
    record(f"source-delimiters:{path.relative_to(ROOT)}", ok, detail)

report = {
    "schema_version": "ghost.sandbox-validation/1",
    "rust_compiler_available": False,
    "checks": checks,
    "passed": sum(1 for item in checks if item["ok"]),
    "failed": sum(1 for item in checks if not item["ok"]),
    "limitations": [
        "The sandbox did not contain rustc/cargo and blocked external toolchain downloads.",
        "FabFilter binaries and a CLAP-capable DAW were not available.",
        "The real nested-child process callback, child GUI parenting, parameter manifest, latency, and state round-trip require target-machine integration tests."
    ],
}
(ROOT / "artifacts/sandbox-validation.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
if errors:
    sys.exit(1)
