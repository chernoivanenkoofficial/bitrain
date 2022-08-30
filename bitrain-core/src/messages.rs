//! Type defenitions of various P2P messages.
//!  
//! For more info see <https://www.bittorrent.org/beps/bep_0003.html#peer-messages>.
use std::{mem::size_of, ops::Deref};

/// BitTorrent integer
pub type BTInt = u32;

pub trait Standalone {
    const ID: u8;
}

/// Container enum represeting supported P2P messages and corresponding payload. See [`Container`].
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
#[derive(Debug, Clone, PartialEq, Recv, Send)]
#[message(mod_path = "crate::messages")]
pub enum Message {
    #[standalone(id = 0)]
    Choke,
    #[standalone(id = 1)]
    Unchoke,
    #[standalone(id = 2)]
    Interested,
    #[standalone(id = 3)]
    NotInterested,
    Have(Have),
    Bitfield(Bitfield),
    Request(Request),
    Piece(Piece),
    Cancel(Cancel),
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Handshake {
    pub reserved: Reserved,
    pub info_hash: Box<[u8; 20]>,
    pub peer_id: Box<[u8; 20]>,
}

impl Handshake {
    const BITTORRENT_PROTOCOL: &'static [u8] = "BitTorrent protocol".as_bytes();

    pub fn ext(&self) -> &Reserved {
        &self.reserved
    }

    pub fn info_hash(&self) -> &[u8; 20] {
        &self.info_hash
    }

    pub fn peer_id(&self) -> &[u8; 20] {
        &self.peer_id
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Default, PartialEq, Encode, Decode)]
#[message(mod_path = "crate::messages")]
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

crate::flag_message! {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Encode, Decode, Standalone)]
#[message(mod_path = "crate::messages")]
#[standalone(id = 4)]
pub struct Have {
    pub piece_index: BTInt,
}

#[derive(Debug, Clone, Default, PartialEq, Encode, Decode, Standalone)]
#[message(mod_path = "crate::messages")]
#[standalone(id = 5)]
pub struct Bitfield {
    pub bits: Vec<u8>,
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Encode, Decode, Standalone)]
#[message(mod_path = "crate::messages")]
#[standalone(id = 6)]
pub struct Request {
    pub piece_index: BTInt,
    pub offset: BTInt,
    pub data_length: BTInt,
}

#[derive(Debug, Clone, Default, PartialEq, Encode, Decode, Standalone)]
#[message(mod_path = "crate::messages")]
#[standalone(id = 7)]
pub struct Piece {
    /// Corresponds to `index` section of P2P piece message.
    pub piece_index: BTInt,
    /// Corresponds to `begin` section of P2P piece message.
    pub offset: BTInt,
    /// Corresponds to `block` section of P2P piece message.
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Encode, Decode, Standalone)]
#[message(mod_path = "crate::messages")]
#[standalone(id = 8)]
pub struct Cancel {
    pub piece_index: BTInt,
    pub offset: BTInt,
    pub data_length: BTInt,
}
use bitrain_derive::{Decode, Encode, Standalone, Recv, Send};
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

/// A trait representing a data type, which can be sent in format, specified by
/// BitTorrent P2P protocol.
pub trait Encode {
    /// Returns the amount of bytes `Self` will be encoded into.
    fn size(&self) -> usize;
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
    fn encode_to(&self, writer: &mut impl Write) -> io::Result<()>;

    fn encode(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(self.size());
        self.encode_to(&mut vec).unwrap();

        vec
    }
}

/// A trait representing a data type, which can be recieved in format, specified by
/// BitTorrent P2P protocol.
pub trait Decode: Sized {
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
    /// ## Arguments
    /// ### len_hint
    ///
    /// Amount of bytes available for parsing.
    ///
    /// On successfull return or parsing failure (`recv_from` returns `Ok(None)`) implementors should update this
    /// argument with `Some(len_hint - bytes_consumed)`.
    ///
    /// If message parsing fails, consumer should not make any assumptions about contents of reader besides fact, that
    /// `len_hint` bytes need to be discarded from `reader` before next meaningfull block of data can be accessed.
    ///
    /// If message parsing fails, but no hint on residual bytes was provided, caller decides how to handle error.
    /// (see [Connection::recv](`crate::peer::Connection::recv()`) for example).  
    ///
    /// In case [`io::Error`] occurs, consumer shouldn't make any asumptions about `len_hint` contents.
    fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self>;

    fn decode(mut bytes: &[u8]) -> Result<Self> {
        let mut len = bytes.len();
        Self::decode_from(&mut len, bytes.by_ref())
    }

    fn decode_or_discard_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
        let result = Self::decode_from(len_hint, reader)?;

        if result.is_none() {
            utils::discard_bytes(reader.by_ref(), *len_hint)?;
        }

        Ok(result)
    }
}

pub type Result<T> = io::Result<Option<T>>;

