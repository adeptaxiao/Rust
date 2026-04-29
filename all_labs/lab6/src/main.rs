//! Бенчмарк: порівняння std::Arc+Mutex vs MyArc+MyMutex.
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use lab6::my_arc::MyArc;
use lab6::my_mutex::MyMutex;

const THREADS: usize = 8;
const ITERS: usize = 200_000;
const CLONES: usize = 1_000_000;

fn bench_std_mutex() -> u128 {
    let counter = Arc::new(Mutex::new(0u64));
    let t = Instant::now();
    let handles: Vec<_> = (0..THREADS).map(|_| {
        let c = Arc::clone(&counter);
        thread::spawn(move || { for _ in 0..ITERS { *c.lock().unwrap() += 1; } })
    }).collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    assert_eq!(*counter.lock().unwrap(), (THREADS * ITERS) as u64);
    t.elapsed().as_micros()
}

fn bench_my_mutex() -> u128 {
    let counter = MyArc::new(MyMutex::new(0u64));
    let t = Instant::now();
    let handles: Vec<_> = (0..THREADS).map(|_| {
        let c = counter.clone();
        thread::spawn(move || { for _ in 0..ITERS { *c.lock() += 1; } })
    }).collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    assert_eq!(*counter.lock(), (THREADS * ITERS) as u64);
    t.elapsed().as_micros()
}

fn bench_std_arc() -> u128 {
    let arc = Arc::new(42u64);
    let t = Instant::now();
    let handles: Vec<_> = (0..THREADS).map(|_| {
        let a = Arc::clone(&arc);
        thread::spawn(move || { for _ in 0..CLONES { let _c = Arc::clone(&a); } })
    }).collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    t.elapsed().as_micros()
}

fn bench_my_arc() -> u128 {
    let arc = MyArc::new(42u64);
    let t = Instant::now();
    let handles: Vec<_> = (0..THREADS).map(|_| {
        let a = arc.clone();
        thread::spawn(move || { for _ in 0..CLONES { let _c = a.clone(); } })
    }).collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    t.elapsed().as_micros()
}

fn diff(custom: u128, std: u128) -> String {
    let pct = (custom as f64 / std as f64 - 1.0) * 100.0;
    if pct >= 0.0 { format!("власна повільніша на {pct:.1}%") } else { format!("власна швидша на {:.1}%", -pct) }
}

fn main() {
    println!("Потоків: {THREADS}, ітерацій: {ITERS}, клонувань Arc: {CLONES}\n");
    let sm = bench_std_mutex(); let mm = bench_my_mutex();
    println!("Mutex:\n  std::Mutex : {sm} мкс\n  MyMutex    : {mm} мкс\n  {}\n", diff(mm, sm));
    let sa = bench_std_arc(); let ma = bench_my_arc();
    println!("Arc clone:\n  std::Arc   : {sa} мкс\n  MyArc      : {ma} мкс\n  {}", diff(ma, sa));
}
