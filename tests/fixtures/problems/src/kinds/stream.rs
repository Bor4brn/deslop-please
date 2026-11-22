use crate::socket::Socket;

pub struct Stream {
    pub socket: Socket,
}

impl Stream {
    pub fn new(socket: Socket) -> Stream {
        Stream { socket }
    }
}
