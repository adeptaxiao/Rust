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

fn main() {
    let args = Cli::parse();
    let storage_env = env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snippets.json".into());

    if storage_env.starts_with("JSON:") {
        let path = storage_env.trim_start_matches("JSON:");
        handle_json_storage(path, args);
    } else if storage_env.starts_with("SQLITE:") {
        let path = storage_env.trim_start_matches("SQLITE:");
        handle_sqlite_storage(path, args);
    }
}

fn handle_json_storage(path: &str, args: Cli) {
    let mut store: SnippetStore = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = args.name {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).unwrap();
        store.snippets.insert(name, (input, now));
        fs::write(path, serde_json::to_string_pretty(&store).unwrap()).unwrap();
        return;
    }

    if let Some(name) = args.read {
        if let Some((content, _)) = store.snippets.get(&name) {
            println!("{}", content);
        }
        return;
    }

    if let Some(name) = args.delete {
        store.snippets.remove(&name);
        fs::write(path, serde_json::to_string_pretty(&store).unwrap()).unwrap();
    }
}

fn handle_sqlite_storage(path: &str, args: Cli) {
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS snippets (
            name TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    ).unwrap();

    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = args.name {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)",
            params![name, input, now],
        ).unwrap();
        return;
    }

    if let Some(name) = args.read {
        let mut stmt = conn.prepare("SELECT content FROM snippets WHERE name = ?1").unwrap();
        let mut rows = stmt.query([name]).unwrap();
        if let Some(row) = rows.next().unwrap() {
            let content: String = row.get(0).unwrap();
            println!("{}", content);
        }
        return;
    }

    if let Some(name) = args.delete {
        conn.execute("DELETE FROM snippets WHERE name = ?1", [name]).unwrap();
    }
}
