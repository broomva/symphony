---
tags:
  - symphony
  - evals
  - egri
type: operations
status: active
area: evaluation
aliases:
  - EGRI Prompt Evals
  - Prompt Optimization
created: 2026-03-17
---

# EGRI Prompt Evaluation Framework

Evaluator-Governed Recursive Improvement (EGRI) for optimizing Symphony's agent prompt template (`stimulus-workflow.md`).

## What is EGRI?

EGRI is a pattern for safe, measurable, rollback-capable optimization:

```
┌──────────────────┐
│  Problem Spec     │  Defines: objective, mutable artifact, constraints, budget
└────────┬─────────┘
         ▼
┌──────────────────┐
│  Constraint Check │  Validates: smoke gate, front matter, required variables
└────────┬─────────┘
         ▼
┌──────────────────┐
│  Mutate Artifact  │  Modify the prompt template (human or agent)
└────────┬─────────┘
         ▼
┌──────────────────┐
│  Evaluate         │  Query Symphony API, compute resolution rate score
└────────┬─────────┘
         ▼
┌──────────────────┐
│  Log to Ledger    │  Append trial result to ledger.jsonl
└────────┬─────────┘
         ▼
┌──────────────────┐
│  Promote/Rollback │  If better → promote. If worse → rollback to baseline.
└──────────────────┘
```

## Key Principle: Mutable Artifact + Immutable Evaluator

- **Mutable**: `stimulus-workflow.md` prompt body (after the YAML front matter)
- **Immutable**: Rust source code, CONTROL.md, .control/ policies, Makefile gates, evaluator scripts

The evaluator never changes. Only the prompt changes. This ensures that improvements are real, not artifacts of a changed measurement.

## Files

| File | Purpose |
|------|---------|
| `problem-spec.yaml` | Problem definition: objective, constraints, budget, autonomy |
| `evaluator.sh` | Scoring script — queries Symphony API, computes resolution rate |
| `run_eval.sh` | Single iteration: constraint-check → evaluate → log |
| `constraint-check.sh` | Validates smoke gate, front matter, template variables |
| `rollback.sh` | Restores stimulus-workflow.md from baseline snapshot |
| `ledger.jsonl` | Append-only trial history (one JSON object per iteration) |
| `baseline/stimulus-workflow.md` | Snapshot of the original prompt for rollback |

## How to Run

### Single evaluation (no mutation)

```bash
make eval-check    # Constraint check only
make eval-run      # Full iteration: constraint-check → evaluate → log
```

### Manual iteration

1. Edit `stimulus-workflow.md` prompt body
2. Run `make eval-check` to validate constraints
3. Run `make eval-run` to evaluate and log
4. Compare score to previous entries in `ledger.jsonl`
5. If worse, run `make eval-rollback` to restore baseline

### Interpreting the Ledger

```bash
# View all trials
cat evals/symphony-prompts/ledger.jsonl | jq .

# Best score
cat evals/symphony-prompts/ledger.jsonl | jq -s 'sort_by(.score) | last'

# Failed constraint checks
cat evals/symphony-prompts/ledger.jsonl | jq 'select(.constraint_pass == false)'
```

### Promoting a Mutation

If a mutation improves the score:
1. Review the change manually
2. Update the baseline: `cp stimulus-workflow.md evals/symphony-prompts/baseline/`
3. Mark the ledger entry as promoted (add `"promoted": true`)
4. Commit both files

## Constraints

All constraints must pass before evaluation:
- `make smoke` passes (compile + clippy + test)
- `stimulus-workflow.md` has valid YAML front matter
- Prompt includes `{{ issue.identifier }}` and `{{ issue.title }}`

## Budget

| Limit | Value |
|-------|-------|
| Max iterations | 10 |
| Max cost | $50 |
| Max time | 2 hours |
| Autonomy | Sandbox (human review required) |

## See Also

- [[CONTROL]] — Setpoints that constrain the optimization
- [[METALAYER]] — Control metalayer architecture
- [[docs/operations/Configuration Reference|Configuration Reference]] — WORKFLOW.md format
- [[docs/Symphony Index|Symphony Index]] — Vault navigation
