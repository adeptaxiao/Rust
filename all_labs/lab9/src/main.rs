//! Async редактор зображень — оптимізований через JoinSet та Semaphore.
#![warn(missing_docs, missing_crate_level_docs, clippy::missing_panics_doc, clippy::missing_errors_doc, clippy::result_large_err)]

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use clap::Parser;
use futures::stream::TryStreamExt;
use image::io::Reader as ImageReader;
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufReader, AsyncReadExt};
use tokio::task::JoinSet;

/// Помилки програми
#[derive(Debug, Error)]
enum AppError {
    /// HTTP
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    /// Зображення
    #[error("зображення: {0}")]
    Image(#[from] image::ImageError),
    /// IO
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// S3
    #[error("s3: {0}")]
    S3(String),
    /// Формат
    #[error("формат: {0}")]
    Size(String),
}

/// Аргументи командного рядка
#[derive(Parser)]
#[command(name = "image_editor", about = "Оптимізований async редактор")]
struct Args {
    /// Файл зі списком
    #[arg(long)]
    files: PathBuf,
    /// Розмір WIDTHxHEIGHT
    #[arg(long, value_parser = parse_dimensions)]
    resize: (u32, u32),
    /// Зберігати пропорції
    #[arg(long, default_value_t = false)]
    keep_aspect: bool,
    /// Паралельність
    #[arg(long, default_value_t = 16)]
    concurrency: usize,
    /// Worker потоки
    #[arg(long)]
    worker_threads: Option<usize>,
}

/// Розбирає "WIDTHxHEIGHT".
/// # Errors
/// [`AppError::Size`] при помилці.
fn parse_dimensions(s: &str) -> Result<(u32, u32), AppError> {
    let (w, h) = s.split_once('x').ok_or_else(|| AppError::Size("WIDTHxHEIGHT".into()))?;
    Ok((w.parse().map_err(|e: std::num::ParseIntError| AppError::Size(e.to_string()))?,
        h.parse().map_err(|e: std::num::ParseIntError| AppError::Size(e.to_string()))?))
}

trait FileUploader: Send + Sync {
    fn upload<'a>(&'a self, name: &'a str, data: Vec<u8>) -> std::pin::Pin<Box<dyn std::future::Future<Output=Result<(),AppError>> + Send + 'a>>;
}

struct FsUploader { base_path: PathBuf }
impl FileUploader for FsUploader {
    fn upload<'a>(&'a self, name: &'a str, data: Vec<u8>) -> std::pin::Pin<Box<dyn std::future::Future<Output=Result<(),AppError>> + Send + 'a>> {
        Box::pin(async move {
            fs::create_dir_all(&self.base_path).await?;
            let mut f = fs::File::create(self.base_path.join(name)).await?;
            f.write_all(&data).await?; Ok(())
        })
    }
}

struct S3Uploader { client: Client, bucket: String }
impl FileUploader for S3Uploader {
    fn upload<'a>(&'a self, name: &'a str, data: Vec<u8>) -> std::pin::Pin<Box<dyn std::future::Future<Output=Result<(),AppError>> + Send + 'a>> {
        Box::pin(async move {
            self.client.put_object().bucket(&self.bucket).key(name).body(ByteStream::from(data))
                .send().await.map_err(|e| AppError::S3(format!("{e:?}")))?; Ok(())
        })
    }
}

/// Будує uploader.
/// # Panics
/// При відсутніх S3 змінних.
fn build_uploader() -> Arc<dyn FileUploader> {
    match std::env::var("MYME_UPLOADER").as_deref() {
        Ok("s3") => {
            let creds = Credentials::new(std::env::var("AWS_ACCESS_KEY_ID").expect("key"), std::env::var("AWS_SECRET_ACCESS_KEY").expect("secret"), None, None, "env");
            let bucket = std::env::var("S3_BUCKET").expect("bucket");
            let mut b = aws_sdk_s3::Config::builder().credentials_provider(creds).region(Region::new(std::env::var("S3_REGION").unwrap_or_else(|_|"us-east-1".into()))).behavior_version_latest();
            if let Some(ep) = std::env::var("S3_ENDPOINT").ok() { b = b.endpoint_url(ep).force_path_style(true); }
            Arc::new(S3Uploader { client: Client::from_conf(b.build()), bucket })
        }
        _ => Arc::new(FsUploader { base_path: PathBuf::from(std::env::var("MYME_FILES_PATH").unwrap_or_else(|_|"out".into())) })
    }
}

fn decode_text(raw: &[u8]) -> String {
    if raw.starts_with(&[0xFF,0xFE]) { let w:Vec<u16>=raw[2..].chunks_exact(2).map(|b|u16::from_le_bytes([b[0],b[1]])).collect(); String::from_utf16_lossy(&w).to_string() }
    else if raw.starts_with(&[0xFE,0xFF]) { let w:Vec<u16>=raw[2..].chunks_exact(2).map(|b|u16::from_be_bytes([b[0],b[1]])).collect(); String::from_utf16_lossy(&w).to_string() }
    else if raw.starts_with(&[0xEF,0xBB,0xBF]) { String::from_utf8_lossy(&raw[3..]).to_string() }
    else { String::from_utf8_lossy(raw).to_string() }
}

/// Завантажує байти зображення — streaming для URL.
/// # Errors
/// [`AppError`] при мережевій або IO помилці.
async fn fetch_bytes(client: &reqwest::Client, line: &str) -> Result<Vec<u8>, AppError> {
    if line.starts_with("http://") || line.starts_with("https://") {
        let resp = client.get(line).send().await?.error_for_status()?;
        let mut buf = Vec::with_capacity(64 * 1024);
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.try_next().await? { buf.extend_from_slice(&chunk); }
        Ok(buf)
    } else {
        let mut buf = Vec::new();
        BufReader::new(fs::File::open(line).await?).read_to_end(&mut buf).await?;
        Ok(buf)
    }
}

fn output_name(line: &str, i: usize) -> String {
    let raw = line.split('?').next().unwrap_or(line);
    let stem = std::path::Path::new(raw.split(['/','\\']).last().unwrap_or("img")).file_stem().and_then(|s|s.to_str()).unwrap_or("img");
    format!("{i}_{stem}.png")
}

fn cpu_process(bytes: Vec<u8>, w: u32, h: u32, keep: bool) -> Result<Vec<u8>, AppError> {
    let img = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?;
    let r = if keep { img.resize(w,h,image::imageops::FilterType::Lanczos3) } else { img.resize_exact(w,h,image::imageops::FilterType::Lanczos3) };
    let mut buf = Cursor::new(Vec::new()); r.write_to(&mut buf, image::ImageFormat::Png)?; Ok(buf.into_inner())
}

async fn process_entry(client: reqwest::Client, up: Arc<dyn FileUploader>, line: String, w: u32, h: u32, keep: bool, i: usize) {
    let bytes = match fetch_bytes(&client, &line).await { Ok(b)=>b, Err(e)=>{ eprintln!("[{line}] {e}"); return; } };
    let data = match tokio::task::spawn_blocking(move || cpu_process(bytes,w,h,keep)).await {
        Ok(Ok(d))=>d, Ok(Err(e))=>{ eprintln!("[{line}] {e}"); return; } Err(e)=>{ eprintln!("[{line}] {e}"); return; }
    };
    let name = output_name(&line, i);
    match up.upload(&name, data).await { Ok(())=>println!("[{line}] → {name}"), Err(e)=>eprintln!("[{line}] {e}") }
}

async fn run(args: Args) {
    let (w,h) = args.resize;
    let up = build_uploader();
    let raw = fs::read(&args.files).await.expect("файл");
    let lines: Vec<String> = decode_text(&raw).lines().map(str::trim).filter(|l|!l.is_empty()).map(String::from).collect();
    let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).pool_max_idle_per_host(args.concurrency).build().expect("client");
    println!("[async/JoinSet] concurrency={}, файлів={}", args.concurrency, lines.len());
    let t = Instant::now();
    let sem = Arc::new(tokio::sync::Semaphore::new(args.concurrency));
    let mut set = JoinSet::new();
    for (i,line) in lines.into_iter().enumerate() {
        let permit = Arc::clone(&sem).acquire_owned().await.expect("sem");
        let (cl,u) = (client.clone(), Arc::clone(&up));
        set.spawn(async move { let _p = permit; process_entry(cl,u,line,w,h,args.keep_aspect,i).await; });
    }
    let mut done = 0;
    while let Some(r) = set.join_next().await { if let Err(e) = r { eprintln!("[task] {e}"); } done+=1; }
    println!("[час] {:.3}с, задач: {done}", t.elapsed().as_secs_f64());
}

fn main() {
    let args = Args::parse();
    let mut b = tokio::runtime::Builder::new_multi_thread();
    b.enable_all();
    if let Some(n) = args.worker_threads { b.worker_threads(n); }
    b.build().expect("rt").block_on(run(args));
}
