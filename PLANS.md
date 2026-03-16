---
tags:
  - symphony
  - roadmap
  - planning
type: roadmap
status: active
area: project
aliases:
  - Plans
  - Implementation Roadmap
created: 2026-03-06
---

# PLANS.md - Symphony Implementation Roadmap

Each task references [[SPEC]] sections. Acceptance criteria are testable assertions verified by [[CONTROL]] setpoints. See [[.planning/ROADMAP|Roadmap Graph]] for the phase dependency diagram and [[docs/roadmap/Project Status|Project Status]] for current completion state.

Phases are ordered by dependency: each phase only depends on prior phases.

---

## Phase 0: Scaffold [DONE]
**Depends on**: nothing
**Gate**: `make smoke` passes

- [x] Rust workspace with 7 crates matching Spec S3.2 abstraction levels
- [x] Core domain model: Issue (S4.1.1), Workspace (S4.1.4), RunAttempt (S4.1.5), LiveSession (S4.1.6), RetryEntry (S4.1.7), OrchestratorState (S4.1.8)
- [x] Stable identifiers and normalization rules (S4.2): workspace_key sanitization, state normalization, session_id composition
- [x] Config layer stub: WORKFLOW.md loader (S5.1-5.2), typed config with defaults (S6.4)
- [x] Tracker trait with 3 required operations (S11.1) + Linear client stub
- [x] Workspace manager stub with safety invariants (S9.5)
- [x] Agent runner protocol types (S10.4) + runner stub
- [x] Orchestrator: dispatch eligibility (S8.2), sort (S8.2), backoff formula (S8.4), reconciliation helpers (S8.5)
- [x] Observability: tracing JSON init (S13.1-13.2), Axum HTTP skeleton (S13.7)
- [x] CLI entry point: positional workflow path, --port flag (S17.7)
- [x] 25 passing tests, `make smoke` green
- [x] Harness artifacts: AGENTS.md, CLAUDE.md, Makefile, CONTROL.md

---

## Phase 1: Config & Workflow [NEXT]
**Depends on**: Phase 0
**Gate**: All Spec S17.1 test cases pass

### Tasks

**1.1 — Complete WORKFLOW.md parsing (S5.1-5.2)**
- File path precedence: explicit runtime path > cwd default `WORKFLOW.md` (S5.1)
- Front matter parsing: `---` delimited YAML, non-map = error `workflow_front_matter_not_a_map` (S5.2)
- Prompt body: trimmed markdown after front matter (S5.2)
- Missing file = `missing_workflow_file` error (S5.1)
- AC: `load_workflow("/nonexistent")` returns `MissingFile`
- AC: Non-map YAML front matter returns `FrontMatterNotMap`

**1.2 — Full front matter extraction to ServiceConfig (S5.3)**
- `tracker` section: kind, endpoint, api_key (with `$VAR` indirection), project_slug, active_states (list or CSV), terminal_states (S5.3.1)
- `polling` section: interval_ms with string-integer coercion (S5.3.2)
- `workspace` section: root with `~` expansion and `$VAR` resolution (S5.3.3)
- `hooks` section: after_create, before_run, after_run, before_remove, timeout_ms (non-positive = default 60000) (S5.3.4)
- `agent` section: max_concurrent_agents, max_turns, max_retry_backoff_ms, max_concurrent_agents_by_state (normalized keys, ignore invalid) (S5.3.5)
- `codex` section: command, approval_policy, thread_sandbox, turn_sandbox_policy, turn_timeout_ms, read_timeout_ms, stall_timeout_ms (S5.3.6)
- `server.port` extension (S5.3 note)
- Unknown keys ignored for forward compatibility (S5.3)
- AC: `extract_config` with full front matter produces correct typed ServiceConfig for every field
- AC: `$VAR` resolving empty env returns empty string (treated as missing)
- AC: Per-state concurrency map normalizes keys via trim+lowercase, ignores non-positive/non-numeric

