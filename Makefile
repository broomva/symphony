.PHONY: smoke check test build clean clippy fmt

# === GATES ===

# Primary gate: must pass before any commit
smoke: check test
	@echo "SMOKE PASS"

# === SENSORS ===

check:
	cargo check --workspace
	cargo clippy --workspace -- -D warnings

test:
	cargo test --workspace

# === BUILD ===

build:
	cargo build --release

# === UTILITIES ===

clippy:
	cargo clippy --workspace -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clean:
	cargo clean

# === CONTROL AUDIT ===

control-audit: smoke fmt-check
	@echo "CONTROL AUDIT PASS"
