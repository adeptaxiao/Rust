# Самостійна робота 4
## Тема: Асинхронне програмування в Rust.

---

## 1. Асинхронні рантайми Rust окрім tokio

### Що таке async runtime

Rust надає лише базові абстракції (`Future`, `async`/`await`, `Poll`) — але не рантайм. Рантайм виконує три функції: планування задач, I/O event loop, та timer wheel. Вибір рантайму залежить від задачі.

---

### `async-std`

Мета: API максимально близький до `std`, але асинхронний. Кожна функція зі стандартної бібліотеки має async-аналог в тому ж місці простору імен.

```rust
use async_std::fs;
use async_std::task;

task::block_on(async {
    let content = fs::read_to_string("file.txt").await.unwrap();
    println!("{content}");
});
```

**Переваги:**
- Легший поріг входу для тих хто знає std
- Автоматичний вибір кількості потоків
- Вбудований executor для тестів (`#[async_std::test]`)

**Недоліки:**
- Значно менша екосистема ніж у tokio
- Повільніший розвиток, менша спільнота
- Деякі API відстають від tokio за можливостями

---

### `smol`

Мінімалістичний рантайм (~1000 рядків коду). Ціль — найменший можливий рантайм без зайвих залежностей.

```rust
fn main() {
    smol::block_on(async {
        let listener = smol::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            smol::spawn(handle(stream)).detach();
        }
    });
}
```

**Переваги:**
- Дуже маленький розмір бінарника
- Простий у розумінні та аудиті
- Добра основа для вбудованих систем

**Недоліки:**
- Мала екосистема
- Відсутній scheduler з пріоритетами
- Не підходить для складних production систем

---

### `glommio`

Рантайм на основі `io_uring` (Linux 5.1+). Використовує thread-per-core модель: кожне ядро має власний executor без синхронізації між ядрами.

```rust
use glommio::{LocalExecutorBuilder, Placement};

LocalExecutorBuilder::new(Placement::Fixed(0))
    .spawn(|| async move {
        glommio::timer::sleep(Duration::from_secs(1)).await;
        println!("done");
    }).unwrap().join().unwrap();
```

**Переваги:**
- Надзвичайно висока продуктивність для I/O workloads
- Zero-copy I/O через io_uring
- Відсутня синхронізація між потоками (shared-nothing)

**Недоліки:**
- Тільки Linux (io_uring)
- thread-per-core модель вимагає переосмислення архітектури
- Задачі не можна переміщати між потоками

---

### `embassy`

Рантайм для мікроконтролерів та вбудованих систем (`no_std`). Пише async/await код для ARM Cortex-M, RISC-V тощо.

```rust
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let mut led = Output::new(p.PA5, Level::High, Speed::Low);
    loop {
        led.toggle();
        Timer::after_millis(500).await;
    }
}
```

**Переваги:**
- Єдиний рантайм для embedded Rust
- Zero allocations — повністю на стеку
- Дуже малий overhead

**Недоліки:**
- Тільки для embedded, не підходить для сервісів
- Обмежені можливості порівняно з tokio

---

### Порівняльна таблиця

| Рантайм | Платформи | Парадигма | Розмір | Підходить для |
|---------|-----------|-----------|--------|---------------|
| **tokio** | Всі | Multi-thread, work-stealing | Великий | Сервіси, CLI, мережа |
| **async-std** | Всі | Multi-thread | Середній | Застосунки, схожі на std |
| **smol** | Всі | Multi-thread | Мінімальний | Бібліотеки, прості сервіси |
| **glommio** | Linux | Thread-per-core | Середній | Висока I/O продуктивність |
| **embassy** | Embedded | Cooperative, no_std | Мінімальний | Мікроконтролери |

---

## 2. Скасування асинхронних задач в Rust

### Чому це складно

У Rust `Future` є ледачою — вона нічого не робить поки її не poll-ять. Скасування відбувається шляхом **drop** футури: executor просто перестає її poll-ити і дропає. Це автоматично, але має наслідки.

**Проблема 1 — Drop у будь-якій точці await:**

```rust
async fn process() {
    let file = File::open("data.txt").await.unwrap(); // точка 1
    let data = read_data(&file).await.unwrap();        // точка 2 — може бути скасовано тут
    write_result(data).await.unwrap();                 // точка 3
    // якщо скасовано між 2 і 3 — файл записаний частково
}
```

Якщо задачу скасують між будь-якими двома `.await`, виконання зупиняється в цій точці. Деструктори (`Drop`) будуть викликані для всього що вже ініціалізоване.

**Проблема 2 — не-cancel-safe функції:**

Деякі операції не є cancel-safe: якщо їх перервати, стан може бути некоректним. Наприклад `AsyncReadExt::read_exact` — якщо скасувати після часткового читання, дані втрачені.

```rust
// НЕ cancel-safe:
tokio::select! {
    _ = socket.read_exact(&mut buf) => { ... } // небезпечно в select!
    _ = timeout => { ... }
}
```

