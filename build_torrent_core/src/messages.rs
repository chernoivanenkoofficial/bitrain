//! Type defenitions of various P2P messages.
//!  
//! For more info see <https://www.bittorrent.org/beps/bep_0003.html#peer-messages>.
use std::mem::size_of;

/// BitTorrent integer
pub type BTInt = u32;

/// Type of P2P message. See [`Message`]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Id {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Unknown = u8::MAX,
}

impl From<u8> for Id {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Choke,
            1 => Self::Unchoke,
            2 => Self::Interested,
            3 => Self::NotInterested,
            4 => Self::Have,
            5 => Self::Bitfield,
            6 => Self::Request,
            7 => Self::Piece,
            8 => Self::Cancel,
            _ => Self::Unknown,
        }
    }
}

/// Container enum represeting supported P2P messages and corresponding payload. See [`Container`],
/// [`ContainSend`] and [`ContainRecv`].
///  
/// # Note
///
/// Handshake is not included, because it's supposed to be sent first when connection
/// is established, so there is no room for variance in this case.
///
/// Keep-alive is not included as well, because message parsing discards any unknown or unsupported
/// message types which can be, in essence, considered as keep-alives themselves, so from perspective
/// of consumer there is no difference between them, thus no need to differentiate between them.
///
/// To send or recieve `keep-alive` message specifically, use [`Container::<()>`].   
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(Have),
    Bitfield(Bitfield),
    Request(Request),
    Piece(Piece),
    Cancel(Cancel),
}

impl Message {
    const MIN_LEN: usize = size_of::<BTInt>();

    pub fn id(&self) -> Id {
        match self {
            Self::Choke => Id::Choke,
            Self::Unchoke => Id::Unchoke,
            Self::Interested => Id::Interested,
            Self::NotInterested => Id::NotInterested,
            Self::Have(_) => Id::Have,
            Self::Bitfield(_) => Id::Bitfield,
            Self::Request(_) => Id::Request,
            Self::Piece(_) => Id::Piece,
            Self::Cancel(_) => Id::Cancel,
        }
    }
}

macro_rules! message_conversions {
    {$($kind:ident),+} => {
        $(
            impl From<$kind> for Message {
                fn from(val: $kind) -> Self {
                    Self::$kind(val)
                }
            }
        )*
    };
}

message_conversions! {
    Have,
    Bitfield,
    Request,
    Piece,
    Cancel
}
pub type Keepalive = ();

#[derive(Debug, Clone, PartialEq)]
pub struct Handshake {
    pub reserved: Reserved,
    pub info_hash: Vec<u8>,
    pub peer_id: Vec<u8>,
}

impl Default for Handshake {
    fn default() -> Self {
        Self {
            reserved: Reserved::default(),
            info_hash: vec![0; 20],
            peer_id: vec![0; 20],
        }
    }
}

impl Handshake {
    const BITTORRENT_PROTOCOL: &'static [u8] = "BitTorrent protocol".as_bytes();

    /// Creates new instance of `Self` and checks that `info_hash` and `peer_id`
    /// are exactly 20 bytes long.
    pub fn new(reserved: Reserved, info_hash: Vec<u8>, peer_id: Vec<u8>) -> Option<Self> {
        if info_hash.len() != 20 || peer_id.len() != 20 {
            None
        } else {
            Some(Self {
                reserved,
                info_hash,
                peer_id,
            })
        }
    }

    pub fn ext(&self) -> &Reserved {
        &self.reserved
    }

    pub fn info_hash(&self) -> &[u8] {
        &self.info_hash
    }

