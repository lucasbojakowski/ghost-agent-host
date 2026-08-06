#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import json

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "artifacts" / "examples"
OUT.mkdir(parents=True, exist_ok=True)
analysis = json.loads((ROOT / "artifacts/reference-analysis/muddy_bass.json").read_text())

plan = {
    "schema_version": "ghost.mix-plan/1",
    "summary": "Control persistent low-mid buildup while preserving the kick and bass onset.",
    "confidence": 0.82,
    "assumptions": ["The twelve-second capture represents the intended bass section."],
    "operations": [
        {
            "operation": "eq_band",
            "settings": {
                "band_id": "low-mid-control",
                "enabled": True,
                "shape": "bell",
                "frequency_hz": 230.0,
                "gain_db": -2.2,
                "q": 1.1,
                "slope_db_oct": None,
                "channel_mode": "stereo",
                "dynamic": {"enabled": True, "range_db": -1.5, "threshold_db": None},
                "rationale": "The low-mid band is materially elevated relative to the synthetic reference.",
                "evidence": [
                    f"low_mid_db={analysis['spectrum']['bands']['low_mid_db']:.3f}",
                    "fixture_manifest=excess persistent 230 Hz low-mid energy"
                ]
            }
        },
        {
            "operation": "compressor",
            "settings": {
                "enabled": True,
                "style": "clean",
                "threshold_db": -18.0,
                "ratio": 1.8,
                "knee_db": 8.0,
                "attack_ms": 28.0,
                "release_ms": 150.0,
                "range_db": 2.5,
                "mix_percent": 65.0,
                "output_gain_db": 0.0,
                "rationale": "Stabilize sustained event level without removing the leading transient.",
                "evidence": [f"crest_factor_db={analysis['loudness_proxy']['crest_factor_db']:.3f}"]
            }
        }
    ],
    "expected_changes": [
        {"metric": "spectrum.bands.low_mid_db", "direction": "decrease", "maximum_delta": 4.0, "unit": "dB"},
        {"metric": "loudness.crest_factor_db", "direction": "decrease", "maximum_delta": 2.5, "unit": "dB"}
    ],
    "cautions": ["Confirm the result with level-matched A/B in the full mix."]
}

intent = {
    "mode": "structured",
    "context": {
        "source": "bass",
        "role": "rhythmic anchor",
        "style": "house",
        "goal": "tighter and more controlled",
        "problem": "low-mid buildup",
        "intensity": "moderate",
        "preserve": ["initial transient", "sub weight"],
        "scope": ["eq", "compression"],
        "notes": "Do not make the bass smaller."
    }
}
capabilities = [
    {
        "plugin": "FabFilter Pro-Q 4",
        "version": "runtime-manifest",
        "supported_operations": ["static bell EQ", "dynamic bell EQ", "stereo placement"],
        "safety_notes": ["Runtime adapter resolves semantic operations to public parameters."]
    },
    {
        "plugin": "FabFilter Pro-C 3",
        "version": "runtime-manifest",
        "supported_operations": ["threshold", "ratio", "knee", "attack", "release", "range", "mix", "output"],
        "safety_notes": ["Style name must appear in the runtime manifest."]
    }
]
prompt_bundle = {
    "schema_version": "ghost.prompt-bundle/1",
    "system_prompt": (ROOT / "prompts/system.md").read_text(),
    "user_intent": intent,
    "analysis_text_json": json.dumps(analysis, indent=2),
    "capability_text_json": json.dumps(capabilities, indent=2),
    "output_contract": "Return exactly one JSON object conforming to ghost.mix-plan/1. No markdown, plots, images, binary data, raw CLAP parameter IDs, filesystem operations, or code."
}

(OUT / "mix_plan.json").write_text(json.dumps(plan, indent=2) + "\n")
(OUT / "prompt_bundle.json").write_text(json.dumps(prompt_bundle, indent=2) + "\n")
print(f"wrote examples to {OUT}")
