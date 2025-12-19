use anyhow::{Context, Result};
use clap::Parser;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    fs,
    io::{self, Read},
};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    read: Option<String>,
    #[arg(long)]
    delete: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct SnippetStore {
    snippets: BTreeMap<String, (String, String)>,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let storage_env = env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snippets.json".into());

    if storage_env.starts_with("JSON:") {
        let path = storage_env.trim_start_matches("JSON:");
        handle_json_storage(path, args)?;
    } else if storage_env.starts_with("SQLITE:") {
        let path = storage_env.trim_start_matches("SQLITE:");
        handle_sqlite_storage(path, args)?;
    } else {
        println!("Unknown storage provider: {}", storage_env);
    }

    Ok(())
}

fn handle_json_storage(path: &str, args: Cli) -> Result<()> {
    let mut store: SnippetStore = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = args.name {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)
            .context("Failed to read snippet content from stdin")?;
        store.snippets.insert(name, (input, now));
        fs::write(path, serde_json::to_string_pretty(&store))
            .context("Failed to write JSON snippet store")?;
    }

    if let Some(name) = args.read {
        if let Some((content, _)) = store.snippets.get(&name) {
            println!("{}", content);
        } else {
            println!("Snippet '{}' not found", name);
        }
    }

    if let Some(name) = args.delete {
        if store.snippets.remove(&name).is_some() {
            fs::write(path, serde_json::to_string_pretty(&store))
                .context("Failed to write JSON snippet store")?;
        } else {
            println!("Snippet '{}' not found", name);
        }
    }

    Ok(())
}

fn handle_sqlite_storage(path: &str, args: Cli) -> Result<()> {
    let conn = Connection::open(path).context("Failed to open SQLite database")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS snippets (
            name TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    ).context("Failed to create snippets table")?;

    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = args.name {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)
            .context("Failed to read snippet content from stdin")?;
        conn.execute(
            "INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)",
            params![name, input, now],
        ).context("Failed to insert snippet into SQLite")?;
    }

    if let Some(name) = args.read {
        let mut stmt = conn.prepare("SELECT content FROM snippets WHERE name = ?1")
            .context("Failed to prepare SELECT statement")?;
        let mut rows = stmt.query([name])
            .context("Failed to execute SELECT query")?;
        if let Some(row) = rows.next().transpose()? {
            let content: String = row.get(0)?;
            println!("{}", content);
        } else {
            println!("Snippet '{}' not found", name);
        }
    }

    if let Some(name) = args.delete {
        let affected = conn.execute("DELETE FROM snippets WHERE name = ?1", [name])
            .context("Failed to execute DELETE query")?;
        if affected == 0 {
            println!("Snippet '{}' not found", name);
        }
    }

    Ok(())
}
