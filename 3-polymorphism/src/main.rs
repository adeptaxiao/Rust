use std::borrow::Cow;
use std::collections::HashMap;

pub trait Storage<K, V> {
    fn set(&mut self, key: K, val: V);
    fn get(&self, key: &K) -> Option<&V>;
    fn remove(&mut self, key: &K) -> Option<V>;
}

pub struct User {
    pub id: u64,
    pub email: Cow<'static, str>,
    pub activated: bool,
}

pub struct UserRepositoryStatic<S: Storage<u64, User>> {
    storage: S,
}

impl<S: Storage<u64, User>> UserRepositoryStatic<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }
    pub fn add(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn get(&self, id: u64) -> Option<&User> {
        self.storage.get(&id)
    }
    pub fn update(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn remove(&mut self, id: u64) -> Option<User> {
        self.storage.remove(&id)
    }
}

pub struct UserRepositoryDynamic<'a> {
    storage: &'a mut dyn Storage<u64, User>,
}

impl<'a> UserRepositoryDynamic<'a> {
    pub fn new(storage: &'a mut dyn Storage<u64, User>) -> Self {
        Self { storage }
    }
    pub fn add(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn get(&self, id: u64) -> Option<&User> {
        self.storage.get(&id)
    }
    pub fn update(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn remove(&mut self, id: u64) -> Option<User> {
        self.storage.remove(&id)
    }
}

pub struct InMemoryStorage<K, V> {
    data: HashMap<K, V>,
}

impl<K: std::cmp::Eq + std::hash::Hash, V> InMemoryStorage<K, V> {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }
}

impl<K: std::cmp::Eq + std::hash::Hash, V> Storage<K, V> for InMemoryStorage<K, V> {
    fn set(&mut self, key: K, val: V) {
        self.data.insert(key, val);
    }
    fn get(&self, key: &K) -> Option<&V> {
        self.data.get(key)
    }
    fn remove(&mut self, key: &K) -> Option<V> {
        self.data.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_repo() {
        let storage = InMemoryStorage::new();
        let mut repo = UserRepositoryStatic::new(storage);
        let user = User { id: 1, email: Cow::from("a@example.com"), activated: true };
        repo.add(user);
        let got = repo.get(1).unwrap();
        assert_eq!(got.email, "a@example.com");
        let removed = repo.remove(1).unwrap();
        assert_eq!(removed.id, 1);
        assert!(repo.get(1).is_none());
    }

    #[test]
    fn test_dynamic_repo() {
        let mut storage = InMemoryStorage::new();
        let mut repo = UserRepositoryDynamic::new(&mut storage);
        let user = User { id: 2, email: Cow::from("b@example.com"), activated: false };
        repo.add(user);
        let got = repo.get(2).unwrap();
        assert_eq!(got.email, "b@example.com");
        let removed = repo.remove(2).unwrap();
        assert_eq!(removed.id, 2);
        assert!(repo.get(2).is_none());
    }
}
