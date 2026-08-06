.PHONY: check test demo fixtures validate package

check:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --workspace --all-features

fixtures:
	python3 scripts/generate_fixtures.py
	python3 scripts/reference_analysis.py
	python3 scripts/build_examples.py
	python3 scripts/mock_evaluate.py

validate:
	python3 scripts/validate_artifacts.py

demo:
	cargo run -p ghost-cli -- demo --fixture fixtures/muddy_bass.wav --intent "Tighten the low mids while preserving punch"

package:
	bash scripts/package.sh
