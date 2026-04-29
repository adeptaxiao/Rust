use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub path: String,
    pub tags: Vec<String>,
}

/// Трейт для різних бекендів збереження індексу
pub trait FileIndex {
    fn add(&mut self, path: &str, tags: &[String]) -> Result<(), String>;
    fn get(&self, tags: &[String]) -> Result<Vec<FileEntry>, String>;
}

// --- JSON бекенд ---

pub struct JsonIndex {
    path: String,
}

impl JsonIndex {
    pub fn open(path: &str) -> Self {
        Self { path: path.to_string() }
    }

    fn load(&self) -> HashMap<String, Vec<String>> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self, data: &HashMap<String, Vec<String>>) -> Result<(), String> {
        let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}

impl FileIndex for JsonIndex {
    fn add(&mut self, path: &str, tags: &[String]) -> Result<(), String> {
        let mut data = self.load();
        data.insert(path.to_string(), tags.to_vec());
        self.save(&data)
    }

    fn get(&self, tags: &[String]) -> Result<Vec<FileEntry>, String> {
        let data = self.load();
        let results = data
            .into_iter()
            .filter(|(_, t)| tags.iter().all(|tag| t.contains(tag)))
            .map(|(path, tags)| FileEntry { path, tags })
            .collect();
        Ok(results)
    }
}

// --- SQLite бекенд ---

pub struct SqliteIndex {
    conn: Connection,
}

impl SqliteIndex {
    pub fn open(path: &str) -> Self {
        let conn = Connection::open(path).expect("не вдалося відкрити SQLite");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                path TEXT NOT NULL,
                tag  TEXT NOT NULL,
                PRIMARY KEY (path, tag)
            );",
        )
        .expect("не вдалося створити таблицю");
        Self { conn }
    }
}

impl FileIndex for SqliteIndex {
    fn add(&mut self, path: &str, tags: &[String]) -> Result<(), String> {
        for tag in tags {
            self.conn
                .execute(
                    "INSERT OR REPLACE INTO entries (path, tag) VALUES (?1, ?2)",
                    params![path, tag],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn get(&self, tags: &[String]) -> Result<Vec<FileEntry>, String> {
        if tags.is_empty() {
            return Ok(vec![]);
        }
        // Шукаємо файли які мають ВСІ задані теги
        let placeholders = tags.iter().enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT path FROM entries WHERE tag IN ({}) GROUP BY path HAVING COUNT(DISTINCT tag) = {}",
            placeholders, tags.len()
        );
        let mut stmt = self.conn.prepare(&query).map_err(|e| e.to_string())?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            tags.iter().map(|t| t as &dyn rusqlite::ToSql).collect();
        let paths: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let mut entries = vec![];
        for path in paths {
            let mut tag_stmt = self.conn
                .prepare("SELECT tag FROM entries WHERE path = ?1")
                .map_err(|e| e.to_string())?;
            let tags: Vec<String> = tag_stmt
                .query_map([&path], |row| row.get(0))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            entries.push(FileEntry { path, tags });
        }
        Ok(entries)
    }
}
