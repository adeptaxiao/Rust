# Самостійна робота 3
## Тема: Багатопотокові застосунки та гарантії Rust.

---

## 1. Проблеми багатопотокового виконання та їх діагностика

### Гонки ресурсів (Race Conditions)

**Data race** — коли два потоки одночасно звертаються до однієї ділянки пам'яті, і хоча б один з них виконує запис, без синхронізації. Rust повністю унеможливлює data races на рівні системи типів через `Send`/`Sync` трейти.

**Race condition** — ширша проблема: коректність програми залежить від непередбачуваного порядку виконання операцій, навіть якщо кожна операція синхронізована. Rust не захищає від цього:

```rust
// TOCTOU (Time-Of-Check-Time-Of-Use) — класична race condition
let val = *counter.lock().unwrap();   // перевірка
// інший потік може змінити значення тут
*counter.lock().unwrap() = val + 1;   // використання
// Правильно: тримати guard протягом усієї операції
```

**Starvation (голодування)** — потік постійно не отримує доступ до ресурсу бо інші потоки завжди мають пріоритет. Типово для `RwLock` при великій кількості читачів — писач може чекати нескінченно.

### Взаємне блокування (Deadlock)

Виникає коли два або більше потоки чекають один на одного у циклі:

```rust
// Потік 1: захоплює mutex_a потім mutex_b
// Потік 2: захоплює mutex_b потім mutex_a
// → обидва чекають вічно
let _a = mutex_a.lock().unwrap();
let _b = mutex_b.lock().unwrap(); // deadlock якщо потік 2 тримає mutex_b
```

**Методи запобігання дедлокам:**
- Завжди захоплювати мютекси в одному порядку
- Використовувати `try_lock()` з відступом замість `lock()`
- Мінімізувати час утримання блокувань
- Використовувати `tokio::sync::Mutex` в async коді замість `std::sync::Mutex`

### Методи діагностики

**Miri** — інтерпретатор MIR (Mid-level Intermediate Representation) що виявляє undefined behavior і деякі race conditions під час виконання:

```bash
cargo +nightly miri test
```

Miri виявляє: data races, use-after-free, неправильне використання `unsafe`, некоректне вирівнювання.

**ThreadSanitizer (TSan)** — інструментує бінарник для виявлення data races. Підтримується в Rust через nightly:

```bash
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test --target x86_64-unknown-linux-gnu
```

**`loom`** — крейт для тестування конкурентного коду шляхом систематичного перебору всіх можливих порядків виконання потоків. Використовується для тестування lock-free структур даних:

```rust
loom::model(|| {
    let data = Arc::new(loom::sync::Mutex::new(0));
    // loom перевіряє всі можливі interleaving потоків
});
```

**`parking_lot`** — замінник стандартних `Mutex`/`RwLock` з кращою діагностикою deadlock в debug режимі.

---

## 2. Неблокуючі структури даних

### Принцип роботи

Неблокуючі (lock-free) структури даних уникають мютексів і замість цього використовують атомарні операції процесора, зокрема **CAS (Compare-And-Swap)**:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