    pub fn peer_id(&self) -> &[u8] {
        &self.peer_id
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reserved([u8; 8]);

impl Reserved {
    pub const BYTES_COUNT: usize = 8;
    pub const EXTENSION: (usize, u8) = (5, 0x10);

    pub fn inner(&self) -> &[u8] {
        &self.0
    }

    ///See <http://www.bittorrent.org/beps/bep_0010.html>
    pub fn supports_extensions(&self) -> bool {
        self.0[Self::EXTENSION.0] & Self::EXTENSION.1 == Self::EXTENSION.1
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub struct Have {
    pub piece_index: BTInt,
}

impl Have {
    const EXPECTED_LEN: usize = size_of::<BTInt>() + size_of::<Id>();
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bitfield {
    pub bits: Vec<u8>,
}

impl Bitfield {
    const MIN_LEN: usize = 1;
}

#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub struct Request {
    pub piece_index: BTInt,
    pub offset: BTInt,
    pub data_length: BTInt,
}

impl Request {
    const EXPECTED_LEN: usize = 3 * size_of::<BTInt>() + size_of::<Id>();
}

#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub struct Cancel {
    pub piece_index: BTInt,
    pub offset: BTInt,
    pub data_length: BTInt,
}

impl Cancel {
    const EXPECTED_LEN: usize = 3 * size_of::<BTInt>() + size_of::<Id>();
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Piece {
    /// Corresponds to `index` section of P2P piece message.
    pub piece_index: BTInt,
    /// Corresponds to `begin` section of P2P piece message.
    pub offset: BTInt,
    /// Corresponds to `block` section of P2P piece message.
    pub data: Vec<u8>,
}

impl Piece {
    const MIN_LEN: usize = 2 * size_of::<BTInt>();
}

use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

/// A trait representing a data type, which can be sent in format, specified by
/// BitTorrent P2P protocol.
pub trait Send {
    /// Returns the amount of bytes `Self` will be serialized into.
    fn size(self) -> usize;
    /// Serializes self into provided writer.
    ///
    /// # Note
    ///
    /// As certain amount of nesting can be present in message structure,
    /// implementors shouldn't flush stream after serializing to avoid small writes. It's up to caller to
    /// ensure that all data is sent to underlying hardware at apropriate moment or is not
    /// lost (i.e. when [`io::BufWriter`] or similar is dropped and all non-submitted data is discarded).
    /// So caller is recomended to always pass mutable reference to writer, instead giving up ownership to `to_stream`.
    ///
    /// The other concern with nested messages is their length. For example [`Piece`] and [`Bitfield`] message formats
    /// consist of message id (message type), optionally some additional data (i.e. `index` and `begin` in `Piece`)
    /// and variable-length byte array. To allow API for such data types to stay consistent with other implementations
    /// (and give more flexibility in case future P2P protocol changes and extensions will need combining/nesting theese),
    /// implementor should serialize length of self only if it's not already encoded at the start of relevant P2P meesage.
    ///
    /// See [`ContainSend`] for more info.
    fn send_to(self, writer: impl Write) -> io::Result<()>;
}

/// A trait representing a data type, which can be recieved in format, specified by
/// BitTorrent P2P protocol.
pub trait Recv: Sized {
    /// Deserializes self from provided reader
    ///
    /// ## Note
    ///
    /// If information about variable length is already encoded in first 4 bytes of relevant P2P message,
    /// implementors should depend only on `len_hint`. To correctly give `len_hint`, wrap `Self` in [`ContainRecv`]
    ///
    /// ## Errors
    ///
    /// BitTorrent P2P allows for different extensions of its basic protocol, so there can be situations
    /// when messages of unknown format are recieved. Communication corruption and poisoning is also
    /// concern when dealing with networks.
    ///
    /// The only (if you are not willing to deal with byte mess) choice when resolving such issues are either
    /// ignore message or shutdown peer connection completely. Former requires discarding residual message bytes
    /// from source stream, so implemetor has to track ammount of risidual bytes and put it into `len_hint` in
    /// case of deserializing logic failure.
    ///
    /// In case [`io::Error`] occurs, consumer shouldn't make any asumptions about `len_hint` contents.
    ///
    /// ## Arguments
    /// ### len_hint
    ///
    /// Implepentors would be able to infer supposed amount of bytes to parse in most cases, but this is not guaranteed
    /// for data of variable length (for example [`Piece`] or [`Bitfield`]). These types of data require `len_hint`
    /// to be specified and should panic, if not provided.
    ///
    /// If message parsing fails (`recv` returns `Ok(None)`), `len_hint` must contain `Some(residual_count)`.
    /// Consumer should not make any assumptions about contents of reader in this case besides fact, that `residual_count`
    /// bytes needs to be discarded from `reader` before next meaningfull block of data can be accessed.
    ///
    /// If message parsing fails, but no hint on residual bytes was provided, caller decides how to handle error.
    /// (see [Connection::recv](`crate::peer::Connection::recv()`) for example).  
    fn recv_from(len_hint: &mut Option<usize>, reader: impl Read) -> Result<Self>;
}

pub type Result<T> = io::Result<Option<T>>;

/// Wraps data, which can be sent accroding to P2P protocol, as standlaone message. See [`SendMessage`].
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct SendContainer<M>(pub M);

/// Wraps data, which can be recieved accroding to P2P protocol. See [`RecvMessage`].
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct RecvContainer<M>(pub M);

impl<M> RecvContainer<M> {
    pub fn into_inner(self) -> M {
        self.0
    }

    pub fn inner(&self) -> &M {
        &self.0
    }

    pub fn inner_mut(&mut self) -> &mut M {
        &mut self.0
    }
}

/// Marker trait, that represents standalone P2P message, which can be sent to peer.
///
/// As any P2P message starts with length, implementor should always encode length of serialized `Self`
/// in the first four bytes with u32 (NetworkEndian).
pub trait SendMessage: Send {}
/// Marker trait, that tepresents standalone P2P message, which can be recieved by peer.
///
/// As any P2P message starts with length, implementor should always encode length of serialized `Self`
/// in the first four bytes with u32 (NetworkEndian).
pub trait RecvMessage: Recv {}

macro_rules! check_length_exact {
    ($hint:ident, $len:expr) => {{
        let len = $len;

        if $hint.unwrap_or(len) != len {
            return Ok(None);
        }
    }};
}

macro_rules! impl_message_without_payload {
    {$($kind:ident),*} => {$(
        #[derive(Debug, Clone, Copy, Default, PartialEq)]
        pub struct $kind;

        impl Recv for $kind {
            fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
                check_length_exact!(len_hint, size_of::<u8>());

                let id = reader.read_u8()?;

                if id == Id::$kind as u8 {
                    *len_hint = Some(0);
                    Ok(Some(Self))
                } else {
                    Ok(None)
                }
            }
        }

        impl Send for &$kind {
            fn size(self) -> usize {
                size_of::<Id>()
            }

            fn send_to(self, mut writer: impl Write) -> io::Result<()> {
                writer.write_u8(Id::$kind as u8)
            }
        }
    )*};
}

macro_rules! impl_send_owned {
    {$($kind:ty),*} => {$(
        impl Send for $kind {
            fn size(self) -> usize {
                (&self).size()
            }

            fn send_to(self, writer: impl Write) -> io::Result<()> {
                (&self).send_to(writer)
            }
        }
    )*};
}

impl_message_without_payload! {
    Choke,
    Unchoke,
    Interested,
    NotInterested
}

impl_send_owned! {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Handshake,
    Have,
    Bitfield,
    Request,
    Piece,
    Cancel,
    Message,
    ()
}

impl<S: Send + Copy> Send for SendContainer<S> {
    fn size(self) -> usize {
        self.0.size() + size_of::<BTInt>()
    }

    fn send_to(self, mut writer: impl Write) -> io::Result<()> {
        writer.write_u8(self.0.size().try_into().expect("Invalid integer value."))?;
        self.0.send_to(writer)
    }
}

impl<S: Send + Copy> SendMessage for SendContainer<S> {}

impl<R: Recv> Recv for RecvContainer<R> {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        let len = reader.read_u32::<NetworkEndian>()? as usize;
        *len_hint = Some(len);

        if let Some(inner) = R::recv_from(len_hint, reader)? {
            Ok(Some(Self(inner)))
        } else {
            Ok(None)
        }
    }
}

impl<R: Recv> RecvMessage for RecvContainer<R> {}

impl Recv for Have {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        check_length_exact!(len_hint, Self::EXPECTED_LEN);

        if reader.read_u8()? != Id::Have as u8 {
            *len_hint = Some(Self::EXPECTED_LEN - 1);
            return Ok(None);
        }

        let piece_index = reader.read_u32::<NetworkEndian>()?;

        *len_hint = Some(0);
        Ok(Some(Self { piece_index }))
    }
}

impl Send for &Have {
    fn send_to(self, mut writer: impl Write) -> io::Result<()> {
        writer.write_u8(Id::Have as u8)?;
        writer.write_u32::<NetworkEndian>(self.piece_index)
    }