**1.3 — Liquid template engine for prompt rendering (S5.4, S12)**
- Integrate `liquid` crate for strict template rendering
- Template input variables: `issue` (all normalized fields incl. labels, blockers), `attempt` (integer or null) (S5.4)
- Strict mode: unknown variables fail, unknown filters fail (S5.4)
- Empty prompt body → fallback prompt "You are working on an issue from Linear." (S5.4)
- Issue object keys converted to strings for template compatibility (S12.2)
- Preserve nested arrays/maps (labels, blockers) so templates can iterate (S12.2)
- AC: Template with `{{ issue.identifier }}` renders correctly
- AC: Template with `{{ unknown_var }}` returns `template_render_error`
- AC: Template with `{{ issue.labels | size }}` works (nested array access)
- AC: `attempt` is `null` on first run, integer on retry

**1.4 — Dispatch preflight validation (S6.3)**
- Startup validation: fail startup if invalid
- Per-tick validation: skip dispatch for that tick, keep reconciliation, emit error
- Validation checks: workflow loadable, `tracker.kind` present+supported, `tracker.api_key` present after `$` resolution, `tracker.project_slug` present for linear, `codex.command` non-empty
- AC: Missing `tracker.kind` → validation error
- AC: Unsupported `tracker.kind` → validation error
- AC: Empty `api_key` after `$VAR` resolution → validation error
- AC: Empty `codex.command` → validation error

**1.5 — Dynamic reload via file watcher (S6.2)**
- Watch WORKFLOW.md for changes (create/modify/remove events)
- On change: re-read, re-parse, re-apply config + prompt template
- Invalid reload: keep last known good config, emit operator-visible error (S6.2)
- Applied to: polling cadence, concurrency limits, active/terminal states, codex settings, workspace paths/hooks, prompt content for future runs (S6.2)
- In-flight sessions NOT automatically restarted (S6.2)
- Re-validate defensively before dispatch in case watch events missed (S6.2)
- AC: Modifying WORKFLOW.md changes effective poll interval without restart
- AC: Invalid YAML change does not crash; keeps last good config

**1.6 — Error surface for workflow/config (S5.5)**
- Error classes: `missing_workflow_file`, `workflow_parse_error`, `workflow_front_matter_not_a_map`, `template_parse_error`, `template_render_error`
- Dispatch gating: file/YAML errors block dispatch; template errors fail only the affected run
- AC: Each error class is a distinct variant in the error type

---

## Phase 2: Linear Tracker Client
**Depends on**: Phase 1 (needs config types)
**Gate**: All Spec S17.3 test cases pass

### Tasks

**2.1 — GraphQL query for candidate issues (S11.2)**
- Query filters: `project: { slugId: { eq: $projectSlug } }` and active state filter
- Fetch all normalized Issue fields (S4.1.1)
- Page size default: 50
- Network timeout: 30000 ms
- AC: Query uses exact `slugId` filter field
- AC: Response parsed into Vec<Issue>

**2.2 — Paginated candidate fetch (S11.2)**
- Follow `endCursor` for multi-page results
- Preserve sort order across pages
- Missing `endCursor` when `hasNextPage=true` → `linear_missing_end_cursor` error
- AC: Pagination produces stable ordered results
- AC: Missing cursor produces specific error

**2.3 — Issue state refresh by IDs (S11.1 op 3, S11.2)**
- Query uses GraphQL ID typing `[ID!]` (S11.2)
- Returns minimal normalized issues (id, identifier, state at minimum)
- Empty input returns empty without API call
- AC: Empty ID list → no API call, empty result
- AC: Query variable type is `[ID!]`

**2.4 — Terminal-state fetch for startup cleanup (S11.1 op 2)**
- Fetch issues in configured terminal states for the project
- Used by orchestrator during startup (S8.6)
- AC: Returns issues in terminal states

**2.5 — Issue normalization (S11.3)**
- `labels` → lowercase strings
- `blocked_by` → derived from inverse relations where relation type is `blocks`
- `priority` → integer only (non-integers become null)
- `created_at`, `updated_at` → parse ISO-8601 timestamps
- AC: Label "BUG" normalized to "bug"
- AC: Non-integer priority becomes None
- AC: Blocker derived from inverse "blocks" relation

