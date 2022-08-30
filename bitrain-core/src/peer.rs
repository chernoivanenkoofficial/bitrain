use std::{
    io::{self, Write},
    net::TcpStream, borrow::Borrow,
};

use crate::messages::{self, Handshake, Send, Recv};
use bufstream::BufStream;

#[allow(dead_code)]
pub struct Peer {
    chocked: bool,
    interested: bool,
    uploaded: usize,
    downloaded: usize,
    addr: (String, u16),
}

impl Peer {
    /// Inializes unconnected peer.
    ///
    /// ### Note
    ///
    /// `new()` does not attempt to connect on instantiating, because client implementors
    /// can use different strategies for load-balancing, torrent peer-list can extend while processing, etc.
    ///
    /// To actually connect to peer and start message exchange consumer should use [connect()](`Peer::connect`).
    pub fn new(addr: (String, u16)) -> Self {
        Self {
            chocked: false,
            interested: false,
            uploaded: 0,
            downloaded: 0,
            addr,
        }
    }

    /// Attempts to connect to peer and exchange handshakes with it.
    pub fn handshake(&mut self, handshake: impl Borrow<Handshake>) -> messages::Result<(Connection, Handshake)> {
        let mut connection = self.connect()?;

        connection.send(handshake.borrow())?;        
        let recieved = connection.recv::<Handshake>()?;
        
        Ok(recieved.map(|h| (connection, h)))
    }

    pub fn connect(&mut self) -> io::Result<Connection> {
        Ok(Connection::new(TcpStream::connect(&self.addr)?))
    }
}

pub struct Connection {
    inner: BufStream<TcpStream>,
}

impl Connection {
    fn new(tcp: TcpStream) -> Self {
        Self {
            inner: BufStream::new(tcp),
        }
    }

    /// Attempts to send specified message to peer. See [`P2PSend`]
    pub fn send<S: Send>(&mut self, message: &S) -> io::Result<()> {
        message.send_to(&mut self.inner)?;
        self.inner.flush()
    }

    ///Attempts to recieve message from peer, discarding residual bytes, if message failed to parse (see [`Recv`]).
    pub fn recv<R: Recv>(&mut self) -> messages::Result<R> {
        R::recv_from(&mut self.inner)
    }
}
