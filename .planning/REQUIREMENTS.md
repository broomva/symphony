# REQUIREMENTS.md - Symphony Spec Conformance Checklist

Status: `[ ]` = not started, `[~]` = in progress, `[x]` = done + tested

## Core Conformance (Spec Section 18.1)

### Domain Model (S4)
- [x] S4.1.1 — Issue entity with all fields (id, identifier, title, description, priority, state, branch_name, url, labels, blocked_by, created_at, updated_at)
- [x] S4.1.2 — WorkflowDefinition (config map + prompt_template string)
- [x] S4.1.3 — ServiceConfig typed view with all config sections
- [x] S4.1.4 — Workspace entity (path, workspace_key, created_now)
- [x] S4.1.5 — RunAttempt entity (issue_id, attempt, workspace_path, started_at, status, error)
- [x] S4.1.6 — LiveSession entity (session_id, thread_id, turn_id, tokens, turn_count)
- [x] S4.1.7 — RetryEntry entity (issue_id, identifier, attempt, due_at_ms, error)
- [x] S4.1.8 — OrchestratorState (running map, claimed set, retry_attempts, codex_totals)
- [x] S4.2 — Workspace key sanitization: `[^A-Za-z0-9._-]` → `_`
- [x] S4.2 — State normalization: trim + lowercase
- [x] S4.2 — Session ID composition: `<thread_id>-<turn_id>`

### Workflow & Config (S5, S6)
- [x] S5.1 — Workflow path: explicit runtime path > cwd default `WORKFLOW.md`
- [x] S5.2 — YAML front matter parsing with `---` delimiters
- [x] S5.2 — Non-map front matter returns typed error
- [x] S5.2 — Missing file returns typed error
- [ ] S5.3.1 — Tracker config: kind, endpoint, api_key ($VAR), project_slug, active/terminal states
- [ ] S5.3.2 — Polling config: interval_ms with string-integer coercion
- [ ] S5.3.3 — Workspace config: root with `~` expansion, `$VAR` resolution
- [ ] S5.3.4 — Hooks config: all 4 hooks + timeout_ms (non-positive = default)
- [ ] S5.3.5 — Agent config: max_concurrent_agents, max_turns, max_retry_backoff_ms, per-state map (normalized keys)
- [ ] S5.3.6 — Codex config: command, approval_policy, sandbox settings, timeouts
- [ ] S5.4 — Strict template rendering with `issue` and `attempt` variables
- [ ] S5.4 — Unknown template variables/filters fail
- [ ] S5.4 — Empty prompt body → fallback prompt
- [ ] S5.5 — Error classes: missing_workflow_file, workflow_parse_error, front_matter_not_a_map, template_parse_error, template_render_error
- [ ] S5.5 — File/YAML errors block dispatch; template errors fail only affected run
- [x] S6.1 — Config defaults apply when optional values missing
- [x] S6.1 — `$VAR` resolution for api_key and path values
- [x] S6.1 — `~` home expansion for paths
- [ ] S6.2 — Dynamic WORKFLOW.md watch/reload/re-apply without restart
- [ ] S6.2 — Invalid reload keeps last good config + emits error
- [ ] S6.2 — Defensive re-validation before dispatch
- [x] S6.3 — Dispatch preflight validation: tracker.kind, api_key, project_slug, codex.command

### Orchestrator (S7, S8)
- [x] S7.1 — Issue orchestration states: Unclaimed, Claimed, Running, RetryQueued, Released
- [ ] S7.1 — Multi-turn continuation: re-check state, start new turn on same thread if active
- [ ] S7.1 — First turn uses full prompt; continuation uses guidance only
- [ ] S7.2 — Run attempt lifecycle: PreparingWorkspace → ... → Succeeded/Failed/TimedOut/Stalled/Canceled
- [ ] S8.1 — Poll loop: reconcile → validate → fetch → sort → dispatch → notify → schedule
- [x] S8.2 — Candidate eligibility: required fields, active state, not claimed/running, slots available
- [x] S8.2 — Blocker rule: Todo with non-terminal blocker → ineligible
- [x] S8.2 — Dispatch sort: priority ASC (null last) → created_at oldest → identifier lexicographic
- [ ] S8.3 — Global concurrency: max_concurrent_agents - running_count
- [ ] S8.3 — Per-state concurrency: max_concurrent_agents_by_state with normalized key
- [x] S8.4 — Continuation retry: 1000ms fixed delay
- [x] S8.4 — Failure retry: min(10000 * 2^(attempt-1), max_backoff)
- [ ] S8.4 — Retry handling: fetch candidates → find issue → dispatch or release
- [ ] S8.5 — Stall detection: elapsed since last activity > stall_timeout → kill + retry
- [ ] S8.5 — Stall timeout ≤ 0 disables detection
- [ ] S8.5 — Terminal state → terminate + clean workspace
- [ ] S8.5 — Active state → update snapshot
- [ ] S8.5 — Neither active nor terminal → terminate without cleanup
- [ ] S8.5 — Refresh failure → keep workers running
- [ ] S8.6 — Startup terminal workspace cleanup
- [ ] S8.6 — Terminal fetch failure → log warning, continue

