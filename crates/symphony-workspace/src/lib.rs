// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Workspace management (Spec Section 9).
//!
//! Creates, reuses, and cleans per-issue workspace directories.
//! Enforces safety invariants (path containment, sanitization).

use std::path::{Path, PathBuf};

use symphony_config::types::{HooksConfig, WorkspaceConfig};
use symphony_core::Workspace;

/// Errors from workspace operations.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace path escapes root: {workspace} not under {root}")]
    PathEscapesRoot { workspace: String, root: String },
    #[error("workspace creation failed: {0}")]
    CreationFailed(String),
    #[error("hook failed: {hook}: {error}")]
    HookFailed { hook: String, error: String },
    #[error("hook timeout: {hook}")]
    HookTimeout { hook: String },
    #[error("path is not a directory: {0}")]
    NotADirectory(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Workspace manager.
pub struct WorkspaceManager {
    config: WorkspaceConfig,
    hooks: HooksConfig,
}

impl WorkspaceManager {
    pub fn new(config: WorkspaceConfig, hooks: HooksConfig) -> Self {
        Self { config, hooks }
    }

    /// Get the workspace root path.
    pub fn root(&self) -> &Path {
        &self.config.root
    }

    /// Compute workspace path for identifier without creating it.
    pub fn workspace_path_for(&self, identifier: &str) -> PathBuf {
        let workspace_key = sanitize_identifier(identifier);
        self.config.root.join(workspace_key)
    }

    /// Create or reuse a workspace for the given issue identifier (S9.1-9.2).
    pub async fn create_for_issue(&self, identifier: &str) -> Result<Workspace, WorkspaceError> {
        let workspace_key = sanitize_identifier(identifier);
        let workspace_path = self.config.root.join(&workspace_key);

        // Safety invariant S9.5: workspace must be under root
        self.validate_path_containment(&workspace_path)?;

        // Check for non-directory at path (S17.2)
        if workspace_path.exists() && !workspace_path.is_dir() {
            return Err(WorkspaceError::NotADirectory(
                workspace_path.display().to_string(),
            ));
        }

        let created_now = !workspace_path.exists();
        if created_now {
            tokio::fs::create_dir_all(&workspace_path)
                .await
                .map_err(|e| WorkspaceError::CreationFailed(e.to_string()))?;
        }

        // Run after_create hook only on new workspace (S9.4)
        if created_now
            && let Some(hook) = &self.hooks.after_create
            && let Err(e) = run_hook_with_env(
                hook,
                &workspace_path,
                self.hooks.timeout_ms,
                &[("SYMPHONY_ISSUE_ID", identifier)],
            )
            .await
        {
            // Fatal: clean up partial workspace
            let _ = tokio::fs::remove_dir_all(&workspace_path).await;
            return Err(e);
        }

        Ok(Workspace {
            path: workspace_path,
            workspace_key,
            created_now,
        })
    }

    /// Run the before_run hook. Failure aborts the attempt (S9.4).
    pub async fn before_run(&self, workspace_path: &Path) -> Result<(), WorkspaceError> {
        self.before_run_with_id(workspace_path, "").await
    }

    /// Run the before_run hook with issue identifier. Failure aborts the attempt (S9.4).
    pub async fn before_run_with_id(
        &self,
        workspace_path: &Path,
        identifier: &str,
    ) -> Result<(), WorkspaceError> {
        self.before_run_with_issue(workspace_path, identifier, "")
            .await
    }

    /// Run the before_run hook with issue identifier and title. Failure aborts the attempt (S9.4).
    pub async fn before_run_with_issue(
        &self,
        workspace_path: &Path,
        identifier: &str,
        title: &str,
    ) -> Result<(), WorkspaceError> {
        if let Some(hook) = &self.hooks.before_run {
            run_hook_with_env(
                hook,
                workspace_path,
                self.hooks.timeout_ms,
                &[
                    ("SYMPHONY_ISSUE_ID", identifier),
                    ("SYMPHONY_ISSUE_TITLE", title),
                ],
            )
            .await?;
        }
        Ok(())
    }

    /// Run the after_run hook. Failure is logged and ignored (S9.4).
    pub async fn after_run(&self, workspace_path: &Path) {
        self.after_run_with_id(workspace_path, "").await;
    }

    /// Run the after_run hook with issue identifier. Failure is logged and ignored (S9.4).
    pub async fn after_run_with_id(&self, workspace_path: &Path, identifier: &str) {
        self.after_run_with_issue(workspace_path, identifier, "")
            .await;
    }

    /// Run the after_run hook with issue identifier and title. Failure is logged and ignored (S9.4).
    pub async fn after_run_with_issue(&self, workspace_path: &Path, identifier: &str, title: &str) {
        if let Some(hook) = &self.hooks.after_run
            && let Err(e) = run_hook_with_env(
                hook,
                workspace_path,
                self.hooks.timeout_ms,
                &[
                    ("SYMPHONY_ISSUE_ID", identifier),
                    ("SYMPHONY_ISSUE_TITLE", title),
                ],
            )
            .await
        {
            tracing::warn!(%e, "after_run hook failed (ignored)");
        }
    }

    /// Run the pr_feedback hook, capturing stdout as feedback content (S59-S62).
    ///
    /// Unlike other hooks, this one returns the hook's stdout as a `String`.
    /// Empty output or failure → returns empty string (non-fatal).
    pub async fn pr_feedback(
        &self,
        workspace_path: &Path,
        identifier: &str,
        title: &str,
    ) -> String {
        let Some(hook) = &self.hooks.pr_feedback else {
            return String::new();
        };

        match run_hook_capture_stdout(
            hook,
            workspace_path,
            self.hooks.timeout_ms,
            &[
                ("SYMPHONY_ISSUE_ID", identifier),
                ("SYMPHONY_ISSUE_TITLE", title),
            ],
        )
        .await
        {
            Ok(output) => output.trim().to_string(),
            Err(e) => {
                tracing::warn!(%e, "pr_feedback hook failed (ignored)");
                String::new()
            }
        }
    }

    /// Clean a workspace directory for a terminal issue (S8.5, S8.6).
    pub async fn clean(&self, identifier: &str) -> Result<(), WorkspaceError> {
        let workspace_key = sanitize_identifier(identifier);
        let workspace_path = self.config.root.join(&workspace_key);

        if workspace_path.exists() {
            // Run before_remove hook (S9.4: failure logged and ignored)
            if let Some(hook) = &self.hooks.before_remove
                && let Err(e) = run_hook_with_env(
                    hook,
                    &workspace_path,
                    self.hooks.timeout_ms,
                    &[("SYMPHONY_ISSUE_ID", identifier)],
                )
                .await
            {
                tracing::warn!(%e, "before_remove hook failed (ignored)");
            }

            tokio::fs::remove_dir_all(&workspace_path)
                .await
                .map_err(WorkspaceError::Io)?;
        }

        Ok(())
    }

    /// Validate path containment (S9.5 Invariant 2).
    fn validate_path_containment(&self, workspace_path: &Path) -> Result<(), WorkspaceError> {
        // Normalize both paths to absolute; workspace_path must have workspace_root as prefix.
        // First try to canonicalize the root (resolves symlinks like /var -> /private/var on macOS).
        let root = self
            .config
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.config.root.clone());

        // For workspace_path, it may not exist yet. Use the canonicalized root + relative part.
        let ws = if workspace_path.exists() {
            workspace_path
                .canonicalize()
                .unwrap_or_else(|_| workspace_path.to_path_buf())
        } else {
            // Construct from canonicalized root + the relative portion
            let rel = workspace_path
                .strip_prefix(&self.config.root)
                .unwrap_or(workspace_path.as_ref());
            root.join(rel)
        };

        if !ws.starts_with(&root) {
            return Err(WorkspaceError::PathEscapesRoot {
                workspace: ws.display().to_string(),
                root: root.display().to_string(),
            });
        }
        Ok(())
    }
}