**2.6 — Error mapping (S11.4)**
- Error categories: `unsupported_tracker_kind`, `missing_tracker_api_key`, `missing_tracker_project_slug`, `linear_api_request`, `linear_api_status`, `linear_graphql_errors`, `linear_unknown_payload`, `linear_missing_end_cursor`
- Orchestrator behavior: candidate fetch failure → skip dispatch; state refresh failure → keep workers; terminal cleanup failure → log + continue (S11.4)
- AC: Each error category maps to a distinct TrackerError variant

---

## Phase 3: Workspace Manager
**Depends on**: Phase 1 (needs config types)
**Gate**: All Spec S17.2 test cases pass

### Tasks

**3.1 — Full workspace creation/reuse lifecycle (S9.1-9.2)**
- Workspace path: `<workspace.root>/<sanitized_issue_identifier>` (S9.1)
- Sanitize identifier: replace `[^A-Za-z0-9._-]` with `_` (S4.2)
- Existing directory → reuse, `created_now=false` (S9.2)
- New directory → create, `created_now=true` (S9.2)
- Existing non-directory at path → handle safely (S17.2)
- AC: Same identifier always produces same workspace path
- AC: Existing dir reused with `created_now=false`
- AC: New dir created with `created_now=true`

**3.2 — Hook execution with correct failure semantics (S9.4)**
- Execute via `sh -lc <script>` with workspace as `cwd` (S9.4)
- `after_create`: runs only on new workspace; failure = fatal to creation (S9.4)
- `before_run`: runs before each attempt; failure/timeout = abort attempt (S9.4)
- `after_run`: runs after each attempt; failure/timeout = logged and ignored (S9.4)
- `before_remove`: runs on cleanup if dir exists; failure/timeout = logged and ignored (S9.4)
- Hook timeout: `hooks.timeout_ms` (default 60000) (S9.4)
- Non-positive timeout → use default (S5.3.4)
- AC: `after_create` failure removes partial workspace dir
- AC: `before_run` failure returns error, aborts attempt
- AC: `after_run` failure is logged but does not propagate
- AC: Hook exceeding timeout produces HookTimeout error

**3.3 — Path containment safety invariant (S9.5)**
- Invariant 1: Coding agent cwd must equal per-issue workspace path (S9.5)
- Invariant 2: Normalize both paths to absolute; `workspace_path` must have `workspace_root` as prefix (S9.5)
- Invariant 3: Workspace key contains only `[A-Za-z0-9._-]` (S9.5)
- AC: Workspace path outside root → error
- AC: Traversal attack `../etc` → sanitized to `_.._etc`, stays under root

**3.4 — Workspace cleanup for terminal issues (S8.6, S8.5)**
- Startup cleanup: remove workspaces for terminal-state issues (S8.6)
- Active-run reconciliation: clean workspace when issue transitions to terminal (S8.5)
- Run `before_remove` hook before deletion (S9.4)
- AC: Terminal issue workspace removed on startup
- AC: Running issue going terminal → workspace cleaned

**3.5 — Filesystem safety (S15.2)**
- Workspace path must remain under configured workspace root
- Coding-agent cwd must be per-issue workspace path
- Directory names use sanitized identifiers
- AC: All path operations validated before execution

---

## Phase 4: Orchestrator Core
**Depends on**: Phases 1, 2, 3
**Gate**: All Spec S17.4 test cases pass

### Tasks

**4.1 — Implement startup sequence (Spec Algorithm 16.1)**
- `start_service()`: configure logging → start observability → start workflow watch → init state → validate config → startup terminal cleanup → schedule first tick → event loop
- AC: Invalid startup config → fail startup with error
- AC: Terminal cleanup runs before first dispatch tick

**4.2 — Implement poll-and-dispatch tick (Spec Algorithm 16.2)**
- `on_tick()`: reconcile first → validate config → fetch candidates → sort → dispatch while slots remain → notify observers → schedule next tick
- If validation fails: skip dispatch, keep reconciliation, schedule next tick (S8.1)
- If candidate fetch fails: log, skip dispatch, schedule next tick (S8.1)
- AC: Reconciliation runs before dispatch every tick
- AC: Validation failure skips dispatch but does not crash

