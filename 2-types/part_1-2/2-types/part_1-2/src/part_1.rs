use std::marker::PhantomData;

// Marker structs для станів
pub struct New;
pub struct Unmoderated;
pub struct Published;
pub struct Deleted;

// Post тип з типом стану
pub struct Post<State> {
    content: String,
    _state: PhantomData<State>,
}

impl Post<New> {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            _state: PhantomData,
        }
    }

    pub fn publish(self) -> Post<Unmoderated> {
        Post {
            content: self.content,
            _state: PhantomData,
        }
    }
}

impl Post<Unmoderated> {
    pub fn allow(self) -> Post<Published> {
        Post {
            content: self.content,
            _state: PhantomData,
        }
    }

    pub fn deny(self) -> Post<Deleted> {
        Post {
            content: self.content,
            _state: PhantomData,
        }
    }
}

impl Post<Published> {
    pub fn delete(self) -> Post<Deleted> {
        Post {
            content: self.content,
            _state: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_flow() {
        let post = Post::<New>::new("Hello Typestate!");
        let post = post.publish();
        let post = post.allow();
        let _post = post.delete();
    }

    #[test]
    fn deny_flow() {
        let post = Post::<New>::new("Spam post");
        let post = post.publish();
        let _post = post.deny();
    }
}
