use anyhow::{Context, Result};
use clap::Parser;
use config::Environment;
use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    fs,
    io::{self, Read},
    path::Path,
};
use tracing::{info, error};
use tracing_subscriber::{FmtSubscriber, EnvFilter};
use rusqlite::{params, Connection};

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

#[derive(Serialize, Deserialize, Default)]
struct SnippetStore {
    snippets: BTreeMap<String, (String, String)>,
}

fn main() -> Result<()> {
    let log_file = env::var("SNIPPETS_APP_LOG_PATH").unwrap_or_else(|_| "snippets.log".into());
    let log_level = env::var("SNIPPETS_APP_LOG_LEVEL").unwrap_or_else(|_| "info".into());
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::new(log_level))
        .with_writer(std::fs::File::create(&log_file)?)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    let args = Cli::parse();
    let storage_env = env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snippets.json".into());

    if storage_env.starts_with("JSON:") {
        let path = storage_env.trim_start_matches("JSON:");
        handle_json_storage(path, args)?;
    } else if storage_env.starts_with("SQLITE:") {
        let path = storage_env.trim_start_matches("SQLITE:");
        handle_sqlite_storage(path, args)?;
    } else {
        error!("Unknown storage provider: {}", storage_env);
    }

    Ok(())
}

fn read_snippet(args: &Cli) -> Result<String> {
    if let Some(url) = &args.download {
        info!("Downloading snippet from {}", url);
        let body = get(url).context("Failed to download snippet")?.text().context("Failed to read downloaded snippet")?;
        Ok(body)
    } else {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).context("Failed to read from stdin")?;
        Ok(input)
    }
}

fn handle_json_storage(path: &str, args: Cli) -> Result<()> {
    let mut store: SnippetStore = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = args.name {
        let content = read_snippet(&args)?;
        store.snippets.insert(name.clone(), (content, now));
        fs::write(path, serde_json::to_string_pretty(&store)).context("Failed to write JSON file")?;
        info!("Snippet '{}' saved", name);
    }

    if let Some(name) = args.read {
        if let Some((content, _)) = store.snippets.get(&name) {
            println!("{}", content);
        } else {
            error!("Snippet '{}' not found", name);
        }
    }

    if let Some(name) = args.delete {
        if store.snippets.remove(&name).is_some() {
            fs::write(path, serde_json::to_string_pretty(&store)).context("Failed to write JSON file")?;
            info!("Snippet '{}' deleted", name);
        } else {
            error!("Snippet '{}' not found", name);
        }
    }

    Ok(())
}

fn handle_sqlite_storage(path: &str, args: Cli) -> Result<()> {
    let conn = Connection::open(path).context("Failed to open SQLite DB")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS snippets (
            name TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    ).context("Failed to create table")?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = args.name {
        let content = read_snippet(&args)?;
        conn.execute(
            "INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)",
            params![name.clone(), content, now],
        ).context("Failed to insert snippet")?;
        info!("Snippet '{}' saved", name);
    }

    if let Some(name) = args.read {
        let mut stmt = conn.prepare("SELECT content FROM snippets WHERE name = ?1")?;
        let mut rows = stmt.query([name.clone()])?;
        if let Some(row) = rows.next().transpose()? {
            let content: String = row.get(0)?;
            println!("{}", content);
        } else {
            error!("Snippet '{}' not found", name);
        }
    }

    if let Some(name) = args.delete {
        let affected = conn.execute("DELETE FROM snippets WHERE name = ?1", [name.clone()])?;
        if affected > 0 {
            info!("Snippet '{}' deleted", name);
        } else {
            error!("Snippet '{}' not found", name);
        }
    }

    Ok(())
}