**4.3 — Candidate selection with all eligibility rules (S8.2)**
- Required fields: `id`, `identifier`, `title`, `state` (S8.2)
- State in `active_states` AND not in `terminal_states` (S8.2)
- Not in `running` map AND not in `claimed` set (S8.2)
- Global concurrency slots available (S8.3)
- Per-state concurrency slots available (S8.3)
- Blocker rule: Todo state with non-terminal blocker → not eligible (S8.2)
- Todo state with all terminal blockers → eligible (S8.2)
- AC: Issue missing `title` → not eligible
- AC: Todo with non-terminal blocker → not eligible
- AC: Todo with all-terminal blockers → eligible

**4.4 — Dispatch sorting (S8.2)**
- Sort order: `priority` ascending (null sorts last) → `created_at` oldest first → `identifier` lexicographic
- AC: Priority 1 dispatches before priority 3
- AC: Null priority dispatches after any numeric priority
- AC: Same priority sorted by oldest creation date

**4.5 — Concurrency control (S8.3)**
- Global: `available_slots = max(max_concurrent_agents - running_count, 0)` (S8.3)
- Per-state: `max_concurrent_agents_by_state[normalized_state]` if present, else global (S8.3)
- State key normalized via trim+lowercase (S8.3)
- AC: Global limit 2 with 2 running → no dispatch
- AC: Per-state limit 1 for "todo" with 1 running "todo" → no dispatch for todo

**4.6 — Dispatch one issue (Spec Algorithm 16.4)**
- `dispatch_issue()`: spawn worker → create running entry with all fields → add to claimed set → remove from retry_attempts
- Worker spawn failure → schedule retry (S16.4)
- AC: Dispatched issue appears in `running` and `claimed`
- AC: Previous retry entry removed on dispatch

**4.7 — Worker attempt lifecycle (Spec Algorithm 16.5)**
- `run_agent_attempt()`: create workspace → run before_run hook → start session → multi-turn loop → stop session → after_run hook
- Multi-turn: after each turn completion, re-check tracker state; if still active and under max_turns, start another turn on same thread (S7.1)
- First turn: full rendered task prompt (S7.1)
- Continuation turns: continuation guidance only (not re-send original prompt) (S7.1)
- AC: Worker loops turns while issue stays active and under max_turns
- AC: First turn uses full prompt; continuation uses guidance

**4.8 — Retry queue with exponential backoff (S8.4)**
- Cancel existing retry timer for same issue before creating new entry (S8.4)
- Continuation retry (normal exit): fixed 1000 ms delay, attempt=1 (S8.4)
- Failure retry: `min(10000 * 2^(attempt-1), max_retry_backoff_ms)` (S8.4)
- Power capped by max_retry_backoff_ms (default 300000 / 5m) (S8.4)
- AC: Normal exit → 1000ms delay, attempt=1
- AC: Failure attempt=1 → 10000ms; attempt=2 → 20000ms; attempt=3 → 40000ms
- AC: Attempt=10 → capped at 300000ms

**4.9 — Retry timer handling (Spec Algorithm 16.6)**
- `on_retry_timer()`: pop retry entry → fetch candidates → find issue → dispatch or release claim
- Issue not found in candidates → release claim (S8.4)
- Issue found but no slots → requeue with "no available orchestrator slots" error (S8.4)
- Issue found and slots available → dispatch (S8.4)
- AC: Retry for non-existent issue releases claim
- AC: Retry with no slots requeues with specific error

**4.10 — Worker exit handling (Spec Algorithm 16.6)**
- `on_worker_exit()`: remove running entry → add runtime to totals → schedule retry
- Normal exit → completed set + continuation retry (attempt 1) (S16.6)
- Abnormal exit → exponential backoff retry (S16.6)
- AC: Normal exit adds to completed set and schedules 1s retry
- AC: Abnormal exit schedules exponential retry

**4.11 — Active run reconciliation (Spec Algorithm 16.3, S8.5)**
- Part A — Stall detection (S8.5):
  - Compute elapsed since `last_codex_timestamp` (or `started_at` if no events) (S8.5)
  - If elapsed > `stall_timeout_ms` → kill worker, queue retry (S8.5)
  - If `stall_timeout_ms <= 0` → skip stall detection entirely (S8.5)
