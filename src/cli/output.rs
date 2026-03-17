// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Output formatting for CLI commands.
//!
//! Supports table (column-aligned) and JSON output modes.

use super::OutputFormat;

/// Print a table with headers and rows.
/// Column widths are computed from the data.
pub fn print_table(headers: &[&str], rows: &[Vec<String>], format: OutputFormat) {
    if format == OutputFormat::Json {
        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (i, header) in headers.iter().enumerate() {
                    let val = row.get(i).cloned().unwrap_or_default();
                    obj.insert(header.to_string(), serde_json::Value::String(val));
                }
                serde_json::Value::Object(obj)
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json_rows).unwrap_or_default()
        );
        return;
    }

    // Compute column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    // Print header
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h.to_uppercase(), width = widths[i]))
        .collect();
    println!("{}", header_line.join("  "));
    let separator: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    println!("{}", separator.join("  "));

    // Print rows
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let width = widths.get(i).copied().unwrap_or(0);
                format!("{:<width$}", cell, width = width)
            })
            .collect();
        println!("{}", cells.join("  "));
    }
}

/// Print a single value with a label (key-value pair).
pub fn print_kv(label: &str, value: &str) {
    println!("  {:<20} {}", label, value);
}

/// Print a JSON value.
pub fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_widths_computed_from_data() {
        // Smoke test: just verify it doesn't panic
        let headers = &["id", "name"];
        let rows = vec![
            vec!["1".to_string(), "short".to_string()],
            vec!["200".to_string(), "a longer name".to_string()],
        ];
        print_table(headers, &rows, OutputFormat::Table);
    }

    #[test]
    fn json_output_is_array() {
        let headers = &["id", "name"];
        let rows = vec![vec!["1".to_string(), "test".to_string()]];
        // Just verify it doesn't panic for JSON mode
        print_table(headers, &rows, OutputFormat::Json);
    }
}
