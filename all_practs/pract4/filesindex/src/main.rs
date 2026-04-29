#![warn(clippy::missing_errors_doc, clippy::result_large_err)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use filesindex_core::open_index;

#[derive(Parser)]
#[command(name = "filesindex", about = "Індексатор файлів за тегами")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Додати файл з тегами до індексу
    Add {
        #[arg(long, help = "Шлях до файлу")]
        path: String,
        #[arg(long, value_delimiter = ',', help = "Теги через кому")]
        tags: Vec<String>,
    },
    /// Знайти файли за тегами
    Get {
        #[arg(long, value_delimiter = ',', help = "Теги для пошуку")]
        tags: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = std::env::var("FILES_INDEX_PATH")
        .unwrap_or_else(|_| "json:.files_index.json".to_string());

    let mut index = open_index(&config)
        .context("не вдалося відкрити індекс")?;

    match cli.command {
        Command::Add { path, tags } => {
            index.add(&path, &tags)
                .with_context(|| format!("не вдалося додати файл: {}", path))?;
            println!("Додано: {} [{}]", path, tags.join(", "));
        }
        Command::Get { tags } => {
            let entries = index.get(&tags)
                .context("не вдалося виконати пошук")?;
            if entries.is_empty() {
                println!("Файли не знайдено");
            } else {
                for e in entries {
                    println!("{} [{}]", e.path, e.tags.join(", "));
                }
            }
        }
    }

    Ok(())
}
