use derive_more::Display;

// --- Newtype патерни для запобігання плутанині в даних ---

/// Унікальний ідентифікатор замовлення
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[display("Замовлення#{}", _0)]
pub struct OrderId(u64);

impl OrderId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Сума в копійках — окремий тип щоб не переплутати з просто числом
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Display)]
#[display("{:.2} грн", *_0 as f64 / 100.0)]
pub struct Price(u64);

impl Price {
    pub fn from_hrn(hrn: u64) -> Self {
        Self(hrn * 100)
    }
}

/// Адреса доставки — окремий тип щоб не переплутати зі звичайним рядком
#[derive(Debug, Clone, Display)]
#[display("{}", _0)]
pub struct Address(String);

impl Address {
    pub fn new(addr: impl Into<String>) -> Self {
        Self(addr.into())
    }
}

/// Трек-номер посилки
#[derive(Debug, Clone, Display)]
#[display("Трек: {}", _0)]
pub struct TrackingCode(String);

impl TrackingCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }
}

// --- Стани замовлення ---

/// Нове замовлення, ще не оплачене
#[derive(Debug)]
pub struct New;

/// Замовлення оплачене
#[derive(Debug)]
pub struct Paid {
    pub price: Price,
}

/// Замовлення передане у доставку
#[derive(Debug)]
pub struct Processing {
    pub price: Price,
    pub address: Address,
    pub tracking: TrackingCode,
}

/// Замовлення доставлене
#[derive(Debug)]
pub struct Delivered {
    pub price: Price,
    pub address: Address,
}

/// Замовлення повернуте після оплати
#[derive(Debug)]
pub struct Refunded {
    pub price: Price,
    pub reason: String,
}

/// Замовлення скасоване
#[derive(Debug)]
pub struct Cancelled {
    pub reason: String,
}

// --- Структура замовлення з параметром стану ---

#[derive(Debug)]
pub struct Order<State> {
    pub id: OrderId,
    pub item: String,
    pub state: State,
}

// Переходи зі стану New
impl Order<New> {
    pub fn new(id: OrderId, item: impl Into<String>) -> Self {
        let item = item.into();
        println!("[{}] Створено замовлення: «{}»", id, item);
        Self { id, item, state: New }
    }

    pub fn pay(self, price: Price) -> Order<Paid> {
        println!("[{}] Оплата отримана: {}", self.id, price);
        Order {
            id: self.id,
            item: self.item,
            state: Paid { price },
        }
    }

    pub fn cancel(self, reason: impl Into<String>) -> Order<Cancelled> {
        let reason = reason.into();
        println!("[{}] Скасовано до оплати: {}", self.id, reason);
        Order {
            id: self.id,
            item: self.item,
            state: Cancelled { reason },
        }
    }
}

// Переходи зі стану Paid
impl Order<Paid> {
    pub fn process(
        self,
        address: Address,
        tracking: TrackingCode,
    ) -> Order<Processing> {
        println!(
            "[{}] Передано в доставку → {} ({})",
            self.id, address, tracking
        );
        Order {
            id: self.id,
            item: self.item,
            state: Processing {
                price: self.state.price,
                address,
                tracking,
            },
        }
    }

    pub fn refund(self, reason: impl Into<String>) -> Order<Refunded> {
        let reason = reason.into();
        println!(
            "[{}] Повернення коштів {} (причина: {})",
            self.id, self.state.price, reason
        );
        Order {
            id: self.id,
            item: self.item,
            state: Refunded {
                price: self.state.price,
                reason,
            },
        }
    }
}

// Переходи зі стану Processing
impl Order<Processing> {
    pub fn deliver(self) -> Order<Delivered> {
        println!(
            "[{}] Доставлено за адресою: {}",
            self.id, self.state.address
        );
        Order {
            id: self.id,
            item: self.item,
            state: Delivered {
                price: self.state.price,
                address: self.state.address,
            },
        }
    }
}

// Підсумки для фінальних станів
impl Order<Delivered> {
    pub fn summary(&self) {
        println!(
            "[{}] ✅ Виконано: «{}» → {}, сплачено {}",
            self.id, self.item, self.state.address, self.state.price
        );
    }
}

impl Order<Refunded> {
    pub fn summary(&self) {
        println!(
            "[{}] 💸 Повернення: «{}», повернуто {} ({})",
            self.id, self.item, self.state.price, self.state.reason
        );
    }
}

impl Order<Cancelled> {
    pub fn summary(&self) {
        println!(
            "[{}] ❌ Скасовано: «{}» — {}",
            self.id, self.item, self.state.reason
        );
    }
}

fn main() {
    println!("=== Сценарій 1: успішна доставка ===");
    let order = Order::new(OrderId::new(1), "Навушники Sony WH-1000XM5");
    let order = order.pay(Price::from_hrn(8999));
    let order = order.process(
        Address::new("вул. Хрещатик 1, Київ"),
        TrackingCode::new("UA123456789"),
    );
    let order = order.deliver();
    order.summary();

    println!();
    println!("=== Сценарій 2: скасування до оплати ===");
    let order = Order::new(OrderId::new(2), "Механічна клавіатура Keychron K2");
    let order = order.cancel("клієнт передумав");
    order.summary();

    println!();
    println!("=== Сценарій 3: повернення після оплати ===");
    let order = Order::new(OrderId::new(3), "Монітор LG 27UK850");
    let order = order.pay(Price::from_hrn(18500));
    let order = order.refund("товар виявився бракованим");
    order.summary();

    // Наступний рядок НЕ скомпілюється — компілятор забороняє
    // перейти у Processing оминувши стан Paid:
    //
    // let bad = Order::new(OrderId::new(4), "test");
    // bad.process(Address::new("Київ"), TrackingCode::new("X")); // ← помилка компіляції!
}
