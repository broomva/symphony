---
tags:
  - symphony
  - architecture
  - plugins
type: documentation
status: active
area: project
created: 2026-03-16
---

# Extending Symphony

Symphony is designed around two primary extension points: **tracker adapters** and **agent runners**. This guide covers how to add support for new issue trackers and coding agents.

## Adding a New Tracker

### The TrackerClient Trait

All trackers implement the `TrackerClient` trait from `symphony-tracker`:

```rust
#[async_trait]
pub trait TrackerClient: Send + Sync {
    /// Fetch issues in configured active states for the project.
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>, TrackerError>;

    /// Fetch issues in the given states (e.g., terminal states for cleanup).
    async fn fetch_issues_by_states(
        &self,
        states: &[String],
    ) -> Result<Vec<Issue>, TrackerError>;

    /// Fetch current states for specific issue IDs (reconciliation).
    async fn fetch_issue_states_by_ids(
        &self,
        issue_ids: &[String],
    ) -> Result<Vec<Issue>, TrackerError>;
}
```

### Implementation Steps

1. **Create a module** in `crates/symphony-tracker/src/` (e.g., `github.rs`)

2. **Implement the trait** with your tracker's API:

```rust
pub struct GitHubClient {
    token: String,
    repo: String,
    // ...
}

#[async_trait]
impl TrackerClient for GitHubClient {
    async fn fetch_candidate_issues(&self) -> Result<Vec<Issue>, TrackerError> {
        // Query your tracker API
        // Return normalized Issue structs
    }
    // ... implement all three methods
}
```

3. **Normalize issues** to the `Issue` struct (from `symphony-core`):

```rust
pub struct Issue {
    pub id: String,            // Unique ID from the tracker
    pub identifier: String,    // Human-readable (e.g., "GH-123")
    pub title: String,         // Issue title
    pub description: Option<String>,
    pub priority: Option<i32>, // Lower = higher priority; null sorts last
    pub state: String,         // Must match active_states/terminal_states
    pub branch_name: Option<String>,
    pub url: Option<String>,
    pub labels: Vec<String>,   // Normalized to lowercase
    pub blocked_by: Vec<BlockerRef>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

4. **Wire it up** in `src/cli/start.rs` — add a match arm for your tracker kind:

```rust
let tracker: Arc<dyn TrackerClient> = match config.tracker.kind.as_str() {
    "linear" => Arc::new(LinearClient::new(/* ... */)),
    "github" => Arc::new(GitHubClient::new(/* ... */)),
    _ => anyhow::bail!("unsupported tracker: {}", config.tracker.kind),
};
```

5. **Add tests** following the pattern in `crates/symphony-tracker/src/linear/tests.rs`

### Key Requirements

- **State normalization**: Always use `trim().to_lowercase()` when comparing states
- **Pagination**: Handle multi-page results (the orchestrator expects a complete list)
- **Error mapping**: Map API errors to `TrackerError` variants
- **Empty inputs**: `fetch_issue_states_by_ids(&[])` must return immediately without an API call

### WORKFLOW.md Configuration

Users configure your tracker in the WORKFLOW.md front matter:

```yaml
tracker:
  kind: github              # Your tracker kind identifier
  api_key: $GITHUB_TOKEN    # $VAR references are resolved from environment
  project_slug: org/repo    # Tracker-specific project identifier
  active_states:
    - open
  terminal_states:
    - closed
```

## Adding a New Agent Runner

### Current Architecture

`AgentRunner` in `symphony-agent` supports two modes:

1. **JSON-RPC mode** (`run_session`) — for agents that speak the Codex app-server protocol
2. **Simple mode** (`run_simple_session`) — for CLI tools that accept stdin prompts

### Adding a New Mode

To support a new agent protocol:

1. Add a new method to `AgentRunner` (e.g., `run_custom_session`)
2. Handle the agent's specific I/O protocol
3. Emit `AgentEvent` callbacks for observability
4. Return an `AgentSession` with token usage

### Configuration

Agent behavior is controlled via the `codex` section in WORKFLOW.md:

```yaml
codex:
  command: "your-agent-command"   # The executable to run
  approval_policy: "auto-edit"    # Agent-specific setting
  turn_timeout_ms: 600000         # Max time per turn
  read_timeout_ms: 5000           # Handshake timeout
  stall_timeout_ms: 300000        # Inactivity timeout (enforced by orchestrator)
```

The `command` field determines which agent binary is launched. The orchestrator automatically selects JSON-RPC mode if the command contains "app-server", otherwise uses simple mode.

## WORKFLOW.md Extension Points

The WORKFLOW.md front matter is designed for forward compatibility:

- **Unknown keys are ignored** — you can add custom sections without breaking existing code
- **`$VAR` resolution** — environment variable references work in any string field
- **`~` expansion** — home directory expansion works in path fields

To add a new configuration section:

1. Add the typed config struct to `crates/symphony-config/src/types.rs`
2. Add extraction logic in `crates/symphony-config/src/loader.rs`
3. Add validation rules in `validate_dispatch_config()` if needed
4. Document defaults in the struct's `Default` implementation

## Testing Your Extension

```bash
# Run all tests
make test

# Run tests for a specific crate
cargo test -p symphony-tracker

# Run with real API credentials (opt-in)
LINEAR_API_KEY=your-key cargo test -- --ignored
```

## See Also

- [[CONTRIBUTING]] — development workflow and conventions
- [[PLANS]] — implementation roadmap
- [[docs/architecture/Crate Map|Crate Map]] — detailed crate responsibilities
- [[CONTROL]] — quality gates and setpoints
