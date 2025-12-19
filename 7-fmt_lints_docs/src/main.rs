//! Main entry point for the snippets-app.

use anyhow::Result;
use clap::Parser;
use std::env;
use snippets_app::{handle_json_storage, handle_sqlite_storage, read_snippet_from_stdin};

/// CLI arguments for the snippets-app.
#[derive(Parser)]
pub struct Cli {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub read: Option<String>,
    #[arg(long)]
    pub delete: Option<String>,
    #[arg(long)]
    pub download: Option<String>,
}

/// Main function
fn main() -> Result<()> {
    let log_file = env::var("SNIPPETS_APP_LOG_PATH").unwrap_or_else(|_| "snippets.log".into());
    let log_level = env::var("SNIPPETS_APP_LOG_LEVEL").unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_writer(std::fs::File::create(&log_file)?)
        .init();

    let args = Cli::parse();
    let storage_env = env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snippets.json".into());
    let content = if args.download.is_some() {
        reqwest::blocking::get(args.download.clone().unwrap())?.text()?
    } else {
        read_snippet_from_stdin()?
    };

    if storage_env.starts_with("JSON:") {
        let path = storage_env.trim_start_matches("JSON:");
        handle_json_storage(path, args.name, args.read, args.delete, Some(content))?;
    } else if storage_env.starts_with("SQLITE:") {
        let path = storage_env.trim_start_matches("SQLITE:");
        handle_sqlite_storage(path, args.name, args.read, args.delete, Some(content))?;
    }

    Ok(())
}