/// Marker trait, that represents standalone P2P message, which can be sent to peer.
///
/// As any P2P message starts with length (besides [`Handshake`], which is already implemented),
/// implementor should always encode length of serialized `Self` in the first four bytes (u32 NetworkEndian).
pub trait Send {
    fn send_to(&self, writer: &mut impl Write) -> io::Result<()>;
}
/// Marker trait, that tepresents standalone P2P message, which can be recieved by peer.
///
/// As any P2P message starts with length, (besides [`Handshake`], which is already implemented),
/// implementor should always decode length of message in stream from the first four bytes (u32 NetworkEndian).
pub trait Recv: Sized {
    fn recv_from(reader: &mut impl Read) -> Result<Self>;
}

#[macro_export]
macro_rules! flag_message {
    {$($kind:ident = $id:expr),*} => {$(
        #[derive(Debug, Clone, Copy, Default, PartialEq, Encode, Decode, Standalone)]
        #[message(mod_path = "crate::messages")]
        #[standalone(id = $id)]
        pub struct $kind;
    )*};
}

/// Wraps data, which can be exchanged accroding to P2P protocol as [standalone](`Standalone`) message. See [`Recv`] and [`Send`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct Container<M>(pub M);

impl<M> Container<M> {
    pub const MAX_DATA_SIZE: usize = u32::MAX as usize - size_of::<BTInt>() - size_of::<u8>();

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

impl<R: Decode + Standalone> Recv for Container<R> {
    fn recv_from(reader: &mut impl Read) -> Result<Self> {
        let mut len = reader.read_u32::<NetworkEndian>()? as usize;
        if len == 0 {
            return Ok(None);
        }

        if reader.read_u8()? != <R as Standalone>::ID {
            return Ok(None);
        } else {
            len -= 1;

            <R as Decode>::decode_or_discard_from(&mut len, reader).map(|opt| opt.map(Self))
        }
    }
}

impl<S: Encode + Standalone> Send for Container<&'_ S> {
    fn send_to(&self, writer: &mut impl Write) -> io::Result<()> {
        let data_len: BTInt = self
            .0
            .size()
            .try_into()
            .expect("Container: data is too big to send.");        

        (data_len + 1).encode_to(writer)?;
        <S as Standalone>::ID.encode_to(writer)?;
        self.0.encode_to(writer)
    }
}

impl Recv for Handshake {
    fn recv_from(reader: &mut impl Read) -> Result<Self> {
        let mut protocol_name_len =
            utils::unwrap_or_return!(u8::decode_or_discard_from(&mut 1, reader.by_ref())?) as usize;
        let protocol = utils::unwrap_or_return!(Vec::decode_or_discard_from(
            &mut protocol_name_len,
            reader
        )?);

        if protocol != Self::BITTORRENT_PROTOCOL {
            // Unknown protocol implies that handshake payload len is unknown
            return Ok(None);
        }

        let mut len_hint = 48;

        let reserved = utils::unwrap_or_return!(<[u8; 8]>::decode_or_discard_from(
            &mut len_hint,
            reader
        )?);
        let info_hash =
            utils::unwrap_or_return!(Box::decode_or_discard_from(&mut len_hint, reader.by_ref())?);
        let peer_id =
            utils::unwrap_or_return!(Box::decode_or_discard_from(&mut len_hint, reader.by_ref())?);

        Ok(Some(Self {
            reserved: Reserved(reserved),
            info_hash,
            peer_id,
        }))
    }
}

impl Send for Handshake {
    fn send_to(&self, writer: &mut impl Write) -> io::Result<()> {
        (Self::BITTORRENT_PROTOCOL.len() as u8).encode_to(writer)?;
        Self::BITTORRENT_PROTOCOL.encode_to(writer)?;
        self.reserved.inner().encode_to(writer)?;
        self.info_hash.encode_to(writer)?;
        self.peer_id.encode_to(writer)
    }
}

impl Encode for () {
    fn size(&self) -> usize {
        0
    }

    fn encode_to(&self, _: &mut impl Write) -> io::Result<()> {
        Ok(())
    }
}

impl Decode for () {
    fn decode_from(_: &mut usize, _: &mut impl Read) -> Result<Self> {
        Ok(Some(()))
    }
}

macro_rules! impl_sr_for_primitive {
    ($([$prim:ty, $write:ident, $read:ident]),*) => {$(
        impl Encode for $prim {
            fn size(&self) -> usize {
                size_of::<Self>()
            }

            fn encode_to(&self, writer: &mut impl Write) -> io::Result<()> {
                WriteBytesExt::$write::<NetworkEndian>(writer, *self)
            }
        }

        impl Decode for $prim {
            fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
                if *len_hint < size_of::<Self>() {
                    Ok(None)
                } else {
                    *len_hint -= size_of::<Self>();
                    ReadBytesExt::$read::<NetworkEndian>(reader).map(Option::Some)
                }
            }
        }
    )*};
}

