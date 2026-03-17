//! Logs command — tail daemon log file.

use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use super::LogsArgs;

/// Run the `logs` command — read and optionally follow the log file.
pub async fn run_logs(args: &LogsArgs) -> anyhow::Result<()> {
    let log_path = symphony_config::loader::expand_path(&args.path);
    let log_path = Path::new(&log_path);

    if !log_path.exists() {
        eprintln!("Log file not found: {}", log_path.display());
        std::process::exit(1);
    }

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
                if let Some(filter_id) = &args.id {
                    // Filter by identifier in JSON log lines
                    if line.contains(filter_id) {
                        println!("{line}");
                    }
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
