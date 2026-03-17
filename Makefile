.PHONY: smoke check test build clean clippy fmt publish publish-dry-run install changelog release harness-audit entropy-check control-refresh control-validate conversations eval-run eval-check eval-rollback

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

# === RELEASE ===

# Generate/update CHANGELOG.md from conventional commits
changelog:
	git-cliff --output CHANGELOG.md
	@echo "CHANGELOG.md updated"

# Release: bump version, generate changelog, commit, tag, push
# Usage: make release VERSION=0.3.0
release: smoke
	@if [ -z "$(VERSION)" ]; then echo "Usage: make release VERSION=0.3.0"; exit 1; fi
	@echo "Releasing v$(VERSION)..."
	sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml && rm -f Cargo.toml.bak
	cargo check --workspace
	git-cliff --tag "v$(VERSION)" --output CHANGELOG.md
	git add Cargo.toml Cargo.lock CHANGELOG.md
	git commit -m "chore(release): v$(VERSION)"
	git tag "v$(VERSION)"
	@echo ""
	@echo "Release v$(VERSION) ready. Push with:"
	@echo "  git push origin master v$(VERSION)"

# === CONTROL AUDIT ===

control-audit: smoke fmt-check
	@echo "CONTROL AUDIT PASS"

# === HARNESS ===

harness-audit:
	bash scripts/harness/audit_harness.sh

entropy-check:
	bash scripts/harness/entropy_check.sh

# === CONTROL ===

control-refresh:
	bash scripts/control/refresh_state.sh

control-validate:
	bash scripts/control/validate_policy.sh

# === MEMORY ===

conversations:
	python3 scripts/conversation-history.py

# === EVALS ===

eval-run:
	bash evals/symphony-prompts/run_eval.sh

eval-check:
	bash evals/symphony-prompts/constraint-check.sh

eval-rollback:
	bash evals/symphony-prompts/rollback.sh
