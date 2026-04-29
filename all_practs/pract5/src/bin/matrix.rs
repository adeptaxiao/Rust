/// Завдання 1: генерація матриці 4096x4096, паралельне підсумовування через канали + rayon

use rayon::prelude::*;
use std::sync::mpsc;
use std::thread;

const SIZE: usize = 4096;

fn main() {
    let (tx1, rx1) = mpsc::channel::<Vec<Vec<i32>>>();
    let (tx2, rx2) = mpsc::channel::<Vec<Vec<i32>>>();

    // Потік-генератор: генерує матрицю і надсилає копію двом споживачам
    thread::spawn(move || {
        println!("Генерую матрицю {}x{}...", SIZE, SIZE);
        let matrix: Vec<Vec<i32>> = (0..SIZE)
            .map(|i| (0..SIZE).map(|j| ((i + j) % 100) as i32).collect())
            .collect();
        tx1.send(matrix.clone()).unwrap();
        tx2.send(matrix).unwrap();
        println!("Матрицю надіслано двом потокам");
    });

    // Потік 1: сума парних рядків
    let h1 = thread::spawn(move || {
        let matrix = rx1.recv().unwrap();
        let sum: i64 = matrix
            .par_iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, row)| row.par_iter().map(|&x| x as i64).sum::<i64>())
            .sum();
        println!("Потік 1 (парні рядки): сума = {}", sum);
        sum
    });

    // Потік 2: сума непарних рядків
    let h2 = thread::spawn(move || {
        let matrix = rx2.recv().unwrap();
        let sum: i64 = matrix
            .par_iter()
            .enumerate()
            .filter(|(i, _)| i % 2 != 0)
            .map(|(_, row)| row.par_iter().map(|&x| x as i64).sum::<i64>())
            .sum();
        println!("Потік 2 (непарні рядки): сума = {}", sum);
        sum
    });

    let sum1 = h1.join().unwrap();
    let sum2 = h2.join().unwrap();
    println!("Загальна сума матриці: {}", sum1 + sum2);
}
