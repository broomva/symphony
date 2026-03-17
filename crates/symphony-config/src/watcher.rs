//! File watcher for WORKFLOW.md dynamic reload (Spec Section 6.2).

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;
use tracing::{error, info};

use crate::loader::{extract_config, load_workflow};
use crate::types::ServiceConfig;

/// Watch a workflow file for changes and broadcast updated configs.
pub async fn watch_workflow(
    path: PathBuf,
    tx: watch::Sender<Arc<ServiceConfig>>,
) -> anyhow::Result<()> {
    use notify::{Event, EventKind, RecursiveMode, Watcher};

    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::channel::<()>(8);

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res
            && matches!(
                event.kind,
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
            )
        {
            let _ = notify_tx.blocking_send(());
        }
    })?;

    watcher.watch(&path, RecursiveMode::NonRecursive)?;

    info!(path = %path.display(), "watching workflow file for changes");

    // Keep watcher alive
    let _watcher = watcher;

    while notify_rx.recv().await.is_some() {
        // Debounce: drain any queued events
        while notify_rx.try_recv().is_ok() {}

        match load_workflow(&path) {
            Ok(def) => {
                let new_config = extract_config(&def);
                info!("workflow reloaded successfully");
                let _ = tx.send(Arc::new(new_config));
            }
            Err(e) => {
                error!(%e, "workflow reload failed, keeping last good config");
            }
        }
    }

    Ok(())
}
