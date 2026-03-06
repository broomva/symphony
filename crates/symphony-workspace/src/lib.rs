//! Workspace management (Spec Section 9).
//!
//! Creates, reuses, and cleans per-issue workspace directories.
//! Enforces safety invariants (path containment, sanitization).

use std::path::Path;

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

    /// Create or reuse a workspace for the given issue identifier.
    pub async fn create_for_issue(&self, identifier: &str) -> Result<Workspace, WorkspaceError> {
        let workspace_key = sanitize_identifier(identifier);
        let workspace_path = self.config.root.join(&workspace_key);

        // Safety invariant: workspace must be under root
        self.validate_path_containment(&workspace_path)?;

        let created_now = !workspace_path.exists();
        if created_now {
            tokio::fs::create_dir_all(&workspace_path)
                .await
                .map_err(|e| WorkspaceError::CreationFailed(e.to_string()))?;
        }

        // Run after_create hook only on new workspace
        if created_now
            && let Some(hook) = &self.hooks.after_create
                && let Err(e) = run_hook(hook, &workspace_path, self.hooks.timeout_ms).await {
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

    /// Run the before_run hook. Failure aborts the attempt.
    pub async fn before_run(&self, workspace_path: &Path) -> Result<(), WorkspaceError> {
        if let Some(hook) = &self.hooks.before_run {
            run_hook(hook, workspace_path, self.hooks.timeout_ms).await?;
        }
        Ok(())
    }

    /// Run the after_run hook. Failure is logged and ignored.
    pub async fn after_run(&self, workspace_path: &Path) {
        if let Some(hook) = &self.hooks.after_run
            && let Err(e) = run_hook(hook, workspace_path, self.hooks.timeout_ms).await {
                tracing::warn!(%e, "after_run hook failed (ignored)");
            }
    }

    /// Clean a workspace directory for a terminal issue.
    pub async fn clean(&self, identifier: &str) -> Result<(), WorkspaceError> {
        let workspace_key = sanitize_identifier(identifier);
        let workspace_path = self.config.root.join(&workspace_key);

        if workspace_path.exists() {
            // Run before_remove hook
            if let Some(hook) = &self.hooks.before_remove
                && let Err(e) = run_hook(hook, &workspace_path, self.hooks.timeout_ms).await {
                    tracing::warn!(%e, "before_remove hook failed (ignored)");
                }

            tokio::fs::remove_dir_all(&workspace_path)
                .await
                .map_err(WorkspaceError::Io)?;
        }

        Ok(())
    }

    fn validate_path_containment(&self, workspace_path: &Path) -> Result<(), WorkspaceError> {
        let root = self
            .config
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.config.root.clone());
        let ws = workspace_path
            .canonicalize()
            .unwrap_or_else(|_| workspace_path.to_path_buf());

        if !ws.starts_with(&root) {
            return Err(WorkspaceError::PathEscapesRoot {
                workspace: ws.display().to_string(),
                root: root.display().to_string(),
            });
        }
        Ok(())
    }
}

/// Sanitize an identifier to a workspace-safe key.
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

/// Execute a hook script in the workspace directory with a timeout.
async fn run_hook(script: &str, cwd: &Path, timeout_ms: u64) -> Result<(), WorkspaceError> {
    use tokio::process::Command;

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        Command::new("sh")
            .args(["-lc", script])
            .current_dir(cwd)
            .output(),
    )
    .await;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
