use anyhow::{Context, Result};
use clap::Parser;
use std::io::{self, BufRead};
use std::path::PathBuf;
use tokio::fs;

#[derive(Parser)]
#[command(name = "web-downloader", about = "Асинхронний завантажувач веб-сторінок")]
struct Cli {
    /// Кількість потоків для tokio runtime (за замовчуванням — кількість ядер CPU)
    #[arg(long)]
    max_threads: Option<usize>,

    /// Файл зі списком URL (по одному на рядок). Якщо не задано — читати зі stdin
    file: Option<PathBuf>,
}

async fn download(client: reqwest::Client, url: String, index: usize) -> Result<()> {
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?;

    let body = resp.text().await.context("читання тіла відповіді")?;

    // Назва файлу: індекс_домен
    let filename = format!(
        "downloads/{}_{}", index,
        url.trim_start_matches("https://")
           .trim_start_matches("http://")
           .split('/')
           .next()
           .unwrap_or("unknown")
    );

    fs::write(&filename, &body)
        .await
        .with_context(|| format!("запис у {}", filename))?;

    println!("✅ {} → {} ({} байт)", url, filename, body.len());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Зчитуємо URL-и
    let urls: Vec<String> = if let Some(path) = &cli.file {
        let content = fs::read_to_string(path)
            .await
            .with_context(|| format!("читання файлу {:?}", path))?;
        content.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect()
    } else {
        let stdin = io::stdin();
        stdin.lock().lines().filter_map(|l| l.ok()).filter(|l| !l.trim().is_empty()).collect()
    };

    if urls.is_empty() {
        println!("Список URL порожній");
        return Ok(());
    }

    fs::create_dir_all("downloads").await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Запускаємо всі завантаження паралельно
    let tasks: Vec<_> = urls
        .into_iter()
        .enumerate()
        .map(|(i, url)| tokio::spawn(download(client.clone(), url, i)))
        .collect();

    for task in tasks {
        if let Err(e) = task.await? {
            eprintln!("❌ Помилка: {:#}", e);
        }
    }

    println!("Завантаження завершено");
    Ok(())
}
