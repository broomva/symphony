# Security Policy

## Reporting a Vulnerability

Please report security issues **privately** — do not open a public issue for
anything exploitable.

- **Preferred:** GitHub → *Security* tab → *Report a vulnerability* (private
  advisory) on this repository.
- **Email:** the repository owner (see the commit author / GitHub profile).

We aim to acknowledge reports within a few days. Responsible disclosure is
greatly appreciated — thank you for helping keep this project secure.

## Handling of Secrets

Symphony's own rule (see `CLAUDE.md` → *Safety Rules*) is: **never commit real
tokens or secret values**. Credentials belong in environment variables or a
secret manager, never in source, config, or documentation.

### Automated enforcement

Two independent, dependency-free guards keep credentials out of the repository:

| Layer | Mechanism | Blocks |
|-------|-----------|--------|
| Redact-at-source | `scripts/conversation-history.py` → `_redact_secrets()` | The conversation bridge scrubs credentials from generated session docs **before** they are written. |
| Pre-commit gate | `.githooks/pre-commit` → `scripts/secret-scan.sh --staged` | Any staged change containing a live credential fails the commit locally. |
| CI gate | `.github/workflows/secret-scan.yml` → `scripts/secret-scan.sh` | Any pushed commit / PR containing a live credential fails the build. |

`scripts/secret-scan.sh` detects credentialed connection strings
(`scheme://user:pass@host`), sensitive `KEY=VALUE` assignments, CLI-flag
credentials (`--token …`), and known token shapes (AWS, Anthropic, OpenAI,
GitHub, Slack, PEM private keys). Placeholders (`localhost`, `$VAR`, `…xxxx`,
`change-me`, already-`[REDACTED]`) are allow-listed.

To activate the local hook once per clone:

```bash
git config core.hooksPath .githooks
```

## Disclosure Log

### 2026-07-13 — Hardcoded credentials in conversation-bridge logs

A researcher reported (thank you) a live Postgres credential committed to a
public conversation-bridge transcript. Investigation found the raw session dump
had also captured a Symphony API bearer token.

- **Exposed:** one Neon Postgres connection string and one `SYMPHONY_API_TOKEN`,
  captured verbatim from `railway variables set …` / `--token …` commands in
  `docs/conversations/`.
- **Root cause:** the conversation bridge published raw tool-call transcripts to
  a public repository with no secret redaction.
- **Remediation:**
  1. Removed the credential values from the working tree (redacted in place).
  2. Added redaction-at-source in the bridge and pre-commit + CI secret-scan
     gates (above) so this class of leak cannot recur.
  3. **Rotated / invalidated the exposed credentials** — because the values were
     public in git history, they are treated as compromised regardless of the
     tree scrub. Rotation is the authoritative fix; the code changes prevent
     recurrence.

> Note: redacting the current tree does not remove a value from historical
> commits. Any credential that was ever committed to a public repository must be
> rotated, not merely deleted.