    fn size(self) -> usize {
        Have::EXPECTED_LEN
    }
}

impl Recv for Bitfield {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        let len = len_hint.expect("Invalid state: length hint expected.");

        if len < Self::MIN_LEN {
            return Ok(None);
        }

        if reader.read_u8()? != Id::Bitfield as u8 {
            *len_hint = Some(len);
            return Ok(None);
        }

        let mut bits = vec![0u8; len];
        reader.read_exact(&mut bits)?;

        *len_hint = Some(0);
        Ok(Some(Self { bits }))
    }
}

impl Send for &Bitfield {
    fn send_to(self, mut writer: impl Write) -> io::Result<()> {
        writer.write_u8(Id::Bitfield as u8)?;
        writer.write_all(&self.bits)
    }

    fn size(self) -> usize {
        self.bits.len() + size_of::<Id>()
    }
}

impl Recv for Request {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        check_length_exact!(len_hint, Self::EXPECTED_LEN);

        if reader.read_u8()? != Id::Request as u8 {
            *len_hint = Some(Self::EXPECTED_LEN - 1);
            return Ok(None);
        }

        let piece_index = reader.read_u32::<NetworkEndian>()?;
        let offset = reader.read_u32::<NetworkEndian>()?;
        let data_length = reader.read_u32::<NetworkEndian>()?;

