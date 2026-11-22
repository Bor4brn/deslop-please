use crate::kinds::epoll::Ready;

pub fn ready() -> Ready {
    Ready::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn makes_one() {
        let _ = crate::kinds::epoll::Ready::new();
    }
}
