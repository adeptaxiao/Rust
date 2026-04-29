use anyhow::{Context, Result};
use clap::Parser;
use futures::stream::{self, StreamExt};
use std::io::{self, BufRead};
use std::path::PathBuf;
use tokio::fs;

#[derive(Parser)]
#[command(name = "web-downloader", about = "Оптимізований async завантажувач (pract9)")]
struct Cli {
    /// Кількість одночасних завантажень (за замовчуванням — кількість ядер)
    #[arg(long, default_value_t = num_cpus())]
    concurrency: usize,

    /// Файл зі списком URL
    file: Option<PathBuf>,
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

async fn download(client: &reqwest::Client, url: &str, index: usize) -> Result<()> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?;

    let body = resp.text().await.context("читання тіла")?;

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
        .with_context(|| format!("запис {}", filename))?;

    println!("✅ {} → {} ({} байт)", url, filename, body.len());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let urls: Vec<String> = if let Some(path) = &cli.file {
        fs::read_to_string(path)
            .await?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect()
    } else {
        io::stdin()
            .lock()
            .lines()
            .filter_map(|l| l.ok())
            .filter(|l| !l.trim().is_empty())
            .collect()
    };

    if urls.is_empty() {
        println!("Список URL порожній");
        return Ok(());
    }

    fs::create_dir_all("downloads").await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    println!(
        "Завантажую {} URL з concurrency={}",
        urls.len(),
        cli.concurrency
    );

    // Рефакторинг: замість spawn для кожної задачі — buffer_unordered
    // Це обмежує кількість одночасних запитів без потреби вручну керувати задачами
    let results: Vec<Result<()>> = stream::iter(urls.iter().enumerate())
        .map(|(i, url)| download(&client, url, i))
        .buffer_unordered(cli.concurrency)
        .collect()
        .await;

    let errors: Vec<_> = results.into_iter().filter_map(|r| r.err()).collect();
    if errors.is_empty() {
        println!("✅ Всі завантаження успішні");
    } else {
        for e in &errors {
            eprintln!("❌ {:#}", e);
        }
    }

    Ok(())
}