- Part B — Tracker state refresh (S8.5):
  - Fetch current states for all running issue IDs (S8.5)
  - Terminal state → terminate worker + clean workspace (S8.5)
  - Still active → update in-memory issue snapshot (S8.5)
  - Neither active nor terminal → terminate worker without cleanup (S8.5)
  - Refresh failure → keep workers, retry next tick (S8.5)
- AC: Stalled session gets killed and retried
- AC: Stall timeout ≤ 0 disables detection
- AC: Terminal transition triggers workspace cleanup
- AC: Refresh failure keeps workers running

**4.12 — Startup terminal workspace cleanup (S8.6)**
- Query tracker for terminal-state issues (S8.6)
- Remove workspace for each returned identifier (S8.6)
- Terminal fetch failure → log warning, continue startup (S8.6)
- AC: Stale terminal workspaces removed on startup
- AC: Fetch failure does not prevent startup

**4.13 — Failure model implementation (S14)**
- Error classes: Workflow/Config, Workspace, Agent Session, Tracker, Observability (S14.1)
- Recovery: dispatch validation failure → skip dispatch + keep service alive; worker failure → retry; tracker fetch failure → skip tick; dashboard failure → no crash (S14.2)
- Restart recovery: no timers restored, fresh poll + re-dispatch (S14.3)
- AC: Service stays alive on dispatch validation failure
- AC: Dashboard errors do not crash orchestrator

---

## Phase 5: Agent Runner (Codex Integration)
**Depends on**: Phases 1, 3
**Gate**: All Spec S17.5 test cases pass

### Tasks

**5.1 — Subprocess launch (S10.1)**
- Command: `bash -lc <codex.command>` with workspace as cwd (S10.1)
- Stdout/stderr: separate streams (S10.1)
- Framing: line-delimited JSON on stdout (S10.1)
- Max line size: 10 MB recommended (S10.1)
- AC: Agent launched with correct cwd
- AC: Stdout and stderr captured separately

**5.2 — Session startup handshake (S10.2)**
- Send in order: `initialize` → `initialized` → `thread/start` → `turn/start` (S10.2)
- `initialize`: clientInfo `{name: "symphony", version: "1.0"}`, capabilities (S10.2)
- Wait for `initialize` response within `read_timeout_ms` (S10.2)
- `thread/start`: approvalPolicy, sandbox, cwd (S10.2)
- `turn/start`: threadId, input (rendered prompt), cwd, title=`<identifier>: <title>`, approvalPolicy, sandboxPolicy (S10.2)
- Session IDs: `thread_id` from thread/start result, `turn_id` from turn/start result (S10.2)
- `session_id = "<thread_id>-<turn_id>"` (S10.2)
- AC: Handshake sends 4 messages in correct order
- AC: Session IDs extracted from responses

**5.3 — Streaming turn processing (S10.3)**
- Read line-delimited messages from stdout only (S10.3)
- Buffer partial lines until newline (S10.3)
- Attempt JSON parse on complete lines (S10.3)
- Stderr: ignore or log as diagnostics, do NOT JSON parse (S10.3)
- Completion: `turn/completed` → success; `turn/failed`/`turn/cancelled`/timeout/exit → failure (S10.3)
- AC: Partial lines buffered correctly
- AC: Stderr does not cause parse errors
- AC: Each completion condition handled distinctly

**5.4 — Multi-turn continuation (S10.3)**
- After successful turn: issue another `turn/start` on same `threadId` (S10.3)
- App-server subprocess stays alive across turns (S10.3)
- Stop subprocess only when worker run is ending (S10.3)
- AC: Multiple turns share same thread_id
- AC: Process not killed between turns

**5.5 — Emitted runtime events (S10.4)**
- Emit structured events upstream to orchestrator callback
- Events: `session_started`, `startup_failed`, `turn_completed`, `turn_failed`, `turn_cancelled`, `turn_input_required`, `approval_auto_approved`, `unsupported_tool_call`, `notification`, `other_message`, `malformed` (S10.4)
- Each event includes: `event`, `timestamp`, `codex_app_server_pid`, optional `usage` map (S10.4)
- AC: Each event type has corresponding handler

