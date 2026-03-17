.PHONY: smoke check test build clean clippy fmt publish publish-dry-run install

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

# === INSTALL ===

install:
	cargo install --path . --bin symphony

# === PUBLISH ===

# Dry-run: verify all crates can be packaged for crates.io
publish-dry-run:
	cargo publish -p symphony-core --dry-run
	cargo publish -p symphony-config --dry-run
	cargo publish -p symphony-tracker --dry-run
	cargo publish -p symphony-workspace --dry-run
	cargo publish -p symphony-agent --dry-run
	cargo publish -p symphony-orchestrator --dry-run
	cargo publish -p symphony-observability --dry-run
	cargo publish -p symphony-cli --dry-run
	@echo "PUBLISH DRY-RUN PASS"

# Publish all crates to crates.io in dependency order
publish: smoke
	cargo publish -p symphony-core
	sleep 30
	cargo publish -p symphony-config
	sleep 30
	cargo publish -p symphony-tracker
	sleep 30
	cargo publish -p symphony-workspace
	sleep 30
	cargo publish -p symphony-agent
	sleep 30
	cargo publish -p symphony-orchestrator
	sleep 30
	cargo publish -p symphony-observability
	sleep 30
	cargo publish -p symphony-cli
	@echo "PUBLISH COMPLETE"

# === CONTROL AUDIT ===

control-audit: smoke fmt-check
	@echo "CONTROL AUDIT PASS"
