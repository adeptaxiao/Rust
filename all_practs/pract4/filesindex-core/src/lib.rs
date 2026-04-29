#![warn(clippy::missing_errors_doc, clippy::result_large_err)]

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Помилки бібліотечного крейту
#[derive(Debug, Error)]
pub enum IndexError {
    #[error("помилка JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("помилка SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("помилка файлової системи: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub path: String,
    pub tags: Vec<String>,
}

/// Трейт для різних бекендів збереження індексу
pub trait FileIndex {
    /// Додає файл з тегами до індексу.
    ///
    /// # Errors
    /// Повертає [`IndexError`] якщо не вдалося зберегти дані.
    fn add(&mut self, path: &str, tags: &[String]) -> Result<(), IndexError>;

    /// Повертає всі файли що мають усі задані теги.
    ///
    /// # Errors
    /// Повертає [`IndexError`] якщо не вдалося прочитати дані.
    fn get(&self, tags: &[String]) -> Result<Vec<FileEntry>, IndexError>;
}

// --- JSON бекенд ---

pub struct JsonIndex {
    path: String,
}

impl JsonIndex {
    pub fn open(path: &str) -> Self {
        Self { path: path.to_string() }
    }

    fn load(&self) -> Result<HashMap<String, Vec<String>>, IndexError> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(serde_json::from_str(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(IndexError::Io(e)),
        }
    }

    fn save(&self, data: &HashMap<String, Vec<String>>) -> Result<(), IndexError> {
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}

impl FileIndex for JsonIndex {
    fn add(&mut self, path: &str, tags: &[String]) -> Result<(), IndexError> {
        let mut data = self.load()?;
        data.insert(path.to_string(), tags.to_vec());
        self.save(&data)
    }

    fn get(&self, tags: &[String]) -> Result<Vec<FileEntry>, IndexError> {
        let data = self.load()?;
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
    /// Відкриває або створює SQLite індекс за заданим шляхом.
    ///
    /// # Errors
    /// Повертає [`IndexError`] якщо не вдалося відкрити базу даних.
    pub fn open(path: &str) -> Result<Self, IndexError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                path TEXT NOT NULL,
                tag  TEXT NOT NULL,
                PRIMARY KEY (path, tag)
            );",
        )?;
        Ok(Self { conn })
    }
}

impl FileIndex for SqliteIndex {
    fn add(&mut self, path: &str, tags: &[String]) -> Result<(), IndexError> {
        for tag in tags {
            self.conn.execute(
                "INSERT OR REPLACE INTO entries (path, tag) VALUES (?1, ?2)",
                params![path, tag],
            )?;
        }
        Ok(())
    }

    fn get(&self, tags: &[String]) -> Result<Vec<FileEntry>, IndexError> {
        if tags.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = tags
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT path FROM entries WHERE tag IN ({}) GROUP BY path HAVING COUNT(DISTINCT tag) = {}",
            placeholders,
            tags.len()
        );
        let mut stmt = self.conn.prepare(&query)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            tags.iter().map(|t| t as &dyn rusqlite::ToSql).collect();
        let paths: Vec<String> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut entries = vec![];
        for path in paths {
            let mut tag_stmt = self
                .conn
                .prepare("SELECT tag FROM entries WHERE path = ?1")?;
            let file_tags: Vec<String> = tag_stmt
                .query_map([&path], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            entries.push(FileEntry { path, tags: file_tags });
        }
        Ok(entries)
    }
}

/// Фабрика: створює потрібний бекенд за рядком конфігурації.
///
/// # Errors
/// Повертає [`IndexError`] якщо не вдалося ініціалізувати бекенд.
pub fn open_index(config: &str) -> Result<Box<dyn FileIndex>, IndexError> {
    let (kind, path) = config
        .split_once(':')
        .expect("FILES_INDEX_PATH має бути у форматі type:path");
    match kind {
        "json" => Ok(Box::new(JsonIndex::open(path))),
        "sqlite" => Ok(Box::new(SqliteIndex::open(path)?)),
        other => panic!("невідомий тип сховища: {}", other),
    }
}