**5.6 — Approval and tool call handling (S10.5)**
- Implementation-defined approval policy — document the chosen posture (S10.5)
- Symphony default: auto-approve command execution + file changes (S10.5 example)
- User-input-required → hard failure, immediate run termination (S10.5)
- Unsupported dynamic tool calls → return tool failure result `{success: false, error: "unsupported_tool_call"}`, continue session (S10.5)
- Approval/user-input events must NOT leave run stalled indefinitely (S10.5)
- AC: Approval requests auto-approved
- AC: User input request → run fails immediately
- AC: Unsupported tool call → failure response sent, session continues

**5.7 — Optional `linear_graphql` tool extension (S10.5)**
- Available when `tracker.kind == "linear"` + valid auth (S10.5)
- Input: `{query, variables?}` — query must be non-empty string, single operation (S10.5)
- Multiple operations → reject as invalid (S10.5)
- Variables optional, must be JSON object when present (S10.5)
- Reuse configured Linear endpoint + auth (S10.5)
- Results: transport success + no GraphQL errors → `success=true`; GraphQL errors → `success=false` with body preserved; invalid input/missing auth/transport failure → `success=false` with error (S10.5)
- AC: Valid query returns GraphQL response
- AC: Multi-operation query rejected
- AC: GraphQL errors preserved in failure response

**5.8 — Timeout enforcement (S10.6)**
- `read_timeout_ms`: startup handshake + sync request timeout (S10.6)
- `turn_timeout_ms`: total turn stream timeout (S10.6)
- `stall_timeout_ms`: enforced by orchestrator via event inactivity (S10.6)
- Error mapping: `codex_not_found`, `invalid_workspace_cwd`, `response_timeout`, `turn_timeout`, `port_exit`, `response_error`, `turn_failed`, `turn_cancelled`, `turn_input_required` (S10.6)
- AC: Read timeout triggers `response_timeout`
- AC: Turn timeout triggers `turn_timeout`

**5.9 — Token usage extraction (S13.5)**
- Prefer absolute thread totals (e.g., `thread/tokenUsage/updated`) (S13.5)
- Ignore delta-style payloads like `last_token_usage` (S13.5)
- Extract input/output/total tokens leniently (S13.5)
- Track deltas vs. last reported totals to avoid double-counting (S13.5)
- AC: Absolute totals correctly accumulated
- AC: Delta payloads ignored

---

## Phase 6: Observability & HTTP Server
**Depends on**: Phase 4 (needs orchestrator state)
**Gate**: All Spec S17.6 test cases pass

### Tasks

**6.1 — Structured JSON logging (S13.1-13.2)**
- Issue-related logs include: `issue_id`, `issue_identifier` (S13.1)
- Session lifecycle logs include: `session_id` (S13.1)
- Stable `key=value` phrasing, action outcome, concise failure reason (S13.1)
- Avoid logging large raw payloads (S13.1)
- Sink failure → continue running, warn through remaining sinks (S13.2)
- AC: Every dispatch/reconciliation log has issue_id + issue_identifier
- AC: Session start/end logs have session_id

**6.2 — Runtime snapshot (S13.3)**
- Snapshot returns: running list (with turn_count), retrying list, codex_totals (input/output/total/seconds_running), rate_limits (S13.3)
- Snapshot errors: timeout, unavailable (S13.3)
- Runtime reported as live aggregate at snapshot time (S13.5)
- AC: Snapshot includes all required fields
- AC: seconds_running includes active session elapsed time

**6.3 — HTTP server enablement (S13.7)**
- Start when CLI `--port` provided OR `server.port` in WORKFLOW.md (S13.7)
- Precedence: CLI overrides config (S13.7)
- Bind loopback `127.0.0.1` by default (S13.7)
- Port `0` = ephemeral (S13.7)
- Port change does not need hot-rebind; restart OK (S13.7)
- AC: --port 8080 starts server on 8080
- AC: server.port=3000 without --port starts on 3000
- AC: --port 8080 with server.port=3000 → uses 8080