        *len_hint = Some(0);
        Ok(Some(Self {
            piece_index,
            offset,
            data_length,
        }))
    }
}

impl Send for &Request {
    fn send_to(self, mut writer: impl Write) -> io::Result<()> {
        writer.write_u8(Id::Request as u8)?;
        writer.write_u32::<NetworkEndian>(self.piece_index)?;
        writer.write_u32::<NetworkEndian>(self.offset)?;
        writer.write_u32::<NetworkEndian>(self.data_length)
    }

    fn size(self) -> usize {
        Request::EXPECTED_LEN
    }
}

impl Recv for Cancel {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        check_length_exact!(len_hint, Self::EXPECTED_LEN);

        if reader.read_u8()? != Id::Cancel as u8 {
            *len_hint = Some(Self::EXPECTED_LEN - 1);
            return Ok(None);
        }

        let piece_index = reader.read_u32::<NetworkEndian>()?;
        let offset = reader.read_u32::<NetworkEndian>()?;
        let data_length = reader.read_u32::<NetworkEndian>()?;

        *len_hint = Some(0);
        Ok(Some(Self {
            piece_index,
            offset,
            data_length,
        }))
    }
}

impl Send for &Cancel {
    fn send_to(self, mut writer: impl Write) -> io::Result<()> {
        writer.write_u8(Id::Cancel as u8)?;
        writer.write_u32::<NetworkEndian>(self.piece_index)?;
        writer.write_u32::<NetworkEndian>(self.offset)?;
        writer.write_u32::<NetworkEndian>(self.data_length)
    }

    fn size(self) -> usize {
        Cancel::EXPECTED_LEN
    }
}

impl Recv for Piece {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        let len = len_hint.expect("Invalid state: length hint expected.");

        if len < Self::MIN_LEN {
            return Ok(None);
        }

        if reader.read_u8()? != Id::Piece as u8 {
            *len_hint = Some(len - 1);
            return Ok(None);
        }

        let piece_index = reader.read_u32::<NetworkEndian>()?;
        let offset = reader.read_u32::<NetworkEndian>()?;

        let data_len = len - Self::MIN_LEN;
        let mut data = vec![0; data_len];

        reader.read_exact(&mut data)?;

        *len_hint = Some(0);
        Ok(Some(Self {
            data,
            piece_index,
            offset,
        }))
    }
}

impl Send for &Piece {
    fn send_to(self, mut writer: impl Write) -> io::Result<()> {
        writer.write_u8(Id::Piece as u8)?;
        writer.write_u32::<NetworkEndian>(self.piece_index)?;
        writer.write_u32::<NetworkEndian>(self.offset)?;
        writer.write_all(&self.data)
    }

    fn size(self) -> usize {
        Piece::MIN_LEN + self.data.len()
    }
}

impl Recv for Message {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        use std::slice::from_ref;
        let len = reader.read_u32::<NetworkEndian>()? as usize;

        if len == 0 {
            *len_hint = Some(0);
            return Ok(None);
        }

        let id = reader.read_u8()?;
        *len_hint = Some(len);
        let reader = from_ref(&id).chain(reader);

        let message = match Id::from(id) {
            Id::Choke => Some(Self::Choke),
            Id::Unchoke => Some(Self::Unchoke),
            Id::Interested => Some(Self::Interested),
            Id::NotInterested => Some(Self::NotInterested),
            Id::Have => {
                let have = Have::recv_from(len_hint, reader)?;
                have.map(Into::into)
            }
            Id::Bitfield => {
                let bitfield = Bitfield::recv_from(len_hint, reader)?;
                bitfield.map(Into::into)
            }
            Id::Request => {
                let request = Request::recv_from(len_hint, reader)?;
                request.map(Into::into)
            }
            Id::Piece => {
                let piece = Piece::recv_from(len_hint, reader)?;
                piece.map(Into::into)
            }
            Id::Cancel => {
                let cancel = Cancel::recv_from(len_hint, reader)?;
                cancel.map(Into::into)
            }
            Id::Unknown => None,
        };

