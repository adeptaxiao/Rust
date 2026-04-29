# Рефакторинг — Лабораторна робота 9

## Зміни відносно lab8

### 1. `buffer_unordered` → `JoinSet` + `Semaphore`

**До (lab7/lab8):**
```rust
stream::iter(lines.into_iter().map(|(i,line)| { ... }))
    .buffer_unordered(args.concurrency)
    .for_each(|()| async {}).await;
```

**Після (lab9):**
```rust
let sem = Arc::new(tokio::sync::Semaphore::new(args.concurrency));
let mut set = JoinSet::new();
for (i, line) in lines.into_iter().enumerate() {
    let permit = Arc::clone(&sem).acquire_owned().await?;
    set.spawn(async move { let _p = permit; process_entry(...).await; });
}
while let Some(r) = set.join_next().await { ... }
```

**Причина:** `JoinSet` + `Semaphore` дає більш явний контроль над кількістю паралельних задач і дозволяє обробляти результати по мірі завершення. `buffer_unordered` простіший але менш гнучкий.

### 2. Streaming завантаження URL

**До:** `resp.bytes().await?.to_vec()` — чекаємо повної відповіді в пам'яті.

**Після:** `resp.bytes_stream()` + `try_next()` — потокове читання чанками з `futures::TryStreamExt`.

**Причина:** Для великих зображень потокове читання зменшує пікове споживання пам'яті.

### 3. `pool_max_idle_per_host`

Додано `.pool_max_idle_per_host(args.concurrency)` до `reqwest::Client` — дозволяє тримати більше відкритих з'єднань у пулі для паралельних запитів до одного хосту.
