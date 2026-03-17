//! Control commands — check, audit, test, validate (no daemon needed).

use std::path::Path;
use std::process::Stdio;

use super::OutputFormat;

/// Run the `check` command (equivalent to `make smoke`).
pub async fn run_check() -> anyhow::Result<()> {
    println!("Running compile check...");
    run_cargo(&["check", "--workspace"]).await?;

    println!("Running clippy...");
    run_cargo(&["clippy", "--workspace", "--", "-D", "warnings"]).await?;

    println!("Running tests...");
    run_cargo(&["test", "--workspace"]).await?;

    println!("SMOKE PASS");
    Ok(())
}

/// Run the `audit` command (equivalent to `make control-audit`).
pub async fn run_audit() -> anyhow::Result<()> {
    run_check().await?;

    println!("Checking formatting...");
    run_cargo(&["fmt", "--all", "--", "--check"]).await?;

    println!("CONTROL AUDIT PASS");
    Ok(())
}

/// Run the `test` command with optional crate filtering.
pub async fn run_test(crate_name: Option<&str>) -> anyhow::Result<()> {
    let mut args = vec!["test"];
    if let Some(name) = crate_name {
        args.push("-p");
        args.push(name);
    } else {
        args.push("--workspace");
    }

    run_cargo(&args).await
}

/// Run the `validate` command — validate workflow without starting daemon.
/// S43: validates without starting daemon.
pub async fn run_validate(workflow_path: &Path, format: OutputFormat) -> anyhow::Result<()> {
    // Load workflow
    let workflow_def = match symphony_config::loader::load_workflow(workflow_path) {
        Ok(def) => def,
        Err(e) => {
            eprintln!("Validation failed: {e}");
            std::process::exit(1);
        }
    };

    // Extract config
    let config = symphony_config::loader::extract_config(&workflow_def);

    // Validate dispatch config
    let validation_errors = symphony_config::loader::validate_dispatch_config(&config);

    // Try rendering the template with a dummy issue
    let template_result = if !workflow_def.prompt_template.is_empty() {
        let dummy_issue = symphony_core::Issue {
            id: "test-id".into(),
            identifier: "TEST-1".into(),
            title: "Test Issue".into(),
            description: Some("Test description".into()),
            priority: Some(1),
            state: "Todo".into(),
            branch_name: Some("test-branch".into()),
            url: Some("https://example.com".into()),
            labels: vec!["test".into()],
            blocked_by: vec![],
            created_at: Some(chrono::Utc::now()),
            updated_at: None,
        };
        symphony_config::template::render_prompt(&workflow_def.prompt_template, &dummy_issue, None)
    } else {
        Ok(String::new())
    };

    if format == OutputFormat::Json {
        let json = serde_json::json!({
            "workflow_path": workflow_path.display().to_string(),
            "config_valid": validation_errors.is_ok(),
            "config_errors": validation_errors.as_ref().err().unwrap_or(&vec![]),
            "template_valid": template_result.is_ok(),
            "template_error": template_result.as_ref().err().map(|e| e.to_string()),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("Validating: {}", workflow_path.display());
        println!();

        // Config validation
        match &validation_errors {
            Ok(()) => println!("  Config:    OK"),
            Err(errors) => {
                println!("  Config:    FAILED");
                for e in errors {
                    println!("    - {e}");
                }
            }
        }

        // Template validation
        match &template_result {
            Ok(_) => println!("  Template:  OK"),
            Err(e) => println!("  Template:  FAILED ({e})"),
        }
    }

    if validation_errors.is_err() || template_result.is_err() {
        std::process::exit(1);
    }

    Ok(())
}

/// Run a cargo command, streaming output to stdout/stderr.
async fn run_cargo(args: &[&str]) -> anyhow::Result<()> {
    let status = tokio::process::Command::new("cargo")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("cargo {} failed with {}", args.join(" "), status);
    }

    Ok(())
}
