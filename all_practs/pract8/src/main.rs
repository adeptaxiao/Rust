use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

// ─── MeasurableFuture ────────────────────────────────────────────────────────

/// Обгортка над Future що вимірює час його виконання.
struct MeasurableFuture<Fut> {
    inner_future: Fut,
    started_at: Option<Instant>,
}

impl<Fut> MeasurableFuture<Fut> {
    fn new(inner_future: Fut) -> Self {
        Self {
            inner_future,
            started_at: None,
        }
    }
}

impl<Fut: Future> Future for MeasurableFuture<Fut> {
    type Output = (Fut::Output, Duration);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: ми не переміщуємо inner_future після закріплення
        let this = unsafe { self.get_unchecked_mut() };

        // Фіксуємо час першого poll
        if this.started_at.is_none() {
            this.started_at = Some(Instant::now());
        }

        // Делегуємо poll до внутрішнього Future
        let inner = unsafe { Pin::new_unchecked(&mut this.inner_future) };
        match inner.poll(cx) {
            Poll::Ready(value) => {
                let elapsed = this.started_at.unwrap().elapsed();
                println!("⏱  Час виконання: {:?}", elapsed);
                Poll::Ready((value, elapsed))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

// ─── DelayFuture ─────────────────────────────────────────────────────────────

/// Future що стає готовою через задану кількість мілісекунд.
/// Не блокує потік. Не використовує сторонніх крейтів.
struct DelayFuture {
    deadline: Instant,
    /// Зберігаємо waker щоб разбудити задачу через spawn_blocking
    waker_registered: bool,
}

impl DelayFuture {
    fn new(millis: u64) -> Self {
        Self {
            deadline: Instant::now() + Duration::from_millis(millis),
            waker_registered: false,
        }
    }
}

impl Future for DelayFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.deadline {
            return Poll::Ready(());
        }

        // Якщо ще не реєстрували waker — запускаємо окремий потік що розбудить нас
        if !self.waker_registered {
            self.waker_registered = true;
            let waker = cx.waker().clone();
            let deadline = self.deadline;
            std::thread::spawn(move || {
                let now = Instant::now();
                if deadline > now {
                    std::thread::sleep(deadline - now);
                }
                waker.wake();
            });
        }

        Poll::Pending
    }
}

// ─── main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    println!("=== Тест MeasurableFuture ===");

    // Вимірюємо просту async-задачу
    let task = async {
        tokio::time::sleep(Duration::from_millis(250)).await;
        println!("Задача виконана!");
        42u32
    };

    let (result, elapsed) = MeasurableFuture::new(task).await;
    println!("Результат: {}, час: {:?}", result, elapsed);

    println!();
    println!("=== Тест DelayFuture (300 мс без блокування потоку) ===");

    let start = Instant::now();
    DelayFuture::new(300).await;
    println!("DelayFuture завершилась через {:?}", start.elapsed());

    println!();
    println!("=== Вимірювання DelayFuture через MeasurableFuture ===");
    let ((), elapsed) = MeasurableFuture::new(DelayFuture::new(150)).await;
    println!("Затримка 150 мс, фактично: {:?}", elapsed);
}
