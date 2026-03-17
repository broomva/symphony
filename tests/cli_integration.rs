// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! CLI integration tests — verify the symphony binary works end-to-end.
//!
//! These tests run the actual compiled binary (not unit tests). They validate:
//! - Command parsing and execution
//! - WORKFLOW.md scaffolding (`symphony init`)
//! - Config validation and display
//! - Error handling for missing files and bad input
//! - Health endpoint connectivity (when daemon is reachable)

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn symphony() -> Command {
    Command::cargo_bin("symphony").unwrap()
}

// ── Version and Help ──

#[test]
fn cli_version_prints_version() {
    symphony()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("symphony"));
}

#[test]
fn cli_help_lists_subcommands() {
    symphony()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("status"));
}

// ── Init ──

#[test]
fn cli_init_creates_linear_workflow() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("WORKFLOW.md");

    symphony()
        .args(["init", "--output", output.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"))
        .stdout(predicate::str::contains("LINEAR_API_KEY"));

    let content = fs::read_to_string(&output).unwrap();
    assert!(content.contains("kind: linear"));
    assert!(content.contains("$LINEAR_API_KEY"));
    assert!(content.contains("Control Metalayer"));
}

#[test]
fn cli_init_creates_github_workflow() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("WORKFLOW.md");

    symphony()
        .args([
            "init",
            "--tracker",
            "github",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("GITHUB_TOKEN"));

    let content = fs::read_to_string(&output).unwrap();
    assert!(content.contains("kind: github"));
    assert!(content.contains("$GITHUB_TOKEN"));
}

#[test]
fn cli_init_refuses_overwrite_without_force() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("WORKFLOW.md");
    fs::write(&output, "existing").unwrap();

    symphony()
        .args(["init", "--output", output.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn cli_init_force_overwrites() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("WORKFLOW.md");
    fs::write(&output, "old content").unwrap();

    symphony()
        .args(["init", "--force", "--output", output.to_str().unwrap()])
        .assert()
        .success();

    let content = fs::read_to_string(&output).unwrap();
    assert!(content.contains("kind: linear"));
}

#[test]
fn cli_init_rejects_unsupported_tracker() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("WORKFLOW.md");

    symphony()
        .args([
            "init",
            "--tracker",
            "jira",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported tracker"));
}

// ── Validate ──

#[test]
fn cli_validate_valid_workflow() {
    let dir = TempDir::new().unwrap();
    let wf = dir.path().join("WORKFLOW.md");
    fs::write(
        &wf,
        r#"---
tracker:
  kind: linear
  api_key: test-key
  project_slug: test-proj
codex:
  command: echo hello
---
Prompt for {{ issue.identifier }}
"#,
    )
    .unwrap();

    symphony()
        .args(["validate", wf.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Config:    OK"))
        .stdout(predicate::str::contains("Template:  OK"));
}

#[test]
fn cli_validate_missing_file() {
    symphony()
        .args(["validate", "/nonexistent/WORKFLOW.md"])
        .assert()
        .failure();
}

#[test]
fn cli_validate_missing_api_key() {
    let dir = TempDir::new().unwrap();
    let wf = dir.path().join("WORKFLOW.md");
    fs::write(
        &wf,
        r#"---
tracker:
  kind: linear
  project_slug: proj
codex:
  command: echo
---
Prompt
"#,
    )
    .unwrap();

    symphony()
        .args(["validate", wf.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("FAILED"));
}

// ── Config ──

#[test]
fn cli_config_shows_resolved() {
    let dir = TempDir::new().unwrap();
    let wf = dir.path().join("WORKFLOW.md");
    fs::write(
        &wf,
        r#"---
tracker:
  kind: github
  api_key: ghp_testtoken
  project_slug: owner/repo
  active_states: [open]
  terminal_states: [closed]
hooks:
  pr_feedback: "echo review comments"
codex:
  command: claude
server:
  port: 9090
---
Prompt
"#,
    )
    .unwrap();

    symphony()
        .args(["config", wf.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("kind:           github"))
        .stdout(predicate::str::contains("project_slug:   owner/repo"))
        .stdout(predicate::str::contains(
            "pr_feedback:    echo review comments",
        ))
        .stdout(predicate::str::contains("port:           9090"));
}

// ── Init → Validate round-trip ──

#[test]
fn cli_init_then_validate_roundtrip() {
    let dir = TempDir::new().unwrap();
    let wf = dir.path().join("WORKFLOW.md");

    // Init with linear
    symphony()
        .args(["init", "--output", wf.to_str().unwrap()])
        .env("LINEAR_API_KEY", "lin_api_test123")
        .assert()
        .success();

    // Validate the generated file
    symphony()
        .args(["validate", wf.to_str().unwrap()])
        .env("LINEAR_API_KEY", "lin_api_test123")
        .assert()
        .success()
        .stdout(predicate::str::contains("Config:    OK"));
}

#[test]
fn cli_init_github_then_validate_roundtrip() {
    let dir = TempDir::new().unwrap();
    let wf = dir.path().join("WORKFLOW.md");

    symphony()
        .args([
            "init",
            "--tracker",
            "github",
            "--output",
            wf.to_str().unwrap(),
        ])
        .env("GITHUB_TOKEN", "ghp_test123")
        .assert()
        .success();

    symphony()
        .args(["validate", wf.to_str().unwrap()])
        .env("GITHUB_TOKEN", "ghp_test123")
        .assert()
        .success()
        .stdout(predicate::str::contains("Config:    OK"));
}

// ── Remote daemon access ──

#[test]
fn cli_remote_without_token_fails_gracefully() {
    // Connecting to a host that requires auth without a token should fail gracefully
    symphony()
        .args([
            "--host",
            "symphony-production-0eaf.up.railway.app",
            "status",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unauthorized")
                .or(predicate::str::contains("connection"))
                .or(predicate::str::contains("error")),
        );
}

// ── Run without workflow ──

#[test]
fn cli_run_missing_workflow_fails() {
    symphony()
        .args(["run", "STI-123", "--workflow-path", "/nonexistent/wf.md"])
        .assert()
        .failure();
}