**6.4 — Dashboard endpoint (S13.7.1)**
- `GET /` → human-readable HTML showing active sessions, retry delays, tokens, runtime, events (S13.7.1)
- Server-rendered HTML or client app consuming JSON API (S13.7.1)
- AC: Dashboard loads in browser

**6.5 — JSON API endpoints (S13.7.2)**
- `GET /api/v1/state` → system summary: running, retrying, codex_totals, rate_limits (S13.7.2)
- `GET /api/v1/<identifier>` → issue-specific detail; `404` for unknown (S13.7.2)
- `POST /api/v1/refresh` → trigger immediate poll; `202 Accepted` (S13.7.2)
- Unsupported methods → `405 Method Not Allowed` (S13.7.2)
- Errors → `{"error":{"code":"...","message":"..."}}` envelope (S13.7.2)
- AC: /api/v1/state returns valid JSON matching spec shape
- AC: Unknown identifier returns 404 with error envelope
- AC: POST /api/v1/refresh returns 202

**6.6 — Token accounting (S13.5)**
- Accumulate aggregate totals in orchestrator state (S13.5)
- Add run duration to cumulative counter when session ends (S13.5)
- Active session elapsed derived from started_at at snapshot time (S13.5)
- Rate-limit tracking: latest payload from any agent update (S13.5)
- AC: Token totals increment correctly across sessions
- AC: Runtime includes active session elapsed time

**6.7 — Humanized event summaries (optional) (S13.6)**
- Observability-only output, no orchestrator logic dependency (S13.6)
- AC: If implemented, orchestrator behaves identically with/without summaries

---

## Phase 7: Integration Testing & CLI
**Depends on**: Phases 1-6
**Gate**: All Spec S17.7 + S17.8 test cases pass

### Tasks

**7.1 — CLI argument parsing (S17.7)**
- Optional positional workflow path argument (S17.7)
- Default: `./WORKFLOW.md` when no path provided (S17.7)
- Nonexistent explicit path → error (S17.7)
- Missing default → error (S17.7)
- Startup failure → clean error message (S17.7)
- Exit 0 on normal shutdown; nonzero on startup failure/abnormal exit (S17.7)
- AC: `symphony /nonexistent` exits nonzero with error
- AC: `symphony` without args uses `./WORKFLOW.md`

**7.2 — End-to-end smoke test with mocks (S17.1-17.6)**
- Mock tracker returning predefined issues
- Mock agent subprocess returning scripted protocol messages
- Validate full dispatch cycle: poll → fetch → dispatch → worker → exit → retry
- AC: Full cycle completes without errors

**7.3 — Workflow reload integration test (S6.2)**
- Start service → modify WORKFLOW.md → verify config re-applied
- Invalid edit → verify last good config kept
- AC: Config change detected within reasonable time

**7.4 — Concurrent dispatch test (S8.3)**
- Max concurrency = 2; feed 5 eligible issues; verify only 2 dispatched
- Per-state limit = 1; feed 3 issues in same state; verify 1 dispatched
- AC: Concurrency limits enforced under load

**7.5 — Real Linear integration test (opt-in) (S17.8)**
- Requires `LINEAR_API_KEY` env var
- Uses isolated test identifiers/workspaces
- Cleans up tracker artifacts when practical
- Skipped = reported as skipped, not silently passed
- AC: Real API call succeeds with valid credentials
- AC: Test skipped cleanly when credentials absent

---

## Phase 8: Open Source Release Preparation
**Depends on**: Phase 7
**Gate**: Repository passes community-readiness checklist

### Tasks

**8.1 — License and Attribution**
- Change license from MIT to Apache 2.0 (matches upstream OpenAI Symphony spec)
- Add NOTICE file with attribution to OpenAI Symphony spec (Apache 2.0)
- Add license headers to source files
- AC: LICENSE file is Apache 2.0
- AC: NOTICE file references OpenAI Symphony spec

**8.2 — CI/CD Pipeline**
- GitHub Actions: `make smoke` on every PR
- Release workflow: build binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
- Publish to crates.io
- Docker image published to ghcr.io
- AC: CI runs on PR, release creates binaries

