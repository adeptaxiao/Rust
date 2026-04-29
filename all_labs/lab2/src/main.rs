//! CLI редактор зображень із підтримкою завантаження у ФС або S3.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use clap::Parser;
use image::io::Reader as ImageReader;

/// Аргументи командного рядка
#[derive(Parser)]
#[command(name = "image_editor", about = "Редактор зображень з вибором сховища")]
struct Args {
    /// Шлях до файлу зі списком зображень
    #[arg(long)]
    files: PathBuf,

    /// Новий розмір у форматі WIDTHxHEIGHT
    #[arg(long, value_parser = parse_dimensions)]
    resize: (u32, u32),
}

fn parse_dimensions(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x').ok_or("очікується WIDTHxHEIGHT")?;
    let w = w.parse::<u32>().map_err(|e| e.to_string())?;
    let h = h.parse::<u32>().map_err(|e| e.to_string())?;
    Ok((w, h))
}

/// Трейт для завантаження файлів у різні сховища
trait FileUploader {
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), String>;
}

/// Зберігає файли у локальну директорію
struct FsUploader {
    base_path: PathBuf,
}

impl FileUploader for FsUploader {
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), String> {
        fs::create_dir_all(&self.base_path).map_err(|e| e.to_string())?;
        fs::write(self.base_path.join(name), data).map_err(|e| e.to_string())
    }
}

/// Завантажує файли у S3-сумісний бакет
struct S3Uploader {
    client: Client,
    bucket: String,
}

impl FileUploader for S3Uploader {
    fn upload(&self, name: &str, data: &[u8]) -> Result<(), String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let body = ByteStream::from(data.to_vec());
        rt.block_on(async {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(name)
                .body(body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            Ok(())
        })
    }
}

/// Будує потрібний uploader залежно від MYME_UPLOADER
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

            Box::new(S3Uploader {
                client: Client::from_conf(builder.build()),
                bucket,
            })
        }
        _ => {
            let path = std::env::var("MYME_FILES_PATH").unwrap_or_else(|_| "out".to_string());
            Box::new(FsUploader { base_path: PathBuf::from(path) })
        }
    }
}

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

fn fetch_image(line: &str) -> Result<image::DynamicImage, String> {
    if line.starts_with("http://") || line.starts_with("https://") {
        let bytes = reqwest::blocking::get(line).map_err(|e| e.to_string())?.bytes().map_err(|e| e.to_string())?;
        ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?.decode().map_err(|e| e.to_string())
    } else {
        ImageReader::open(line).map_err(|e| e.to_string())?.decode().map_err(|e| e.to_string())
    }
}

fn process_entry(line: &str, width: u32, height: u32, uploader: &dyn FileUploader, index: usize) {
    match fetch_image(line) {
        Err(e) => eprintln!("[{line}] помилка: {e}"),
        Ok(img) => {
            let resized = img.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
            let name = format!("{index}.png");
            let mut buf = Cursor::new(Vec::new());
            if let Err(e) = resized.write_to(&mut buf, image::ImageFormat::Png) {
                eprintln!("[{line}] помилка кодування: {e}");
                return;
            }
            match uploader.upload(&name, &buf.into_inner()) {
                Ok(()) => println!("[{line}] завантажено як {name}"),
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
        process_entry(line, width, height, uploader.as_ref(), i);
    }
}