/// Sanitize an identifier to a workspace-safe key (S4.2, S9.5 Invariant 3).
/// Only `[A-Za-z0-9._-]` allowed; replace all others with `_`.
pub fn sanitize_identifier(identifier: &str) -> String {
    identifier
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Execute a hook script in the workspace directory with a timeout (S9.4).
/// Accepts optional environment variables to pass to the script.
async fn run_hook_with_env(
    script: &str,
    cwd: &Path,
    timeout_ms: u64,
    env_vars: &[(&str, &str)],
) -> Result<(), WorkspaceError> {
    use tokio::process::Command;

    let mut cmd = Command::new("sh");
    cmd.args(["-lc", script]).current_dir(cwd);
    for (key, val) in env_vars {
        cmd.env(key, val);
    }

    let result =
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), cmd.output()).await;

    match result {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(WorkspaceError::HookFailed {
                    hook: script.chars().take(50).collect(),
                    error: stderr.to_string(),
                })
            }
        }
        Ok(Err(e)) => Err(WorkspaceError::HookFailed {
            hook: script.chars().take(50).collect(),
            error: e.to_string(),
        }),
        Err(_) => Err(WorkspaceError::HookTimeout {
            hook: script.chars().take(50).collect(),
        }),
    }
}

/// Execute a hook script and capture stdout (for pr_feedback hook).
/// Returns stdout content on success, error on failure/timeout.
async fn run_hook_capture_stdout(
    script: &str,
    cwd: &Path,
    timeout_ms: u64,
    env_vars: &[(&str, &str)],
) -> Result<String, WorkspaceError> {
    use tokio::process::Command;

    let mut cmd = Command::new("sh");
    cmd.args(["-lc", script]).current_dir(cwd);
    for (key, val) in env_vars {
        cmd.env(key, val);
    }

    let result =
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), cmd.output()).await;

    match result {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(WorkspaceError::HookFailed {
                    hook: script.chars().take(50).collect(),
                    error: stderr.to_string(),
                })
            }
        }
        Ok(Err(e)) => Err(WorkspaceError::HookFailed {
            hook: script.chars().take(50).collect(),
            error: e.to_string(),
        }),
        Err(_) => Err(WorkspaceError::HookTimeout {
            hook: script.chars().take(50).collect(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sanitization tests (S4.2, S9.5 Invariant 3) ──

    #[test]
    fn sanitize_basic_identifier() {
        assert_eq!(sanitize_identifier("ABC-123"), "ABC-123");
    }

    #[test]
    fn sanitize_replaces_slashes() {
        assert_eq!(sanitize_identifier("PROJ/feat#42"), "PROJ_feat_42");
    }

    #[test]
    fn sanitize_preserves_dots_underscores_hyphens() {
        assert_eq!(sanitize_identifier("v1.0_beta-2"), "v1.0_beta-2");
    }

    #[test]
    fn sanitize_traversal_attack() {
        // S9.5: "../etc" → dots are allowed, slash replaced: ".._etc"
        assert_eq!(sanitize_identifier("../etc"), ".._etc");
    }

    #[test]
    fn sanitize_spaces_and_special() {
        assert_eq!(sanitize_identifier("my issue @#$"), "my_issue____");
    }

    // ── Workspace creation/reuse (S9.1-9.2) ──

    #[tokio::test]
    async fn create_new_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );

        let ws = mgr.create_for_issue("ABC-123").await.unwrap();
        assert!(ws.created_now);
        assert_eq!(ws.workspace_key, "ABC-123");
        assert!(ws.path.exists());
        assert!(ws.path.is_dir());
    }

    #[tokio::test]
    async fn reuse_existing_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let ws_dir = dir.path().join("ABC-123");
        std::fs::create_dir_all(&ws_dir).unwrap();

        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );

        let ws = mgr.create_for_issue("ABC-123").await.unwrap();
        assert!(!ws.created_now); // reused
        assert_eq!(ws.workspace_key, "ABC-123");
    }

    #[tokio::test]
    async fn same_identifier_same_path() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );

        let ws1 = mgr.create_for_issue("ABC-123").await.unwrap();
        let ws2 = mgr.create_for_issue("ABC-123").await.unwrap();
        assert_eq!(ws1.path, ws2.path);
    }

    #[tokio::test]
    async fn non_directory_at_path_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("ABC-123");
        std::fs::write(&file_path, "not a dir").unwrap();

        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );

        let err = mgr.create_for_issue("ABC-123").await.unwrap_err();
        assert!(matches!(err, WorkspaceError::NotADirectory(_)));
    }

    // ── Hook execution (S9.4) ──

    #[tokio::test]
    async fn after_create_runs_on_new() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig {
                after_create: Some("touch after_create_ran".into()),
                timeout_ms: 5000,
                ..Default::default()
            },
        );

        let ws = mgr.create_for_issue("HOOK-1").await.unwrap();
        assert!(ws.created_now);
        assert!(ws.path.join("after_create_ran").exists());
    }

    #[tokio::test]
    async fn after_create_failure_removes_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig {
                after_create: Some("exit 1".into()),
                timeout_ms: 5000,
                ..Default::default()
            },
        );

        let err = mgr.create_for_issue("HOOK-2").await.unwrap_err();
        assert!(matches!(err, WorkspaceError::HookFailed { .. }));
        // Workspace directory should be cleaned up
        assert!(!dir.path().join("HOOK-2").exists());
    }

    #[tokio::test]
    async fn before_run_failure_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("ws")).unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig {
                before_run: Some("exit 1".into()),
                timeout_ms: 5000,
                ..Default::default()
            },
        );

        let err = mgr.before_run(&dir.path().join("ws")).await.unwrap_err();
        assert!(matches!(err, WorkspaceError::HookFailed { .. }));
    }

    #[tokio::test]
    async fn after_run_failure_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("ws")).unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig {
                after_run: Some("exit 1".into()),
                timeout_ms: 5000,
                ..Default::default()
            },
        );

        // Should not panic or return error
        mgr.after_run(&dir.path().join("ws")).await;
    }

    #[tokio::test]
    async fn hook_timeout_produces_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("ws")).unwrap();

        let err = run_hook_with_env("sleep 10", &dir.path().join("ws"), 100, &[])
            .await
            .unwrap_err();
        assert!(matches!(err, WorkspaceError::HookTimeout { .. }));
    }

    // ── Cleanup (S8.5, S8.6) ──

    #[tokio::test]
    async fn clean_removes_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let ws_dir = dir.path().join("TERM-1");
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::write(ws_dir.join("file.txt"), "data").unwrap();

        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );

        mgr.clean("TERM-1").await.unwrap();
        assert!(!ws_dir.exists());
    }

    #[tokio::test]
    async fn clean_nonexistent_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );

        mgr.clean("NONEXISTENT").await.unwrap();
    }

    // ── PR feedback hook (S59-S62) ──

    #[tokio::test]
    async fn pr_feedback_captures_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig {
                pr_feedback: Some("echo 'review comment: fix typo'".into()),
                timeout_ms: 5000,
                ..Default::default()
            },
        );

        let feedback = mgr.pr_feedback(&ws, "STI-100", "Test Issue").await;
        assert_eq!(feedback, "review comment: fix typo");
    }

    #[tokio::test]
    async fn pr_feedback_empty_when_no_hook() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(), // no pr_feedback
        );

        let feedback = mgr.pr_feedback(&ws, "STI-100", "Test Issue").await;
        assert!(feedback.is_empty());
    }

    #[tokio::test]
    async fn pr_feedback_failure_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig {
                pr_feedback: Some("exit 1".into()),
                timeout_ms: 5000,
                ..Default::default()
            },
        );

        // Failure returns empty, not error (S62)
        let feedback = mgr.pr_feedback(&ws, "STI-100", "Test Issue").await;
        assert!(feedback.is_empty());
    }

    // ── Path containment (S9.5) ──

    #[test]
    fn path_containment_valid() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );
        let path = dir.path().join("valid-workspace");
        assert!(mgr.validate_path_containment(&path).is_ok());
    }

    #[test]
    fn path_containment_traversal_attack_sanitized() {
        // Traversal "../etc" gets sanitized to "_.._etc" by sanitize_identifier
        // so the resulting path stays under root
        let dir = tempfile::tempdir().unwrap();
        let key = sanitize_identifier("../etc");
        assert_eq!(key, ".._etc");
        let path = dir.path().join(&key);
        let mgr = WorkspaceManager::new(
            WorkspaceConfig {
                root: dir.path().to_path_buf(),
            },
            HooksConfig::default(),
        );
        assert!(mgr.validate_path_containment(&path).is_ok());
    }
}
