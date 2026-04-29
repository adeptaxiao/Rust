# Рефакторинг pract7 → pract9

## Що було змінено і чому

### 1. `tokio::spawn` → `stream::buffer_unordered`

**До (pract7):**
```rust
let tasks: Vec<_> = urls.iter().enumerate()
    .map(|(i, url)| tokio::spawn(download(client.clone(), url, i)))
    .collect();
for task in tasks { task.await?; }
```

**Після (pract9):**
```rust
stream::iter(urls.iter().enumerate())
    .map(|(i, url)| download(&client, url, i))
    .buffer_unordered(cli.concurrency)
    .collect::<Vec<_>>()
    .await;
```

**Причина:** `tokio::spawn` запускав усі задачі одразу без обмеження кількості паралельних запитів. При великому списку URL це може призвести до вичерпання файлових дескрипторів або перевантаження сервера. `buffer_unordered` з крейту `futures` дозволяє обмежити кількість одночасних запитів (`concurrency`) і є більш ідіоматичним рішенням для таких задач.

### 2. `client.clone()` → `&client`

`reqwest::Client` вже використовує `Arc` внутрішньо, тому передача `&client` у `async fn` (без spawn) є коректною і уникає зайвих клонувань.

### 3. Параметр `--concurrency` замість `--max-threads`

Термін `concurrency` точніше відображає суть: ми обмежуємо не кількість OS-потоків, а кількість одночасно виконуваних async-задач.

## Висновок

Основне покращення — контроль над паралелізмом через `buffer_unordered`. Решта логіки залишилась незмінною, оскільки pract7 вже використовувала async/await коректно.
