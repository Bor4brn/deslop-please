use crate::kinds::stream::Stream;

pub struct Socket;

pub fn open() -> Stream {
    Stream::new(Socket)
}
