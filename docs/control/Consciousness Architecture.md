---
tags:
  - symphony
  - control
  - architecture
  - meta
type: architecture
status: active
area: consciousness
aliases:
  - Consciousness Architecture
  - Three Substrates
created: 2026-03-17
---

# Consciousness Architecture

Symphony's agent consciousness is built on **three substrates** that together provide persistent, self-improving context across sessions.

## Three Substrates

```
┌─────────────────────────────────────────────────────────────┐
│                  AGENT CONSCIOUSNESS                         │
├─────────────────┬──────────────────┬────────────────────────┤
│  1. CONTROL     │  2. KNOWLEDGE    │  3. EPISODIC           │
│  METALAYER      │  GRAPH           │  MEMORY                │
│                 │                  │                        │
│  .control/      │  docs/           │  docs/conversations/   │
│  CONTROL.md     │  CLAUDE.md       │  ~/.claude/projects/   │
│  METALAYER.md   │  AGENTS.md       │                        │
│                 │  .planning/      │                        │
│                 │                  │                        │
│  Behavioral     │  Declarative     │  Episodic              │
│  governance     │  memory          │  memory                │
│                 │                  │                        │
│  "What MUST     │  "What IS        │  "What HAS             │
│   be true"      │   known"         │   happened"            │
└─────────────────┴──────────────────┴────────────────────────┘
```

### 1. Control Metalayer (Behavioral Governance)

The control metalayer defines **what must be true** at all times. It constrains agent behavior through setpoints, sensors, and actuators.

- **Source of truth**: [[CONTROL]] (76 setpoints)
- **Machine-readable**: `.control/policy.yaml`, `.control/state.json`
- **Reference**: [[METALAYER]]
- **Effect**: Agents cannot commit code that violates blocking setpoints

### 2. Knowledge Graph (Declarative Memory)

The Obsidian vault forms a **wikilinked knowledge graph** of everything known about the project: architecture, conventions, status, requirements.

- **Entry point**: [[docs/Symphony Index|Symphony Index]]
- **Convention**: [[docs/Vault Conventions|Vault Conventions]]
- **Key nodes**: [[CLAUDE]], [[AGENTS]], [[PLANS]], [[SPEC]]
- **Effect**: Agents orient themselves by traversing wikilinks before working

### 3. Episodic Memory (Conversation History)

Session logs capture **what has happened** across agent interactions. The conversation bridge transforms raw logs into searchable, linked markdown.

- **Bridge**: `scripts/conversation-history.py`
- **Output**: [[docs/conversations/Conversations|Conversations]]
- **Sources**: `~/.claude/projects/`, `.entire/logs/`
- **Effect**: Agents can review prior sessions to avoid repeating mistakes

## Progressive Crystallization

Knowledge flows from volatile to permanent through a crystallization path:

```
Working Memory (current conversation)
    ↓  save to auto-memory
Auto-Memory (~/.claude/projects/*/memory/)
    ↓  bridge script
Conversation Logs (docs/conversations/)
    ↓  extract patterns
Knowledge Graph (docs/, .planning/)
    ↓  formalize constraints
Policy Rules (.control/policy.yaml)
    ↓  enforce invariants
Invariants (CONTROL.md setpoints)
```

Each layer is **more stable** than the one above it:
- Working memory lasts one session
- Auto-memory persists across sessions but is mutable
- Conversation logs are append-only
- Knowledge graph is manually curated
- Policy rules require deliberate changes
- Invariants (blocking setpoints) require deviation log entries

## Self-Evolution Cycle

The three substrates form a feedback loop that enables self-improvement:

1. **Agent works** → creates conversation log entries
2. **Bridge runs** → converts logs to searchable markdown
3. **Next agent reads** → learns from prior sessions
4. **Patterns emerge** → crystallize into knowledge graph docs
5. **Constraints formalize** → become setpoints in CONTROL.md
6. **Setpoints enforce** → constrain future agent behavior

This cycle means Symphony's development process **gets better over time** — each session leaves the project more observable, more constrained, and more correct.

## See Also

- [[docs/control/Session Protocol|Session Protocol]] — Actionable on-start/during/on-completion protocol
- [[METALAYER]] — Control metalayer architecture and `.control/` directory
- [[CONTROL]] — Active setpoints (behavioral governance)
- [[docs/conversations/Conversations|Conversations]] — Session history index
- [[CLAUDE]] — Agent conventions including consciousness protocol
- [[AGENTS]] — Architecture guide with context-gathering instructions
