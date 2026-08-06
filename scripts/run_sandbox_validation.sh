#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/generate_fixtures.py
python3 scripts/reference_analysis.py
python3 scripts/build_examples.py
python3 scripts/mock_evaluate.py
python3 scripts/validate_artifacts.py