        Ok(message)
    }
}

impl RecvMessage for Message {}

impl Send for &Message {
    fn send_to(self, writer: impl Write) -> io::Result<()> {
        match self {
            Message::Choke => Send::send_to(SendContainer(&Choke), writer),
            Message::Unchoke => Send::send_to(SendContainer(&Unchoke), writer),
            Message::Interested => Send::send_to(SendContainer(&Interested), writer),
            Message::NotInterested => Send::send_to(SendContainer(&NotInterested), writer),
            Message::Have(have) => Send::send_to(SendContainer(have), writer),
            Message::Bitfield(bitfield) => Send::send_to(SendContainer(bitfield), writer),
            Message::Request(req) => Send::send_to(SendContainer(req), writer),
            Message::Piece(piece) => Send::send_to(SendContainer(piece), writer),
            Message::Cancel(cancel) => Send::send_to(SendContainer(cancel), writer),
        }
    }

    fn size(self) -> usize {
        Message::MIN_LEN
            + match self {
                Message::Choke => Send::size(SendContainer(&Choke)),
                Message::Unchoke => Send::size(SendContainer(&Unchoke)),
                Message::Interested => Send::size(SendContainer(&Interested)),
                Message::NotInterested => Send::size(SendContainer(&NotInterested)),
                Message::Have(have) => Send::size(SendContainer(have)),
                Message::Bitfield(bitfield) => Send::size(SendContainer(bitfield)),
                Message::Request(req) => Send::size(SendContainer(req)),
                Message::Piece(piece) => Send::size(SendContainer(piece)),
                Message::Cancel(cancel) => Send::size(SendContainer(cancel)),
            }
    }
}

impl SendMessage for &Message {}
impl SendMessage for Message {}

impl Recv for Handshake {
    fn recv_from(len_hint: &mut Option<usize>, mut reader: impl Read) -> Result<Self> {
        let protocol_name_len = reader.read_u8()? as usize;

        let mut protocol = vec![0; protocol_name_len];
        reader.read_exact(&mut protocol)?;

        if protocol != Self::BITTORRENT_PROTOCOL {
            // Unknown protocol implies that handshake payload len is unknown
            *len_hint = None;
            return Ok(None);
        }

        let mut reserved = [0; 8];
        reader.read_exact(&mut reserved)?;

        let mut info_hash = vec![0u8; 20];
        reader.read_exact(&mut info_hash)?;

        let mut peer_id = vec![0u8; 20];
        reader.read_exact(&mut peer_id)?;

        *len_hint = Some(0);
        Ok(Self::new(Reserved(reserved), info_hash, peer_id))
    }
}

impl RecvMessage for Handshake {}

impl Send for &Handshake {
    fn size(self) -> usize {
        68
    }

    fn send_to(self, mut writer: impl Write) -> io::Result<()> {
        writer.write_u8(Handshake::BITTORRENT_PROTOCOL.len() as u8)?;
        writer.write_all(Handshake::BITTORRENT_PROTOCOL)?;
        writer.write_all(self.reserved.inner())?;
        writer.write_all(&self.info_hash)?;
        writer.write_all(&self.peer_id)
    }
}

impl SendMessage for Handshake {}
impl SendMessage for &'_ Handshake {}

impl Send for &() {
    fn size(self) -> usize {
        0
    }

    fn send_to(self, _: impl Write) -> io::Result<()> {
        Ok(())
    }
}

impl Recv for () {
    fn recv_from(len_hint: &mut Option<usize>, _: impl Read) -> Result<Self> {
        check_length_exact!(len_hint, 0);

        Ok(Some(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::fmt::Debug;

    #[rstest]
    #[case::choke(Choke)]
    #[case::unchoke(Unchoke)]
    #[case::interested(Interested)]
    #[case::not_interested(NotInterested)]
    #[case::have(Have::default())]
    #[case::bitfield(Bitfield::default())]
    #[case::request(Request::default())]
    #[case::piece(Piece::default())]
    #[case::cancel(Cancel::default())]
    #[case::handshake(Handshake::default())]
    fn vise_versa<S: Send + Recv + Clone + PartialEq + Debug>(#[case] data: S) {
        let mut bytes = vec![];
        data.clone().send_to(&mut bytes).unwrap();

        let mut len_hint = Some(bytes.len());
        let recieved = S::recv_from(&mut len_hint, &bytes[..]).unwrap();

        assert_eq!(Some(data), recieved);
    }
}
