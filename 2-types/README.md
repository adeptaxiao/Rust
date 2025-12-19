use std::marker::PhantomData;

// Marker types for states
pub struct New;
pub struct Unmoderated;
pub struct Published;
pub struct Deleted;

pub struct Post<State> {
    text: String,
    _state: PhantomData<State>,
}

impl Post<New> {
    pub fn create(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            _state: PhantomData,
        }
    }

    pub fn publish(self) -> Post<Unmoderated> {
        Post {
            text: self.text,
            _state: PhantomData,
        }
    }
}

impl Post<Unmoderated> {
    pub fn allow(self) -> Post<Published> {
        Post {
            text: self.text,
            _state: PhantomData,
        }
    }

    pub fn deny(self) -> Post<Deleted> {
        Post {
            text: self.text,
            _state: PhantomData,
        }
    }
}

impl Post<Published> {
    pub fn delete(self) -> Post<Deleted> {
        Post {
            text: self.text,
            _state: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path() {
        let post = Post::create("Hello world");
        let post = post.publish();
        let post = post.allow();
        let _post = post.delete();
    }

    #[test]
    fn denied_post() {
        let post = Post::create("Spam");
        let post = post.publish();
        let _post = post.deny();
    }
}
