---
tags:
  - symphony
  - marketing
type: marketing
status: draft
area: launch
created: 2026-03-17
---

# X Launch Post

## Primary Post (280 chars)

Shipping Symphony -- an open-source orchestration engine that turns your Linear/GitHub issues into autonomous coding agents.

Point it at your backlog. It polls, clones, runs Claude Code, creates PRs, and handles review feedback. All from a single WORKFLOW.md.

github.com/broomva/symphony

## Thread

### 1/7
We built Symphony because we got tired of manually assigning coding tasks to AI agents one at a time.

What if your issue tracker *was* the interface? Drop an issue to "In Progress", and an agent picks it up automatically.

### 2/7
How it works:

1. Symphony polls Linear or GitHub for active issues
2. Creates an isolated workspace per issue (clones your repo)
3. Runs Claude Code with a rendered prompt (issue title, description, labels)
4. Commits, pushes, and auto-creates a PR
5. Captures PR review comments and feeds them back to the next turn

### 3/7
The secret sauce: a control metalayer.

Every agent session is grounded in a CONTROL.md file with explicit setpoints -- what must be true. Agents check setpoints before coding, run tests to verify them, and update docs after.

This is how you get reliability at scale, not just vibes.

### 4/7
The other key: a knowledge context graph.

Symphony repos are Obsidian vaults with wikilinked documentation. Agents traverse [[links]] to find architecture decisions, crate maps, and configuration references.

Memory persists across sessions. Context compounds.

### 5/7
Built in Rust. 222 tests. Zero warnings.

- 2 trackers: Linear + GitHub Issues
- Lifecycle hooks: clone, rebase, commit, push, PR creation
- PR feedback loop: captures review comments for next agent turn
- done_state: auto-transitions issues when the agent succeeds
- Prometheus /metrics endpoint

### 6/7
Get started in 30 seconds:

```
cargo install symphony-cli
symphony init
# edit WORKFLOW.md with your project details
symphony start
```

Or curl:
```
curl -fsSL https://raw.githubusercontent.com/broomva/symphony/master/install.sh | sh
```

### 7/7
Symphony is Apache 2.0. We're using it to orchestrate agents on our own product (Stimulus -- a procurement intelligence platform).

The first dogfood run: Symphony picked up a Linear ticket, implemented a live support chat feature, pushed a PR, and marked the issue Done. Fully autonomous.

github.com/broomva/symphony
