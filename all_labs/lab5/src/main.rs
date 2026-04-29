//! CLI редактор зображень із паралельною обробкою CPU-bound задач через rayon.
#![warn(missing_docs, missing_crate_level_docs, clippy::missing_panics_doc, clippy::missing_errors_doc, clippy::result_large_err)]

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use clap::Parser;
use image::io::Reader as ImageReader;
use rayon::prelude::*;
use thiserror::Error;

/// Помилки програми
#[derive(Debug, Error)]
enum AppError {
    /// Помилка HTTP
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    /// Помилка зображення
    #[error("зображення: {0}")]
    Image(#[from] image::ImageError),
    /// Помилка IO
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Помилка S3
    #[error("s3: {0}")]
    S3(String),
    /// Некоректний формат
    #[error("формат: {0}")]
    Size(String),
}

/// Аргументи командного рядка
#[derive(Parser)]
#[command(name = "image_editor", about = "Паралельний редактор зображень")]
struct Args {
    /// Файл зі списком зображень
    #[arg(long)]
    files: PathBuf,
    /// Розмір WIDTHxHEIGHT
    #[arg(long, value_parser = parse_dimensions)]
    resize: (u32, u32),
    /// Зберігати пропорції
    #[arg(long, default_value_t = false)]
    keep_aspect: bool,
    /// Послідовний режим (для порівняння)
    #[arg(long, default_value_t = false)]
    sequential: bool,
}

/// Розбирає "WIDTHxHEIGHT".
/// # Errors
/// [`AppError::Size`] при некоректному форматі.
fn parse_dimensions(s: &str) -> Result<(u32, u32), AppError> {
    let (w, h) = s.split_once('x').ok_or_else(|| AppError::Size("WIDTHxHEIGHT".into()))?;
    Ok((w.parse().map_err(|e: std::num::ParseIntError| AppError::Size(e.to_string()))?,
        h.parse().map_err(|e: std::num::ParseIntError| AppError::Size(e.to_string()))?))
}

/// Трейт завантажувача файлів
trait FileUploader: Send + Sync {
    /// Завантажує дані під заданим іменем.
    /// # Errors
    /// [`AppError`] при помилці запису.
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), AppError>;
}

/// Локальне файлове сховище
struct FsUploader { base_path: PathBuf }
impl FileUploader for FsUploader {
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), AppError> {
        fs::create_dir_all(&self.base_path)?;
        fs::write(self.base_path.join(name), data)?;
        Ok(())
    }
}

/// S3 сховище
struct S3Uploader { client: Client, bucket: String, runtime: tokio::runtime::Runtime }
impl FileUploader for S3Uploader {
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), AppError> {
        let (body, bucket, key, client) = (ByteStream::from(data.to_vec()), self.bucket.clone(), name.to_string(), self.client.clone());
        self.runtime.block_on(async move {
            client.put_object().bucket(bucket).key(key).body(body).send().await.map_err(|e| AppError::S3(format!("{e:?}")))?;
            Ok(())
        })
    }
}

/// Будує uploader за MYME_UPLOADER.
/// # Panics
/// При відсутніх змінних середовища для S3.
fn build_uploader() -> Arc<dyn FileUploader> {
    match std::env::var("MYME_UPLOADER").as_deref() {
        Ok("s3") => {
            let creds = Credentials::new(
                std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID"),
                std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY"),
                None, None, "env");
            let bucket = std::env::var("S3_BUCKET").expect("S3_BUCKET");
            let mut b = aws_sdk_s3::Config::builder().credentials_provider(creds)
                .region(Region::new(std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string())))
                .behavior_version_latest();
            if let Some(ep) = std::env::var("S3_ENDPOINT").ok() { b = b.endpoint_url(ep).force_path_style(true); }
            Arc::new(S3Uploader { client: Client::from_conf(b.build()), bucket,
                runtime: tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("rt") })
        }
        _ => Arc::new(FsUploader { base_path: PathBuf::from(std::env::var("MYME_FILES_PATH").unwrap_or_else(|_| "out".to_string())) })
    }
}

fn decode_text(raw: &[u8]) -> String {
    if raw.starts_with(&[0xFF,0xFE]) { let w: Vec<u16>=raw[2..].chunks_exact(2).map(|b|u16::from_le_bytes([b[0],b[1]])).collect(); String::from_utf16_lossy(&w).to_string() }
    else if raw.starts_with(&[0xFE,0xFF]) { let w: Vec<u16>=raw[2..].chunks_exact(2).map(|b|u16::from_be_bytes([b[0],b[1]])).collect(); String::from_utf16_lossy(&w).to_string() }
    else if raw.starts_with(&[0xEF,0xBB,0xBF]) { String::from_utf8_lossy(&raw[3..]).to_string() }
    else { String::from_utf8_lossy(raw).to_string() }
}

/// Завантажує байти зображення.
/// # Errors
/// [`AppError`] при мережевій або IO помилці.
fn fetch_bytes(line: &str) -> Result<Vec<u8>, AppError> {
    if line.starts_with("http://") || line.starts_with("https://") {
        Ok(reqwest::blocking::Client::builder().timeout(Duration::from_secs(30)).build()?.get(line).send()?.bytes()?.to_vec())
    } else { Ok(fs::read(line)?) }
}

fn output_name(line: &str, index: usize) -> String {
    let raw = line.split('?').next().unwrap_or(line);
    let stem = std::path::Path::new(raw.split(['/','\\']).last().unwrap_or("img")).file_stem().and_then(|s|s.to_str()).unwrap_or("img");
    format!("{index}_{stem}.png")
}

/// Обробляє один запис.
fn process_entry(line: &str, w: u32, h: u32, keep: bool, up: &dyn FileUploader, i: usize) {
    let bytes = match fetch_bytes(line) { Ok(b)=>b, Err(e)=>{ eprintln!("[{line}] {e}"); return; } };
    let img = match ImageReader::new(Cursor::new(bytes)).with_guessed_format().and_then(|r|Ok(r.decode())) {
        Ok(Ok(img))=>img, Ok(Err(e))=>{ eprintln!("[{line}] {e}"); return; } Err(e)=>{ eprintln!("[{line}] {e}"); return; }
    };
    let resized = if keep { img.resize(w,h,image::imageops::FilterType::Lanczos3) } else { img.resize_exact(w,h,image::imageops::FilterType::Lanczos3) };
    let mut buf = Cursor::new(Vec::new());
    if let Err(e) = resized.write_to(&mut buf, image::ImageFormat::Png) { eprintln!("[{line}] {e}"); return; }
    let name = output_name(line, i);
    match up.upload(&name, &buf.into_inner()) { Ok(())=>println!("[{line}] → {name}"), Err(e)=>eprintln!("[{line}] {e}") }
}

fn main() {
    let args = Args::parse();
    let (w, h) = args.resize;
    let up = build_uploader();
    let raw = fs::read(&args.files).expect("файл не знайдено");
    let lines: Vec<(usize, String)> = decode_text(&raw).lines().map(str::trim).filter(|l|!l.is_empty()).map(String::from).enumerate().collect();
    let t = Instant::now();
    if args.sequential {
        println!("[послідовний] {} файлів", lines.len());
        for (i,l) in &lines { process_entry(l,w,h,args.keep_aspect,up.as_ref(),*i); }
    } else {
        println!("[паралельний] {} потоків, {} файлів", rayon::current_num_threads(), lines.len());
        lines.par_iter().for_each(|(i,l)| process_entry(l,w,h,args.keep_aspect,up.as_ref(),*i));
    }
    println!("[час] {:.3}с", t.elapsed().as_secs_f64());
}
