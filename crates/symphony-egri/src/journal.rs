// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Lago-compatible EGRI event journaling.
//!
//! Writes evaluation records to a JSONL ledger file.

use std::path::Path;

use crate::types::EvalRecord;

/// Write an evaluation record to the JSONL ledger file.
/// Creates parent directories if needed. Appends one JSON line.
pub async fn write_eval_record(
    ledger_path: &Path,
    record: &EvalRecord,
) -> Result<(), std::io::Error> {
    // Ensure parent directory exists
    if let Some(parent) = ledger_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let json = serde_json::to_string(record).map_err(std::io::Error::other)?;

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path)
        .await?;
    file.write_all(json.as_bytes()).await?;
    file.write_all(b"\n").await?;
    file.flush().await?;

    Ok(())
}

/// Read all evaluation records from a JSONL ledger file.
pub async fn read_eval_records(ledger_path: &Path) -> Result<Vec<EvalRecord>, std::io::Error> {
    let content = tokio::fs::read_to_string(ledger_path).await?;
    let records: Vec<EvalRecord> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn write_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = dir.path().join("sub/dir/ledger.jsonl");

        let record = EvalRecord {
            timestamp: Utc::now(),
            score: 0.85,
            completed: 17,
            retrying: 3,
            total_tokens: 42000,
            total_sessions: 20,
            threshold: 0.7,
            passed: true,
        };

        write_eval_record(&ledger, &record).await.unwrap();
        write_eval_record(&ledger, &record).await.unwrap();

        let records = read_eval_records(&ledger).await.unwrap();
        assert_eq!(records.len(), 2);
        assert!((records[0].score - 0.85).abs() < 0.001);
        assert_eq!(records[0].completed, 17);
    }
}
