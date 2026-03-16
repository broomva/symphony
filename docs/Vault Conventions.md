---
tags:
  - symphony
  - meta
aliases:
  - Conventions
  - Documentation Standards
created: 2026-03-16
---

# Vault Conventions

This repository is an Obsidian vault. These conventions ensure a coherent, navigable knowledge graph that serves both human developers and AI agents.

## Tag Taxonomy

Use **flat tags** (not nested) for simplicity and Bases compatibility. Every note must have at least `symphony` plus one category tag.

### Category Tags

| Tag | Use For | Example Files |
|-----|---------|---------------|
| `#symphony` | All notes in this vault | Everything |
| `#architecture` | System design, ADRs, structure | Architecture Overview, Domain Model |
| `#crate` | Per-crate documentation | symphony-core, symphony-agent |
| `#operations` | Build, deploy, config, monitoring | Control Harness, Configuration Reference |
| `#roadmap` | Plans, status, milestones | Project Status, Production Roadmap |
| `#planning` | Requirements, state, decisions | STATE, REQUIREMENTS, PROJECT |
| `#control` | Quality gates, setpoints, testing | CONTROL |
| `#spec` | Specification reference | SPEC |
| `#decision` | Architecture Decision Records | ARCHITECTURE (Open Core) |
| `#contributing` | Community, contributor docs | CONTRIBUTING |
| `#meta` | Vault conventions, vault health | This file |

### Domain Tags (optional, for specificity)

| Tag | Use For |
|-----|---------|
| `#domain` | Core domain model types |
| `#config` | Configuration and workflow |
| `#linear` | Linear tracker integration |
| `#graphql` | GraphQL-specific |
| `#workspace` | Workspace lifecycle |
| `#agent` | Agent subprocess |
| `#orchestrator` | Scheduling and dispatch |
| `#observability` | Logging, metrics, HTTP API |
| `#security` | Safety invariants, path containment |
| `#jsonrpc` | JSON-RPC protocol |
| `#http` | HTTP server and endpoints |
| `#quality` | Testing, CI, lint |
| `#scheduling` | Dispatch, concurrency, retry |
| `#production` | Production readiness |
| `#reference` | Reference material |

## Frontmatter Standard

Every `.md` note in `docs/` and `.planning/` MUST have:

```yaml
---
tags:
  - symphony
  - <category>        # at least one from the category list
aliases:
  - <short name>      # for quick search (e.g., "Crate Map")
created: YYYY-MM-DD
---
```

Root governance files (README, CLAUDE, AGENTS, etc.) SHOULD have frontmatter for graph integration but MAY omit it if it conflicts with their primary role (e.g., GitHub rendering).

## Linking Patterns

### Wikilinks (preferred for internal)

```markdown
[[SPEC]]                                    # Root file
[[docs/crates/symphony-core|symphony-core]] # Docs with display text
[[#Heading in same note]]                   # Same-note heading
```

### Rules

1. **Every note links out** — at minimum, a "See Also" section at the bottom
2. **Every note is linked to** — if you create a note, link it from at least one existing note
3. **Use display text** for paths: `[[docs/crates/symphony-core|symphony-core]]` not `[[docs/crates/symphony-core]]`
4. **Prefer wikilinks** over markdown links for vault-internal references
5. **Use markdown links** only for external URLs

### Callouts

Use callouts for important cross-references and context:

```markdown
> [!info] Related
> Brief context pointing to related notes.

> [!abstract] Implementation
> Points from spec to implementation files.

> [!warning] Known Gap
> Documents a known limitation with link to tracking note.
```

## Note Structure Template

```markdown
---
tags:
  - symphony
  - <category>
aliases:
  - <Short Name>
created: YYYY-MM-DD
---

# Title

Brief description (1-2 sentences).

## Content sections...

## See Also

- [[Related Note 1]] — why it's related
- [[Related Note 2]] — why it's related
```

## Bases Dashboards

`.base` files in `docs/` provide structured views:

| Base | Purpose |
|------|---------|
| [[docs/Crates Dashboard.base\|Crates Dashboard]] | Table of all crate documentation |
| [[docs/Architecture Map.base\|Architecture Map]] | Architecture notes grouped by type |
| [[docs/Vault Health.base\|Vault Health]] | Connectivity audit: orphans, dead ends, hubs |
| [[docs/roadmap/Roadmap Tracker.base\|Roadmap Tracker]] | Planning and roadmap notes with last-updated |

## Folder Structure

```
symphony/                    # Vault root
├── .obsidian/               # Vault config (app.json, graph.json tracked; workspace.json gitignored)
├── docs/                    # Obsidian-native documentation
│   ├── Symphony Index.md    # Entry point / MOC (Map of Content)
│   ├── Vault Conventions.md # This file
│   ├── *.base               # Bases dashboards
│   ├── architecture/        # System design docs
│   ├── crates/              # Per-crate documentation
│   ├── operations/          # Build, config, monitoring
│   └── roadmap/             # Status, plans, milestones
├── .planning/               # Project state and requirements
├── CLAUDE.md                # Agent conventions (references vault)
├── AGENTS.md                # Architecture guide (references vault)
├── CONTROL.md               # Quality setpoints
├── PLANS.md                 # Implementation roadmap
├── SPEC.md                  # Canonical specification
├── README.md                # GitHub entry point
├── WORKFLOW.md              # Live configuration
├── ARCHITECTURE.md          # Open core ADR
└── CONTRIBUTING.md          # Contributor guide
```

### Folder vs Tags

- **Folders** for physical organization and file grouping
- **Tags** for cross-cutting concerns and Bases filtering
- A crate doc lives in `docs/crates/` (folder) AND has `#crate` + `#symphony` (tags)
- Tags enable Bases queries across folders; folders keep the file tree navigable

## Agent Documentation Obligations

Per [[CLAUDE]] and [[AGENTS]], when developing:

| Action | Update |
|--------|--------|
| Add a feature | Relevant `docs/crates/` note |
| Add config option | [[docs/operations/Configuration Reference\|Configuration Reference]] |
| Add test/setpoint | [[CONTROL]] + [[docs/operations/Control Harness\|Control Harness]] |
| Complete a phase | [[.planning/STATE\|State]] + [[.planning/REQUIREMENTS\|Requirements]] + [[docs/roadmap/Project Status\|Project Status]] |
| Architecture decision | New note in `docs/architecture/` or update [[ARCHITECTURE]] |

## See Also

- [[docs/Symphony Index|Symphony Index]] — vault navigation hub
- [[CLAUDE]] — agent conventions including vault section
- [[AGENTS]] — architecture guide including documentation obligations