impl Encode for u8 {
    fn size(&self) -> usize {
        size_of::<Self>()
    }

    fn encode_to(&self, writer: &mut impl Write) -> io::Result<()> {
        WriteBytesExt::write_u8(writer, *self)
    }
}

impl Decode for u8 {
    fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
        if *len_hint < size_of::<Self>() {
            Ok(None)
        } else {
            ReadBytesExt::read_u8(reader).map(Option::Some)
        }
    }
}

impl_sr_for_primitive!(
    [u16, write_u16, read_u16],
    [u32, write_u32, read_u32],
    [u64, write_u64, read_u64],
    [u128, write_u128, read_u128]
);

impl Encode for [u8] {
    fn size(&self) -> usize {
        self.len()
    }

    fn encode_to(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(self)
    }
}

impl<const D: usize> Encode for [u8; D] {
    fn size(&self) -> usize {
        self.as_ref().size()
    }

    fn encode_to(&self, writer: &mut impl Write) -> io::Result<()> {
        self.as_ref().encode_to(writer)
    }
}

impl Decode for Vec<u8> {
    fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
        let mut buf = vec![0; *len_hint];
        reader.read_exact(&mut buf[..])?;
        *len_hint = 0;

        Ok(Some(buf))
    }
}

impl Decode for Box<[u8]> {
    fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
        Vec::<u8>::decode_from(len_hint, reader).map(|opt| opt.map(Into::into))
    }
}

impl<const D: usize> Decode for [u8; D] {
    fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
        if *len_hint < D {
            Ok(None)
        } else {
            let mut buf = [0; D];
            reader.read_exact(&mut buf)?;

            *len_hint -= D;
            Ok(Some(buf))
        }
    }
}

impl<const D: usize> Decode for Box<[u8; D]> {
    fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
        #![allow(const_item_mutation)]

        if *len_hint < D {
            Ok(None)
        } else {
            //Boxed arrays never return Ok(None) so unwrap never falls
            unsafe {
                let boxed_slice = Box::<[u8]>::decode_from(&mut D, reader)?.unwrap_unchecked();
                //Slice len checked to be equal to D
                let boxed_array = boxed_slice.try_into().unwrap_unchecked();

                *len_hint -= D;
                Ok(Some(boxed_array))
            }
        }
    }
}

impl Encode for &str {
    fn size(&self) -> usize {
        self.len()
    }

    fn encode_to(&self, writer: &mut impl Write) -> io::Result<()> {
        self.as_bytes().encode_to(writer)
    }
}

impl Decode for String {
    fn decode_from(len_hint: &mut usize, reader: &mut impl Read) -> Result<Self> {
        //Byte representaions never return Ok(None) so unwrap never falls
        unsafe {
            let bytes = Vec::decode_from(len_hint, reader)?.unwrap_unchecked();
            let string = String::from_utf8(bytes).ok();

            Ok(string)
        }
    }
}

pub mod utils {
    use std::io;

    pub fn discard_bytes(reader: impl io::Read, count: usize) -> io::Result<()> {
        io::copy(&mut reader.take(count as u64), &mut io::sink())?;

        Ok(())
    }

    #[macro_export]
    macro_rules! unwrap_or_return {
        ($opt:expr) => {
            if let Some(val) = $opt {
                val
            } else {
                return Ok(None);
            }
        };
    }

    pub use unwrap_or_return;
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
    fn encode_decode<S: Encode + Decode + PartialEq + Debug>(#[case] data: S) {
        let bytes = data.encode();
        let recieved = S::decode(&bytes).expect("Decoding rrror");

        assert_eq!(Some(data), recieved);
    }

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
    fn container<S: Encode + Standalone + Decode + PartialEq + Debug>(#[case] data: S) {
        let mut buf = vec![];

        Container(&data).send_to(&mut buf).unwrap();
        let recieved = Container::recv_from((&buf[..]).by_ref())
            .unwrap()
            .map(Container::into_inner);

        assert_eq!(Some(data), recieved);
    }

    #[rstest]
    #[case::msg_choke(Message::Choke)]
    #[case::msg_unchoke(Message::Unchoke)]
    #[case::msg_interested(Message::Interested)]
    #[case::msg_not_interested(Message::NotInterested)]
    #[case::msg_have(Message::Have(Default::default()))]
    #[case::msg_bitfield(Message::Bitfield(Default::default()))]
    #[case::msg_request(Message::Request(Default::default()))]
    #[case::msg_piece(Message::Piece(Default::default()))]
    #[case::msg_cancel(Message::Cancel(Default::default()))]
    fn send_recv<M: Send + Recv + PartialEq + Debug>(#[case] message: M) {
        let mut buf = vec![];

        message.send_to(&mut buf).unwrap();
        let recieved = <M as Recv>::recv_from((&buf[..]).by_ref())
            .unwrap();

        assert_eq!(Some(message), recieved);
    }
}
