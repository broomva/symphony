// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Logs command — tail daemon log file with level and time filtering.

use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use super::LogsArgs;

/// Parse a relative duration string ("5m", "1h", "30s") into seconds.
fn parse_relative_duration(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 2 {
        return None;
    }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().ok()?;
    match unit {
        "s" => Some(num),
        "m" => Some(num * 60),
        "h" => Some(num * 3600),
        "d" => Some(num * 86400),
        _ => None,
    }
}

/// Parse --since value into a chrono DateTime cutoff.
fn parse_since(since: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    // Try relative duration first
    if let Some(secs) = parse_relative_duration(since) {
        return Some(chrono::Utc::now() - chrono::Duration::seconds(secs));
    }
    // Try ISO 8601
    since.parse::<chrono::DateTime<chrono::Utc>>().ok()
}

/// Extract log level from a JSON log line.
fn extract_level(line: &str) -> Option<&str> {
    // Fast path: look for "level":"..." pattern in JSON
    let level_key = "\"level\":\"";
    if let Some(start) = line.find(level_key) {
        let value_start = start + level_key.len();
        if let Some(end) = line[value_start..].find('"') {
            return Some(&line[value_start..value_start + end]);
        }
    }
    None
}

/// Extract timestamp from a JSON log line.
fn extract_timestamp(line: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let key = "\"timestamp\":\"";
    if let Some(start) = line.find(key) {
        let value_start = start + key.len();
        if let Some(end) = line[value_start..].find('"') {
            let ts_str = &line[value_start..value_start + end];
            return ts_str.parse().ok();
        }
    }
    None
}

/// Pretty-print a JSON log line for terminal display.
fn pretty_print_line(line: &str) {
    // Try to extract fields for pretty display
    let level = extract_level(line).unwrap_or("???");
    let msg_key = "\"message\":\"";
    let message = if let Some(start) = line.find(msg_key) {
        let value_start = start + msg_key.len();
        if let Some(end) = line[value_start..].find('"') {
            &line[value_start..value_start + end]
        } else {
            line
        }
    } else {
        // Not JSON, print as-is
        println!("{line}");
        return;
    };

    // Extract time portion
    let time = extract_timestamp(line)
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "??:??:??".to_string());

    // Extract target/span if present
    let target_key = "\"target\":\"";
    let target = if let Some(start) = line.find(target_key) {
        let value_start = start + target_key.len();
        if let Some(end) = line[value_start..].find('"') {
            &line[value_start..value_start + end]
        } else {
            ""
        }
    } else {
        ""
    };

    println!("[{time}] {level:>5} {target}: {message}");
}

/// Run the `logs` command — read and optionally follow the log file.
pub async fn run_logs(args: &LogsArgs) -> anyhow::Result<()> {
    let log_path = symphony_config::loader::expand_path(&args.path);
    let log_path = Path::new(&log_path);

    if !log_path.exists() {
        eprintln!("Log file not found: {}", log_path.display());
        std::process::exit(1);
    }

    let since_cutoff = args.since.as_deref().and_then(parse_since);
    let level_filter = args.level.as_deref().map(|l| l.to_lowercase());
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());

    let file = std::fs::File::open(log_path)?;
    let mut reader = BufReader::new(file);

    // If following, start from end minus some lines
    if args.follow {
        // Seek near end to show last 20 lines first
        let metadata = std::fs::metadata(log_path)?;
        let file_size = metadata.len();
        let seek_back = 4096.min(file_size);
        reader.seek(SeekFrom::End(-(seek_back as i64)))?;

        // Discard partial first line
        if seek_back < file_size {
            let mut discard = String::new();
            let _ = reader.read_line(&mut discard);
        }
    }

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                if args.follow {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
                break;
            }
            Ok(_) => {
                let line = line.trim_end();

                // Filter by issue identifier
                if let Some(filter_id) = &args.id
                    && !line.contains(filter_id)
                {
                    continue;
                }

                // Filter by level
                if let Some(ref lvl) = level_filter
                    && let Some(line_level) = extract_level(line)
                    && line_level.to_lowercase() != *lvl
                {
                    continue;
                }

                // Filter by timestamp
                if let Some(cutoff) = since_cutoff
                    && let Some(ts) = extract_timestamp(line)
                    && ts < cutoff
                {
                    continue;
                }

                if is_tty {
                    pretty_print_line(line);
                } else {
                    println!("{line}");
                }
            }
            Err(e) => {
                eprintln!("Error reading log: {e}");
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_relative_duration_seconds() {
        assert_eq!(parse_relative_duration("30s"), Some(30));
    }

    #[test]
    fn parse_relative_duration_minutes() {
        assert_eq!(parse_relative_duration("5m"), Some(300));
    }

    #[test]
    fn parse_relative_duration_hours() {
        assert_eq!(parse_relative_duration("2h"), Some(7200));
    }

    #[test]
    fn parse_relative_duration_days() {
        assert_eq!(parse_relative_duration("1d"), Some(86400));
    }

    #[test]
    fn parse_relative_duration_invalid() {
        assert_eq!(parse_relative_duration("abc"), None);
        assert_eq!(parse_relative_duration(""), None);
    }

    #[test]
    fn extract_level_from_json() {
        let line = r#"{"timestamp":"2026-01-01T00:00:00Z","level":"INFO","message":"hello"}"#;
        assert_eq!(extract_level(line), Some("INFO"));
    }

    #[test]
    fn extract_level_missing() {
        assert_eq!(extract_level("plain text line"), None);
    }

    #[test]
    fn extract_timestamp_from_json() {
        let line = r#"{"timestamp":"2026-01-15T10:30:00Z","level":"INFO","message":"test"}"#;
        let ts = extract_timestamp(line);
        assert!(ts.is_some());
    }
}
