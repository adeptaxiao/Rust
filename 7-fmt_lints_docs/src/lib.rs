//! Library for snippets-app, including JSON and SQLite storage.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
};
use rusqlite::{params, Connection};

/// Represents the snippet store for JSON storage.
#[derive(Serialize, Deserialize, Default)]
pub struct SnippetStore {
    pub snippets: BTreeMap<String, (String, String)>,
}

/// Reads snippet content from stdin.
pub fn read_snippet_from_stdin() -> Result<String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("Failed to read from stdin")?;
    Ok(input)
}

/// Handles snippet operations in JSON storage.
pub fn handle_json_storage(
    path: &str,
    name: Option<String>,
    read: Option<String>,
    delete: Option<String>,
    content: Option<String>,
) -> Result<()> {
    let mut store: SnippetStore =
        fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = name {
        store.snippets.insert(name.clone(), (content.unwrap_or_default(), now));
        fs::write(path, serde_json::to_string_pretty(&store)).context("Failed to write JSON file")?;
    }

    if let Some(name) = read {
        if let Some((content, _)) = store.snippets.get(&name) {
            println!("{}", content);
        }
    }

    if let Some(name) = delete {
        store.snippets.remove(&name);
        fs::write(path, serde_json::to_string_pretty(&store)).context("Failed to write JSON file")?;
    }

    Ok(())
}

/// Handles snippet operations in SQLite storage.
pub fn handle_sqlite_storage(
    path: &str,
    name: Option<String>,
    read: Option<String>,
    delete: Option<String>,
    content: Option<String>,
) -> Result<()> {
    let conn = Connection::open(path).context("Failed to open SQLite DB")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS snippets (
            name TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )
    .context("Failed to create table")?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(name) = name {
        conn.execute(
            "INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)",
            params![name.clone(), content.unwrap_or_default(), now],
        )
        .context("Failed to insert snippet")?;
    }

    if let Some(name) = read {
        let mut stmt = conn.prepare("SELECT content FROM snippets WHERE name = ?1")?;
        let mut rows = stmt.query([name.clone()])?;
        if let Some(row) = rows.next().transpose()? {
            let content: String = row.get(0)?;
            println!("{}", content);
        }
    }

    if let Some(name) = delete {
        conn.execute("DELETE FROM snippets WHERE name = ?1", [name.clone()])?;
    }

    Ok(())
}
