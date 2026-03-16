---
tags:
  - symphony
  - crate
  - workspace
  - security
type: crate
status: active
area: workspace
created: 2026-03-16
---

# symphony-workspace

**Spec coverage**: S9 (Workspace Management), S15 (Security)
**Path**: `crates/symphony-workspace/src/`
**Tests**: 18

Per-issue directory lifecycle with hook execution and path containment safety.

## Source File

Single file `lib.rs` (512 lines) containing:

- `WorkspaceManager` — main struct with workspace root
- `create_or_reuse()` — workspace creation/reuse with `created_now` flag
- `run_hook()` — subprocess execution with timeout enforcement
- `run_before_run()` / `run_after_run()` / `clean()` — lifecycle operations
- `validate_containment()` — path safety invariant
- `sanitize_identifier()` — character filtering

## Hook Execution (S9.4)

All hooks run via `sh -lc <script>` with workspace as cwd:

| Hook | When | Failure | Environment |
|------|------|---------|-------------|
| `after_create` | New workspace only | Fatal (removes dir) | `SYMPHONY_ISSUE_ID` |
| `before_run` | Before each attempt | Fatal (aborts run) | `SYMPHONY_ISSUE_ID` |
| `after_run` | After each attempt | Logged, ignored | `SYMPHONY_ISSUE_ID` |
| `before_remove` | On cleanup | Logged, ignored | `SYMPHONY_ISSUE_ID` |

Timeout: `hooks.timeout_ms` (default 60000, non-positive = default).

## Safety Invariants (S9.5, S15.2)

1. **Invariant 1**: Agent cwd must equal workspace path
2. **Invariant 2**: Workspace path must have workspace root as prefix (canonicalize + check)
3. **Invariant 3**: Workspace key contains only `[A-Za-z0-9._-]`

Traversal attack example: `../etc` → sanitized to `_.._etc`, stays under root.

## See Also

- [[docs/architecture/Domain Model|Domain Model]] — Workspace type
- [[CONTROL]] — setpoints S23-S28 (workspace safety)
- [[docs/operations/Configuration Reference|Configuration Reference]] — workspace + hooks config
