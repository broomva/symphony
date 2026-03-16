---
tags:
  - symphony
  - crate
  - config
created: 2026-03-16
---

# symphony-config

**Spec coverage**: S5 (Workflow), S6 (Config), S12 (Prompt)
**Path**: `crates/symphony-config/src/`
**Tests**: 36

Parses [[WORKFLOW]] files, manages typed configuration, renders Liquid templates, and watches for changes.

## Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `loader.rs` | 668 | YAML front matter parsing, env var resolution, config extraction, validation |
| `template.rs` | 314 | Liquid template engine, strict mode, fallback prompt |
| `watcher.rs` | 55 | `notify`-based WORKFLOW.md file watcher |
| `types.rs` | 147 | `ServiceConfig`, `TrackerConfig`, `AgentConfig`, `CodexConfig`, etc. |
| `lib.rs` | 17 | Module exports |

## Key Features

- **Front matter parsing** (S5.2): `---` delimited YAML, non-map = error
- **Environment variable resolution**: `$VAR` → `env::var()`, empty fallback
- **Tilde expansion**: `~/path` → `$HOME/path`
- **Per-state concurrency map**: keys normalized via `trim().to_lowercase()`
- **Dispatch validation** (S6.3): tracker.kind, api_key, project_slug, codex.command
- **Dynamic reload** (S6.2): file watcher triggers re-parse; invalid changes keep last good config
- **Liquid templates** (S5.4): strict variable/filter checking, `issue` + `attempt` variables

## See Also

- [[docs/operations/Configuration Reference|Configuration Reference]] — all settings
- [[WORKFLOW]] — live configuration example
- [[SPEC]] S5-6 — canonical spec sections
