mod storage;

use clap::{Parser, Subcommand};
use storage::{FileIndex, JsonIndex, SqliteIndex};

#[derive(Parser)]
#[command(name = "filesindex", about = "Індексатор файлів за тегами")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Додати файл з тегами
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

fn open_index() -> Box<dyn FileIndex> {
    let env = std::env::var("FILES_INDEX_PATH")
        .unwrap_or_else(|_| "json:.files_index.json".to_string());

    let (kind, path) = env
        .split_once(':')
        .expect("FILES_INDEX_PATH має бути у форматі type:path");

    match kind {
        "json" => Box::new(JsonIndex::open(path)),
        "sqlite" => Box::new(SqliteIndex::open(path)),
        other => panic!("невідомий тип сховища: {}", other),
    }
}

fn main() {
    let cli = Cli::parse();
    let mut index = open_index();

    match cli.command {
        Command::Add { path, tags } => {
            index.add(&path, &tags).expect("не вдалося додати запис");
            println!("Додано: {} [{}]", path, tags.join(", "));
        }
        Command::Get { tags } => {
            let entries = index.get(&tags).expect("не вдалося виконати пошук");
            if entries.is_empty() {
                println!("Файли не знайдено");
            } else {
                for e in entries {
                    println!("{} [{}]", e.path, e.tags.join(", "));
                }
            }
        }
    }
}
