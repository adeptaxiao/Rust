//! CLI редактор зображень із обробкою помилок через thiserror та повною документацією.
#![warn(
    missing_docs,
    missing_crate_level_docs,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::result_large_err
)]

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use clap::Parser;
use image::io::Reader as ImageReader;
use thiserror::Error;

/// Перелік можливих помилок програми
#[derive(Debug, Error)]
enum AppError {
    /// Помилка HTTP-запиту
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    /// Помилка обробки зображення
    #[error("зображення: {0}")]
    Image(#[from] image::ImageError),
    /// Помилка файлової системи
    #[error("файлова система: {0}")]
    Io(#[from] std::io::Error),
    /// Помилка S3
    #[error("s3: {0}")]
    S3(String),
    /// Некоректний формат розміру
    #[error("формат розміру: {0}")]
    Size(String),
}

/// Аргументи командного рядка
#[derive(Parser)]
#[command(name = "image_editor", about = "Редактор зображень з документованими помилками")]
struct Args {
    /// Шлях до файлу зі списком зображень
    #[arg(long)]
    files: PathBuf,
    /// Зберігати пропорції зображення
    #[arg(long, default_value_t = false)]
    keep_aspect: bool,
    /// Новий розмір у форматі WIDTHxHEIGHT
    #[arg(long, value_parser = parse_dimensions)]
    resize: (u32, u32),
}

/// Розбирає рядок "WIDTHxHEIGHT" у пару чисел.
///
/// # Errors
/// Повертає [`AppError::Size`] якщо формат некоректний.
fn parse_dimensions(s: &str) -> Result<(u32, u32), AppError> {
    let (w, h) = s.split_once('x').ok_or_else(|| AppError::Size("очікується WIDTHxHEIGHT".into()))?;
    let w = w.parse::<u32>().map_err(|e| AppError::Size(e.to_string()))?;
    let h = h.parse::<u32>().map_err(|e| AppError::Size(e.to_string()))?;
    Ok((w, h))
}

/// Трейт для завантаження оброблених файлів у сховище
trait FileUploader {
    /// Завантажує `data` під іменем `name`.
    ///
    /// # Errors
    /// Повертає [`AppError`] при помилці запису.
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), AppError>;
}

/// Зберігає файли у локальну директорію
struct FsUploader {
    base_path: PathBuf,
}

impl FileUploader for FsUploader {
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), AppError> {
        fs::create_dir_all(&self.base_path)?;
        fs::write(self.base_path.join(name), data)?;
        Ok(())
    }
}

/// Завантажує файли у S3-сумісний бакет
struct S3Uploader {
    client: Client,
    bucket: String,
}

impl FileUploader for S3Uploader {
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), AppError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| AppError::S3(e.to_string()))?;
        let body = ByteStream::from(data.to_vec());
        rt.block_on(async {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(name)
                .body(body)
                .send()
                .await
                .map_err(|e| AppError::S3(format!("{e:?}")))?;
            Ok(())
        })
    }
}

/// Будує потрібний uploader залежно від змінної середовища MYME_UPLOADER.
///
/// # Panics
/// Панікує якщо MYME_UPLOADER=s3 але не задані потрібні змінні середовища.
fn build_uploader() -> Box<dyn FileUploader> {
    match std::env::var("MYME_UPLOADER").as_deref() {
        Ok("s3") => {
            let access_key = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID не задано");
            let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY не задано");
            let bucket = std::env::var("S3_BUCKET").expect("S3_BUCKET не задано");
            let region_str = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            let endpoint = std::env::var("S3_ENDPOINT").ok();
            let creds = Credentials::new(access_key, secret_key, None, None, "env");
            let mut builder = aws_sdk_s3::Config::builder()
                .credentials_provider(creds)
                .region(Region::new(region_str))
                .behavior_version_latest();
            if let Some(ep) = endpoint {
                builder = builder.endpoint_url(ep).force_path_style(true);
            }
            Box::new(S3Uploader { client: Client::from_conf(builder.build()), bucket })
        }
        _ => {
            let path = std::env::var("MYME_FILES_PATH").unwrap_or_else(|_| "out".to_string());
            Box::new(FsUploader { base_path: PathBuf::from(path) })
        }
    }
}

/// Декодує байти у рядок, підтримуючи UTF-8 та UTF-16 BOM.
fn decode_text(raw: &[u8]) -> String {
    if raw.starts_with(&[0xFF, 0xFE]) {
        let words: Vec<u16> = raw[2..].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        String::from_utf16_lossy(&words).to_string()
    } else if raw.starts_with(&[0xFE, 0xFF]) {
        let words: Vec<u16> = raw[2..].chunks_exact(2).map(|b| u16::from_be_bytes([b[0], b[1]])).collect();
        String::from_utf16_lossy(&words).to_string()
    } else if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&raw[3..]).to_string()
    } else {
        String::from_utf8_lossy(raw).to_string()
    }
}

/// Будує HTTP-клієнт з таймаутами.
///
/// # Errors
/// Повертає [`AppError::Http`] якщо не вдалося збудувати клієнт.
fn http_client() -> Result<reqwest::blocking::Client, AppError> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()?)
}

/// Завантажує зображення за рядком (URL або локальний шлях).
///
/// # Errors
/// Повертає [`AppError`] при помилці мережі, IO або декодування.
fn fetch_image(line: &str) -> Result<image::DynamicImage, AppError> {
    if line.starts_with("http://") || line.starts_with("https://") {
        let bytes = http_client()?.get(line).send()?.bytes()?;
        Ok(ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?)
    } else {
        Ok(ImageReader::open(line)?.decode()?)
    }
}

/// Формує ім'я вихідного файлу за індексом та оригінальним рядком.
fn make_output_name(line: &str, index: usize) -> String {
    let raw = line.split('?').next().unwrap_or(line);
    let last = raw.split(['/', '\\']).last().unwrap_or("image");
    let stem = std::path::Path::new(last).file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    format!("{index}_{stem}.png")
}

/// Обробляє один рядок: завантажує, змінює розмір, відвантажує.
fn process_entry(line: &str, width: u32, height: u32, keep_aspect: bool, uploader: &dyn FileUploader, index: usize) {
    match fetch_image(line) {
        Err(e) => eprintln!("[{line}] помилка: {e}"),
        Ok(img) => {
            let resized = if keep_aspect { img.resize(width, height, image::imageops::FilterType::Lanczos3) } else { img.resize_exact(width, height, image::imageops::FilterType::Lanczos3) };
            let name = make_output_name(line, index);
            let mut buf = Cursor::new(Vec::new());
            if let Err(e) = resized.write_to(&mut buf, image::ImageFormat::Png) {
                eprintln!("[{line}] помилка кодування: {e}");
                return;
            }
            match uploader.upload(&name, &buf.into_inner()) {
                Ok(()) => println!("[{line}] збережено як {name}"),
                Err(e) => eprintln!("[{line}] помилка завантаження: {e}"),
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let (width, height) = args.resize;
    let uploader = build_uploader();
    let raw = fs::read(&args.files).expect("не вдалося прочитати файл");
    let content = decode_text(&raw);
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() { continue; }
        process_entry(line, width, height, args.keep_aspect, uploader.as_ref(), i);
    }
}
