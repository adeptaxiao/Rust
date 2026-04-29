/// Завдання 2: рекурсивне читання файлів + AES-256-GCM шифрування + лічильник

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::rand_core::RngCore;
use std::env;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use walkdir::WalkDir;

fn main() {
    let dir = env::args().nth(1).unwrap_or_else(|| "test_dir".to_string());

    let (tx, rx) = mpsc::sync_channel::<(PathBuf, Vec<u8>)>(32);
    let rx = Arc::new(Mutex::new(rx));

    // Глобальний лічильник оброблених файлів
    let counter = Arc::new(Mutex::new(0usize));

    // Потік-читач: рекурсивно обходить директорію
    let dir_clone = dir.clone();
    thread::spawn(move || {
        for entry in WalkDir::new(&dir_clone)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path().to_path_buf();
            match std::fs::read(&path) {
                Ok(data) => {
                    tx.send((path, data)).unwrap();
                }
                Err(e) => eprintln!("Помилка читання {:?}: {}", path, e),
            }
        }
    });

    // 3 потоки-шифрувальники
    let mut handles = vec![];
    for worker_id in 0..3 {
        let rx = Arc::clone(&rx);
        let counter = Arc::clone(&counter);

        let h = thread::spawn(move || {
            let key = Aes256Gcm::generate_key(&mut OsRng);
            let cipher = Aes256Gcm::new(&key);

            loop {
                let item = {
                    let lock = rx.lock().unwrap();
                    lock.recv().ok()
                };
                let (path, data) = match item {
                    Some(v) => v,
                    None => break,
                };

                let mut nonce_bytes = [0u8; 12];
                OsRng.fill_bytes(&mut nonce_bytes);
                let nonce = Nonce::from_slice(&nonce_bytes);

                match cipher.encrypt(nonce, data.as_ref()) {
                    Ok(encrypted) => {
                        let out_path = path.with_extension("data");
                        // Зберігаємо: nonce(12 байт) + зашифровані дані
                        let mut out = nonce_bytes.to_vec();
                        out.extend(encrypted);
                        if let Err(e) = std::fs::write(&out_path, &out) {
                            eprintln!("Помилка запису {:?}: {}", out_path, e);
                        } else {
                            let mut cnt = counter.lock().unwrap();
                            *cnt += 1;
                            println!(
                                "[worker {}] Зашифровано: {:?} → {:?}",
                                worker_id, path, out_path
                            );
                        }
                    }
                    Err(e) => eprintln!("Помилка шифрування {:?}: {}", path, e),
                }
            }
        });
        handles.push(h);
    }

    // Потік-монітор: слідкує за лічильником
    let counter_mon = Arc::clone(&counter);
    let monitor = thread::spawn(move || {
        let mut last = 0;
        loop {
            thread::sleep(std::time::Duration::from_millis(100));
            let current = *counter_mon.lock().unwrap();
            if current != last {
                println!("📊 Оброблено файлів: {}", current);
                last = current;
            }
            // Завершуємо коли всі воркери вже завершились і лічильник не змінюється
            if last > 0 {
                thread::sleep(std::time::Duration::from_millis(300));
                let after = *counter_mon.lock().unwrap();
                if after == last {
                    break;
                }
            }
        }
    });

    for h in handles {
        h.join().unwrap();
    }
    monitor.join().unwrap();

    println!("✅ Готово. Всього оброблено: {}", *counter.lock().unwrap());
}