**Проблема 3 — відсутній механізм сповіщення:**

На відміну від Go (context.Context) або Java (Thread.interrupt()), Rust не має вбудованого механізму сповістити задачу про скасування. Задача просто дропається.

### Способи скасування

**`JoinHandle::abort()` (tokio)**

Найпростіший спосіб. Tokio надсилає сигнал скасування задачі яка виконується через `spawn`:

```rust
let handle = tokio::spawn(async {
    tokio::time::sleep(Duration::from_secs(100)).await;
});
handle.abort(); // задача буде скасована при наступному poll
let result = handle.await; // Err(JoinError { cancelled: true })
```

Працює лише для задач запущених через `tokio::spawn`.

**`CancellationToken` (tokio-util)**

Найбільш ідіоматичний підхід для graceful cancellation:

```rust
use tokio_util::sync::CancellationToken;

let token = CancellationToken::new();
let child_token = token.child_token();

tokio::spawn(async move {
    tokio::select! {
        _ = child_token.cancelled() => {
            println!("скасовано, cleanup...");
        }
        result = do_work() => {
            println!("завершено: {:?}", result);
        }
    }
});

token.cancel(); // сповіщає всі child tokens
```

`CancellationToken` можна клонувати і передавати у вкладені задачі. `child_token()` дозволяє будувати дерево скасування.

**`tokio::select!` з timeout**

```rust
tokio::select! {
    result = long_operation() => result,
    _ = tokio::time::sleep(Duration::from_secs(5)) => {
        Err(anyhow::anyhow!("timeout"))
    }
}
```

**`futures::future::select` та `race`**

```rust
use futures::future;
let result = future::select(operation1(), operation2()).await;
// повертає першу що завершилась, дропає другу
```

### Рекомендації

- Документувати чи є функція cancel-safe
- Використовувати `CancellationToken` для graceful shutdown
- При роботі з `select!` перевіряти cancel-safety кожної гілки
- Використовувати `tokio::time::timeout` для операцій з дедлайном

---

## 3. Модель акторів

### Що таке модель акторів

Модель акторів — парадигма конкурентного програмування де основна одиниця — **актор**: ізольований об'єкт зі своїм станом, mailbox (поштовою скринькою) і поведінкою. Актори взаємодіють виключно через передачу повідомлень. Стан актора недоступний ззовні.

Три основних принципи:
1. Актори обробляють повідомлення послідовно (без внутрішніх гонок)
2. Актори можуть створювати нових акторів
3. Актори спілкуються лише через повідомлення

### Реалізації в Rust

**`actix`** — найвідоміший actor framework, основа web-фреймворку `actix-web`:

```rust
use actix::prelude::*;

struct Counter { count: usize }
impl Actor for Counter { type Context = Context<Self>; }

#[derive(Message)] #[rtype(result = "usize")]
struct Increment;

impl Handler<Increment> for Counter {
    type Result = usize;
    fn handle(&mut self, _: Increment, _: &mut Context<Self>) -> usize {
        self.count += 1;
        self.count
    }
}

let addr = Counter { count: 0 }.start();
let result = addr.send(Increment).await.unwrap(); // 1
```

**`xactor`** — легший альтернатив actix.

**`kameo`** — сучасний actor framework побудований поверх tokio з фокусом на простоті.

### Порівняння з async/await

| Характеристика | Async/await | Модель акторів |
|----------------|------------|----------------|
| **Стан** | Спільний через Arc/Mutex | Ізольований у кожному акторі |
| **Комунікація** | Канали, shared memory | Повідомлення через mailbox |
| **Ізоляція помилок** | Ручна | Вбудована (supervisor) |
| **Складність** | Менша для простих задач | Більша архітектурна складність |
| **Масштабованість** | Горизонтальна через tokio | Природна через розподіл акторів |

### Спільні риси

- Обидва підходи побудовані поверх async Rust (actix використовує tokio)
- Обидва уникають блокування потоків
- В обох задачі/актори є легковажними — можна мати тисячі
- `Future` і актор — обидва обробляють одну "подію" за раз

### Відмінні риси

- Актор має **адресу** (`Addr<T>`) — постійний ідентифікатор. Async задача не має постійної адреси
- Актор **інкапсулює стан**: зовнішній код не може напряму звернутись до стану актора
- Actix підтримує **supervisor**: якщо актор падає, supervisor може перезапустити його (як в Erlang/Elixir)
- Async/await природніше для request/response потоків, актори — для довгоживучих сутностей зі станом

### Коли використовувати модель акторів

- Компоненти з чітко визначеним станом і поведінкою (сесія користувача, з'єднання з БД)
- Системи де потрібна ізоляція відмов і перезапуск компонентів
- Коли природна модель предметної області — "об'єкти що спілкуються"
- Мікросервіси або розподілені системи де потрібна location transparency
