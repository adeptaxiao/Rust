//! CLI програма для зміни розміру зображень із файлу або URL.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use clap::Parser;
use image::io::Reader as ImageReader;

/// Аргументи командного рядка
#[derive(Parser)]
#[command(name = "image_editor", about = "Змінює розмір зображень із списку файлів/URL")]
struct Args {
    /// Шлях до файлу зі списком зображень (по рядку)
    #[arg(long)]
    files: PathBuf,

    /// Новий розмір у форматі WIDTHxHEIGHT, наприклад 800x600
    #[arg(long, value_parser = parse_dimensions)]
    resize: (u32, u32),
}

/// Розбирає рядок "WIDTHxHEIGHT" у пару чисел.
fn parse_dimensions(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x').ok_or("очікується формат WIDTHxHEIGHT")?;
    let w = w.parse::<u32>().map_err(|e| e.to_string())?;
    let h = h.parse::<u32>().map_err(|e| e.to_string())?;
    Ok((w, h))
}

/// Повертає вихідну директорію зі змінної середовища MYME_FILES_PATH.
fn output_dir() -> PathBuf {
    PathBuf::from(std::env::var("MYME_FILES_PATH").unwrap_or_else(|_| "out".to_string()))
}

/// Читає вміст файлу, обробляючи BOM для UTF-8 та UTF-16.
fn read_text_file(path: &Path) -> String {
    let raw = fs::read(path).expect("не вдалося прочитати файл");
    decode_text(&raw)
}

/// Декодує байти у рядок, підтримуючи UTF-8 BOM, UTF-16 LE та UTF-16 BE.
fn decode_text(raw: &[u8]) -> String {
    if raw.starts_with(&[0xFF, 0xFE]) {
        // UTF-16 LE
        let words: Vec<u16> = raw[2..]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16_lossy(&words).to_string()
    } else if raw.starts_with(&[0xFE, 0xFF]) {
        // UTF-16 BE
        let words: Vec<u16> = raw[2..]
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16_lossy(&words).to_string()
    } else if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 BOM
        String::from_utf8_lossy(&raw[3..]).to_string()
    } else {
        String::from_utf8_lossy(raw).to_string()
    }
}

/// Завантажує зображення з URL.
fn fetch_image_url(url: &str) -> Result<image::DynamicImage, String> {
    let bytes = reqwest::blocking::get(url)
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())
}

/// Відкриває зображення з локального шляху.
fn open_image_file(path: &str) -> Result<image::DynamicImage, String> {
    ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())
}

/// Формує ім'я вихідного файлу на основі індексу рядка.
fn make_output_name(index: usize) -> String {
    format!("{index}.png")
}

/// Обробляє один рядок: завантажує, змінює розмір і зберігає зображення.
fn process_entry(line: &str, width: u32, height: u32, out_dir: &Path, index: usize) {
    let img_result = if line.starts_with("http://") || line.starts_with("https://") {
        fetch_image_url(line)
    } else {
        open_image_file(line)
    };

    match img_result {
        Err(e) => eprintln!("[{line}] помилка: {e}"),
        Ok(img) => {
            let resized = img.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
            let out_path = out_dir.join(make_output_name(index));
            match resized.save(&out_path) {
                Ok(()) => println!("[{line}] збережено у {}", out_path.display()),
                Err(e) => eprintln!("[{line}] помилка збереження: {e}"),
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let (width, height) = args.resize;
    let out_dir = output_dir();

    fs::create_dir_all(&out_dir).expect("не вдалося створити вихідну директорію");

    let content = read_text_file(&args.files);

    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        process_entry(line, width, height, &out_dir, i);
    }
}
