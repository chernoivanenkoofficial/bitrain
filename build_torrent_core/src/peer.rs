use std::{
    io::{self, Read, Write},
    net::TcpStream, borrow::Borrow,
};

use crate::messages::{self, Handshake, Recv, Send, SendMessage, RecvMessage};
use bufstream::BufStream;

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
    pub fn connect(&mut self, handshake: impl Borrow<Handshake>) -> messages::Result<(Connection, Handshake)> {
        let mut tcp_stream = TcpStream::connect(&self.addr)?;
        handshake.borrow().send_to(&mut tcp_stream)?;

        if let Some(handshake) = Handshake::recv_from(&mut None, &mut tcp_stream)? {
            Ok(Some((Connection::new(tcp_stream), handshake)))
        } else {
            Ok(None)
        }
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
    pub fn send<S: SendMessage>(&mut self, message: S) -> io::Result<()> {
        message.send_to(&mut self.inner)?;
        self.inner.flush()
    }

    ///Attempts to recieve message from peer, discarding residual bytes, if message failed to parse
    pub fn recv<R: RecvMessage>(&mut self) -> messages::Result<R> {
        let mut hint = None;
        let message = R::recv_from(&mut hint, &mut self.inner)?;

        if message.is_none() {
            let residual_count = hint.expect("Invalid state: P2PSend implementor didn't provide hint on residual bytes of unknown message");
            self.discard_unkown_message(residual_count)?;
            Ok(None)
        } else {
            Ok(message)
        }
    }

    fn discard_unkown_message(&mut self, residual_count: usize) -> io::Result<()> {
        io::copy(
            &mut Read::by_ref(&mut self.inner).take(residual_count as u64),
            &mut io::sink(),
        )?;        

        Ok(())
    }
}
