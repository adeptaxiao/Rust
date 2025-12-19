use anyhow::Result;
use clap::Parser;
use std::env;
use snippets_app::{handle_json_storage, handle_sqlite_storage, read_snippet_from_stdin};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    read: Option<String>,
    #[arg(long)]
    delete: Option<String>,
    #[arg(long)]
    download: Option<String>,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let storage_env = env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snippets.json".into());
    let content = if args.download.is_some() {
        reqwest::blocking::get(args.download.unwrap())?.text()?
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