### Workspace (S9)
- [x] S9.1 — Workspace path: `<root>/<sanitized_identifier>`
- [x] S9.2 — Create new dir → `created_now=true`; reuse existing → `created_now=false`
- [ ] S9.2 — Only run `after_create` when `created_now=true`
- [ ] S9.3 — Workspace population is implementation-defined (via hooks)
- [ ] S9.4 — Hook execution: `sh -lc <script>` with workspace cwd
- [ ] S9.4 — after_create failure = fatal to creation
- [ ] S9.4 — before_run failure = fatal to attempt
- [ ] S9.4 — after_run failure = logged and ignored
- [ ] S9.4 — before_remove failure = logged and ignored
- [ ] S9.4 — Hook timeout enforcement (hooks.timeout_ms)
- [x] S9.5 — Invariant 1: cwd == workspace_path before agent launch
- [x] S9.5 — Invariant 2: workspace_path has workspace_root as prefix
- [x] S9.5 — Invariant 3: workspace key only `[A-Za-z0-9._-]`

### Agent Runner (S10)
- [ ] S10.1 — Launch via `bash -lc <codex.command>` with workspace cwd
- [ ] S10.1 — Stdout/stderr separate; line-delimited JSON on stdout
- [ ] S10.2 — Handshake: initialize → initialized → thread/start → turn/start
- [ ] S10.2 — Session IDs: thread_id from thread/start, turn_id from turn/start
- [ ] S10.3 — Turn completion: turn/completed, turn/failed, turn/cancelled, timeout, exit
- [ ] S10.3 — Partial lines buffered; stderr not parsed as protocol
- [ ] S10.3 — Continuation turns on same threadId
- [ ] S10.4 — Emitted events: session_started, startup_failed, turn_completed, etc.
- [ ] S10.5 — Approval policy documented and implemented
- [ ] S10.5 — User input → hard failure (no stall)
- [ ] S10.5 — Unsupported tool call → failure result, continue session
- [ ] S10.6 — Timeouts: read_timeout_ms, turn_timeout_ms, stall_timeout_ms
- [ ] S10.6 — Error mapping: codex_not_found, turn_timeout, etc.

### Issue Tracker (S11)
- [ ] S11.1 — fetch_candidate_issues()
- [ ] S11.1 — fetch_issues_by_states()
- [ ] S11.1 — fetch_issue_states_by_ids()
- [ ] S11.2 — Linear query: slugId filter, pagination (page_size=50), timeout 30000ms
- [ ] S11.2 — Issue state refresh uses `[ID!]` variable type
- [ ] S11.3 — Labels lowercase, blockers from inverse "blocks" relation
- [ ] S11.3 — Priority integer only (non-integer → null)
- [ ] S11.3 — Timestamps: ISO-8601 parsing
- [ ] S11.4 — Error categories: all 8 TrackerError variants

### Prompt (S12)
- [ ] S12.1 — Inputs: prompt_template + issue + attempt
- [ ] S12.2 — Strict variable + filter checking
- [ ] S12.2 — Issue keys as strings; preserve nested arrays/maps
- [ ] S12.3 — attempt: null on first run, integer on retry/continuation
- [ ] S12.4 — Render failure → fail run attempt

### Observability (S13)
- [ ] S13.1 — Structured logs: issue_id + issue_identifier on issue logs; session_id on session logs
- [ ] S13.2 — Sink failure → continue running
- [ ] S13.3 — Runtime snapshot: running, retrying, codex_totals, rate_limits
- [ ] S13.5 — Token accounting: absolute totals, ignore deltas, avoid double-counting
- [ ] S13.5 — Runtime: cumulative ended + active elapsed at snapshot time

### CLI (S17.7)
- [x] S17.7 — Optional positional workflow path; default ./WORKFLOW.md
- [ ] S17.7 — Nonexistent explicit path → error
- [ ] S17.7 — Missing default → error
- [ ] S17.7 — Exit 0 on normal shutdown; nonzero on failure

### Security (S15)
- [x] S15.2 — Workspace path under root
- [x] S15.2 — Coding-agent cwd = workspace path
- [x] S15.2 — Sanitized directory names
- [ ] S15.3 — $VAR indirection; no secrets in logs
- [ ] S15.4 — Hooks are trusted config; timeout enforced; output truncated

## Extension Conformance (Spec Section 18.2)

- [ ] S13.7 — HTTP server: CLI --port > server.port, loopback default
- [ ] S13.7.1 — Dashboard at /
- [ ] S13.7.2 — JSON API: /api/v1/state, /api/v1/<id>, /api/v1/refresh
- [ ] S13.7.2 — Error semantics: 404, 405, 202, error envelope
- [ ] S10.5 — linear_graphql client-side tool extension

## Real Integration (Spec Section 17.8)

- [ ] S17.8 — Real Linear smoke test with valid credentials
- [ ] S17.8 — Isolated test identifiers/workspaces
- [ ] S17.8 — Skipped when credentials absent (reported as skipped)
