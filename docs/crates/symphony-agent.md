---
tags:
  - symphony
  - crate
  - agent
  - jsonrpc
created: 2026-03-16
---

# symphony-agent

**Spec coverage**: S10 (Agent Runner Protocol)
**Path**: `crates/symphony-agent/src/`
**Tests**: 16

Manages coding agent subprocesses via JSON-RPC protocol. Handles handshake, turn streaming, tool calls, and multi-turn continuation.

## Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `runner.rs` | 926 | Process spawn, handshake, turn loop, tool handling, token extraction |
| `protocol.rs` | 202 | JSON-RPC message types, agent events, turn outcomes |

## Handshake Sequence (S10.2)

Four messages sent in order:
1. `initialize` — clientInfo `{name: "symphony", version: "1.0"}`
2. `initialized` — notification (no ID)
3. `thread/start` — approvalPolicy, sandbox, cwd
4. `turn/start` — threadId, input (rendered prompt), title, policies

Session IDs: `session_id = "<thread_id>-<turn_id>"`

## Turn Streaming (S10.3)

- Line-delimited JSON on stdout only
- Partial lines buffered until newline
- Stderr ignored (or logged as diagnostics, never parsed)
- Completion signals: `turn/completed`, `turn/failed`, `turn/cancelled`, timeout, exit

## Tool Call Handling (S10.5)

| Event | Response |
|-------|----------|
| Approval request | Auto-approve (high-trust) |
| User input required | Hard failure, immediate termination |
| Unsupported tool call | `{success: false, error: "unsupported_tool_call"}`, continue |
| `linear_graphql` | Delegate to [[docs/crates/symphony-tracker|tracker]]`.graphql_tool` |

## Multi-turn (S7.1, S10.3)

- First turn: full rendered task prompt
- Continuation turns: guidance only (not re-send original prompt)
- Same `thread_id` across turns; new `turn_id` per turn
- Process stays alive between turns

## Dual Mode

- **RPC mode**: JSON-RPC protocol for Codex app-server
- **Simple mode**: Pipe-based for CLI agents (e.g., `claude --dangerously-skip-permissions`)

## Timeout Enforcement (S10.6)

| Timeout | Scope | Config Key |
|---------|-------|------------|
| `read_timeout_ms` | Handshake + sync requests | `codex.read_timeout_ms` |
| `turn_timeout_ms` | Total turn stream | `codex.turn_timeout_ms` |
| `stall_timeout_ms` | Event inactivity (orchestrator-enforced) | `codex.stall_timeout_ms` |

## See Also

- [[docs/architecture/Architecture Overview|Architecture Overview]] — worker lifecycle
- [[CONTROL]] — setpoints S29-S33 (agent runner)
