#!/usr/bin/env python3
"""Self-contained tests for the conversation-bridge secret redactor.

Run: python3 scripts/test_secret_redaction.py   (exit 0 = pass, 1 = fail)
No pytest dependency so it runs anywhere, including CI.
"""
import importlib.util
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location(
    "ch", ROOT / "scripts" / "conversation-history.py"
)
assert spec is not None and spec.loader is not None
ch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ch)
redact = ch._redact_secrets

# NOTE: every fixture below is a SYNTHETIC, non-working value chosen only to
# exercise the regexes. Never place a real credential in this file. (This file
# is also excluded from scripts/secret-scan.sh because it contains secret-shaped
# fixtures by design.)
_TK = "0a1b2c3d4e5f60718293a4b5c6d7e8f900112233445566778899aabbccddeeff"  # fake 64-hex

# (input, secret substring that MUST NOT survive)
MUST_REDACT = [
    (f"SYMPHONY_API_TOKEN={_TK}", _TK),
    (f'SYMPHONY_API_TOKEN="{_TK}"', _TK),
    (f"export SYMPHONY_API_TOKEN={_TK}", _TK),
    (f"symphony --host h.railway.app --token {_TK} status", _TK),
    (f"--token {_TK} \\", _TK),
    (
        'ANTHROPIC_API_KEY="sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH"',
        "sk-ant-api03-abcdefghijklmnopqrstuvwxyz",
    ),
    ("GITHUB_TOKEN=ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789", "ghp_ABCDEFGHIJKLMNOP"),
    ("aws key AKIAZ0Z1Z2Z3Z4Z5Z6Z7 here", "AKIAZ0Z1Z2Z3Z4Z5Z6Z7"),  # AKIA + 16
    (
        'DATABASE_URL="postgresql://dbuser:fakePassw0rd123@ep-x.neon.tech/neondb"',
        "fakePassw0rd123",
    ),
]

# Inputs that MUST be returned unchanged (source code + placeholders).
MUST_KEEP = [
    "config.tracker.api_key = resolve_env(&api_key);",
    "api_key: String::new(),",
    "token: parsed.token,",
    'api_token: std::env::var("SYMPHONY_API_TOKEN").ok().filter(|s| !s.is_empty()),',
    "const token = process.env.SYMPHONY_API_TOKEN;",
    "symphony --host symphony-production-0eaf.up.railway.app status",
    "LINEAR_API_KEY=lin_api_xxxx",
    "AUTH_SECRET=change-me-to-a-random-string",
    "SYMPHONY_API_TOKEN=$TOKEN",
    "DATABASE_URL=postgresql://localhost:5432/symphony_dashboard",
    'ANTHROPIC_API_KEY="sk-ant-api03-..."',  # truncated placeholder, not a real key
    "commit c861ef63bf7b24076bd8503f8d392f06a2416f3f",  # git SHA
]


def main() -> int:
    failures = []
    for text, secret in MUST_REDACT:
        out = redact(text)
        if secret in out or "REDACT" not in out:
            failures.append(f"NOT redacted: {text!r} -> {out!r}")
    for text in MUST_KEEP:
        out = redact(text)
        if out != text:
            failures.append(f"wrongly changed: {text!r} -> {out!r}")
    # Idempotency: redacting twice equals redacting once.
    for text, _ in MUST_REDACT:
        once = redact(text)
        if redact(once) != once:
            failures.append(f"not idempotent: {text!r}")

    total = len(MUST_REDACT) + len(MUST_KEEP) + len(MUST_REDACT)
    if failures:
        print(f"FAIL ({len(failures)}/{total}):")
        for f in failures:
            print("  -", f)
        return 1
    print(f"OK: {total} secret-redaction assertions passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
