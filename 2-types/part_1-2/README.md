// PART 1: Typestate implementation

pub struct Post<State> {
    content: String,
    _state: State,
}

// State markers
pub struct New;
pub struct Unmoderated;
pub struct Published;
pub struct Deleted;

impl Post<New> {
    pub fn new(content: String) -> Self {
        Post {
            content,
            _state: New,
        }
    }

    pub fn publish(self) -> Post<Unmoderated> {
        Post {
            content: self.content,
            _state: Unmoderated,
        }
    }
}

impl Post<Unmoderated> {
    pub fn allow(self) -> Post<Published> {
        Post {
            content: self.content,
            _state: Published,
        }
    }

    pub fn deny(self) -> Post<Deleted> {
        Post {
            content: self.content,
            _state: Deleted,
        }
    }
}

impl Post<Published> {
    pub fn delete(self) -> Post<Deleted> {
        Post {
            content: self.content,
            _state: Deleted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_post_flow() {
        let post = Post::new("Hello".into());
        let post = post.publish();
        let post = post.allow();
        let _post = post.delete();
    }

    #[test]
    fn deny_flow() {
        let post = Post::new("Test".into());
        let post = post.publish();
        let _post = post.deny();
    }
}