let counter = AtomicUsize::new(0);
// CAS: якщо значення == 0, замінити на 1
counter.compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed);
```

CAS є атомарною апаратною інструкцією: процесор гарантує що перевірка і заміна відбуваються нероздільно.

### Рівні гарантій

- **Lock-free** — хоча б один потік завжди прогресує. Без deadlock, але можливе starvation
- **Wait-free** — кожен потік завершує операцію за обмежену кількість кроків. Найсильніша гарантія
- **Obstruction-free** — потік прогресує якщо виконується ізольовано

### Реалізації в Rust std

`std::sync::atomic::*` — атомарні типи (`AtomicBool`, `AtomicUsize`, `AtomicPtr` тощо). Основа для всіх lock-free структур.

`std::sync::Arc` — підрахунок посилань через атомарні операції.

### Крейти екосистеми

**`crossbeam`** — найпопулярніший крейт для lock-free структур:
- `crossbeam::queue::SegQueue` — lock-free MPMC черга (multiple producer, multiple consumer)
- `crossbeam::queue::ArrayQueue` — lock-free черга обмеженого розміру
- `crossbeam::deque` — work-stealing deque (використовується в rayon)

**`dashmap`** — конкурентна HashMap без зовнішнього Mutex. Розбиває карту на shard-и, кожен зі своїм RwLock, що зменшує contention.

**`flurry`** — port Java ConcurrentHashMap для Rust.

### Переваги неблокуючих структур

- Відсутній deadlock (немає мютексів)
- Краща масштабованість при великій кількості потоків
- Менші затримки у worst case (немає чекання на звільнення блокування)
- Потоки не блокуються при збої іншого потоку

### Недоліки

- Складніше реалізувати коректно
- ABA-проблема: значення змінилося з A на B і назад на A, CAS не помічає цього
- Може бути повільніше за мютекс при низькому contention через overhead CAS retry

---

## 3. Бібліотеки для роботи з каналами

### `std::sync::mpsc`

Вбудований канал стандартної бібліотеки. **MPSC** — multiple producer, single consumer.

```rust
use std::sync::mpsc;
let (tx, rx) = mpsc::channel(); // необмежений
let (tx, rx) = mpsc::sync_channel(32); // обмежений, блокує producer
```

Переваги: не потрібні зовнішні залежності. Недоліки: лише один consumer, обмежена продуктивність.

### `crossbeam-channel`

Найпопулярніша альтернатива. **MPMC** — multiple producer, multiple consumer.

```rust
use crossbeam_channel::{bounded, unbounded};
let (tx, rx) = bounded(100);  // обмежений канал
let (tx, rx) = unbounded();   // необмежений
```

Підтримує `select!` — очікування на кількох каналах одночасно:

```rust
crossbeam_channel::select! {
    recv(rx1) -> msg => println!("з rx1: {:?}", msg),
    recv(rx2) -> msg => println!("з rx2: {:?}", msg),
}
```

Продуктивніший за std::mpsc завдяки lock-free реалізації. Підтримує `after()` та `tick()` для таймерів.

### `tokio::sync` канали

Для асинхронного коду tokio надає кілька типів каналів:

**`mpsc`** — many producers, one consumer. `send()` є async, не блокує потік:

```rust
let (tx, mut rx) = tokio::sync::mpsc::channel(32);
tokio::spawn(async move { tx.send("hello").await.unwrap(); });
while let Some(msg) = rx.recv().await { println!("{msg}"); }
```

**`broadcast`** — один producer, багато consumers. Кожен consumer отримує копію кожного повідомлення. Використовується для pub/sub:

```rust
let (tx, mut rx1) = tokio::sync::broadcast::channel(16);
let mut rx2 = tx.subscribe();
```

**`watch`** — зберігає лише останнє значення. Consumer завжди бачить актуальний стан:

```rust
let (tx, rx) = tokio::sync::watch::channel(0i32);
// Корисно для передачі конфігурації або стану shutdown
```

**`oneshot`** — передає рівно одне значення, після чого канал закривається. Використовується для відповідей на запити:

```rust
let (tx, rx) = tokio::sync::oneshot::channel();
tokio::spawn(async move { tx.send(42).unwrap(); });
let result = rx.await.unwrap();
```

### Порівняльна таблиця

| Бібліотека | Тип | Async | MPMC | Продуктивність |
|------------|-----|-------|------|----------------|
| `std::mpsc` | MPSC | Ні | Ні | Середня |
| `crossbeam-channel` | MPMC | Ні | Так | Висока |
| `tokio::sync::mpsc` | MPSC | Так | Ні | Висока (async) |
| `tokio::sync::broadcast` | Broadcast | Так | Так | Середня |
| `tokio::sync::watch` | Останнє значення | Так | Так | Висока |
| `flume` | MPMC | Обидва | Так | Висока |

### `flume`

Універсальний канал що підтримує і sync і async API одночасно:

```rust
let (tx, rx) = flume::bounded(32);
tx.send("sync")?;             // синхронний send
tx.send_async("async").await?; // асинхронний send
```

Особливо корисний для бібліотек що мають підтримувати обидва режими.