**8.3 — Docker Support**
- Multi-stage Dockerfile (builder + runtime)
- `docker-compose.yml` with Symphony + example workflow
- AC: `docker build .` produces working image
- AC: `docker compose up` runs Symphony with example config

**8.4 — Example Workflows**
- `examples/linear-claude.md` — Linear + Claude Code (current default)
- `examples/linear-codex.md` — Linear + OpenAI Codex
- `examples/github-claude.md` — GitHub Issues + Claude Code (placeholder for future tracker)
- AC: Each example is a valid WORKFLOW.md with inline comments

**8.5 — Contributing Guide**
- CONTRIBUTING.md: how to add tracker plugins, agent runners, build/test/lint
- CODE_OF_CONDUCT.md
- Issue templates: bug report, feature request, tracker plugin
- AC: New contributor can build and test within 5 minutes

**8.6 — Plugin Architecture Documentation**
- Document how to add a new tracker (trait implementation)
- Document how to add a new agent runner
- Document the WORKFLOW.md format extension points
- AC: EXTENDING.md covers tracker + agent runner plugin guide

---

## Phase 9: Symphony Cloud (Managed Service)
**Depends on**: Phase 8
**Gate**: MVP dashboard deployed with single-tenant orchestration

### Tasks

**9.1 — Scaffold symphony-cloud repo (next-forge)**
- Initialize next-forge monorepo: apps/web, apps/app, apps/api
- Strip unused packages (CMS, Storybook initially)
- Configure Turborepo pipeline
- AC: `bun dev` starts all apps locally

**9.2 — Symphony Client SDK**
- TypeScript client for Symphony's HTTP API (`/api/v1/state`, `/api/v1/refresh`, etc.)
- Auto-generated types from Symphony's JSON schema
- Published as `@symphony/client` package in monorepo
- AC: Client can fetch state, trigger refresh, query individual issues

**9.3 — Dashboard MVP (apps/app)**
- Real-time view of running/retrying agents
- Issue detail with logs, token usage, retry history
- Workflow editor (WORKFLOW.md visual editor)
- Connects to Symphony API via client SDK
- AC: Dashboard shows live agent status from running Symphony instance

**9.4 — Control Plane API (apps/api)**
- Tenant provisioning: create/manage Symphony instances
- Workflow CRUD: store and deploy WORKFLOW.md configs
- API key management for Linear/GitHub tokens (encrypted at rest)
- AC: API can create a tenant and start a Symphony instance

**9.5 — Auth and Multi-tenancy**
- Clerk integration for user/team authentication
- Tenant isolation: each tenant gets own Symphony instance + workspace root
- Role-based access: admin, member, viewer
- AC: Two tenants cannot see each other's data

**9.6 — Billing and Usage Metering**
- Stripe integration for subscriptions
- Usage metering: agent-hours, token consumption, concurrent slots
- Tier enforcement: limit concurrent agents per plan
- AC: Free tier limited to 1 agent; paid tier scales

**9.7 — Infrastructure and Deployment**
- Per-tenant Symphony binary orchestration (containers or processes)
- Auto-scaling: spin up/down based on active issues
- Health monitoring and auto-restart
- AC: Tenant's Symphony instance recovers from crash within 60s

**9.8 — Desktop App (Tauri, optional)**
- Tauri v2 wrapper around dashboard React components from packages/ui
- Connects to cloud API or local Symphony instance
- Auto-updater for new versions
- Distribute: DMG (macOS), MSI (Windows), AppImage (Linux)
- AC: Desktop app shows same dashboard as web

---

## Implementation-Defined Decisions

These choices are required by the spec but left to the implementation. Document here:

| Decision | Spec Reference | Symphony Choice |
|----------|---------------|-----------------|
| Approval policy | S10.5, S5.3.6 | Auto-approve all (high-trust) |
| Thread sandbox | S5.3.6 | TBD (choose during Phase 5) |
| Turn sandbox policy | S5.3.6 | TBD (choose during Phase 5) |
| Trust boundary | S15.1 | Trusted environment (single-user) |
| Harness hardening | S15.5 | Workspace isolation + path containment |
