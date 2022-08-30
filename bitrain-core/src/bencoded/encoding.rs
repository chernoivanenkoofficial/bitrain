use std::collections::HashMap;
use std::io::Write;
use std::slice::from_ref;

use super::{BInt, BStr, BString};

mod delimiters {
    pub const INT_PREFIX: u8 = b'i';
    pub const LIST_PREFIX: u8 = b'l';
    pub const DICTIONARY_PREFIX: u8 = b'd';

    pub const STRING_INFIX: u8 = b':';

    pub const END_SUFFIX: u8 = b'e';
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait BDecode: Sized {
    fn decode(bytes: &mut impl Iterator<Item = u8>) -> Result<Self>;
}

pub trait BEncode: Sized {
    fn encode(self) -> Box<[u8]> {
        let mut bytes = Vec::new();
        //Fails only on allocation error, which itself results is panic, so unwrap is virtually infallible
        self.encode_into_stream(&mut bytes).unwrap();

        bytes.into_boxed_slice()
    }

    fn encode_into_stream(self, stream: &mut impl Write) -> std::io::Result<()> {
        stream.write_all(&self.encode())
    }
}

pub type BList = Vec<Entry>;
pub type BSlice = [Entry];
pub type BDictionary = HashMap<BString, Entry>;

#[derive(Debug, Clone)]
pub enum Entry {
    Integer(BInt),
    String(BString),
    List(BList),
    Dictionary(BDictionary),
}

impl Entry {
    pub fn parse_or_err<T, E>(self, err: E) -> std::result::Result<T, E>
    where
        T: TryFrom<Self>,
    {
        self.try_into().map_err(|_| err)
    }

    pub fn parse<T>(self) -> Option<T>
    where
        Self: TryInto<T>,
    {
        self.try_into().ok()
    }
}

impl TryFrom<Entry> for BDictionary {
    type Error = Entry;

    fn try_from(value: Entry) -> std::result::Result<Self, Self::Error> {
        if let Entry::Dictionary(val) = value {
            Ok(val)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Entry> for BList {
    type Error = Entry;

    fn try_from(value: Entry) -> std::result::Result<Self, Self::Error> {
        if let Entry::List(val) = value {
            Ok(val)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Entry> for BString {
    type Error = Entry;

    fn try_from(value: Entry) -> std::result::Result<Self, Self::Error> {
        if let Entry::String(val) = value {
            Ok(val)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Entry> for BInt {
    type Error = Entry;

    fn try_from(value: Entry) -> std::result::Result<Self, Self::Error> {
        if let Entry::Integer(val) = value {
            Ok(val)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Entry> for String {
    type Error = Entry;

    fn try_from(value: Entry) -> std::result::Result<Self, Self::Error> {
        let bstring = BString::try_from(value)?;

        if std::str::from_utf8(&bstring).is_ok() {
            Ok(unsafe { String::from_utf8_unchecked(Vec::from(bstring)) })
        } else {
            Err(Entry::String(bstring))
        }
    }
}

impl BDecode for Entry {
    fn decode(bytes: &mut impl Iterator<Item = u8>) -> Result<Self> {
        let mut peekable = bytes.peekable();

        match peekable.peek() {
            Some(&delimiters::INT_PREFIX) => Ok(Self::Integer(BInt::decode(&mut peekable)?)),
            Some(&delimiters::LIST_PREFIX) => Ok(Self::List(Vec::<Entry>::decode(&mut peekable)?)),
            Some(&delimiters::DICTIONARY_PREFIX) => Ok(Self::Dictionary(
                HashMap::<BString, Entry>::decode(&mut peekable)?,
            )),
            Some(_) => Ok(Self::String(BString::decode(&mut peekable)?)),
            None => Err(Error::InvalidFormat),
        }
    }
}

impl BEncode for &Entry {
    fn encode_into_stream(self, stream: &mut impl Write) -> std::io::Result<()> {
        match self {
            Entry::Integer(i) => i.encode_into_stream(stream),
            Entry::String(s) => s.encode_into_stream(stream),
            Entry::List(l) => l.encode_into_stream(stream),
            Entry::Dictionary(d) => d.encode_into_stream(stream),
        }
    }
}

impl BDecode for BInt {
    fn decode(bytes: &mut impl Iterator<Item = u8>) -> Result<Self> {
        if bytes.next() != Some(delimiters::INT_PREFIX) {
            return Err(Error::InvalidFormat);
        };

        let repr = utils::collect_up_to(bytes, delimiters::END_SUFFIX);

        //MBDO: Check for leading zeroes

        utils::parse_utf8_bytes(&repr)
    }
}

impl BEncode for BInt {
    fn encode_into_stream(self, stream: &mut impl Write) -> std::io::Result<()> {
        stream.write_all(from_ref(&delimiters::INT_PREFIX))?;
        stream.write_all(format!("{}", self).as_bytes())?;
        stream.write_all(from_ref(&delimiters::END_SUFFIX))?;

        Ok(())
    }
}

impl BDecode for BString {
    fn decode(bytes: &mut impl Iterator<Item = u8>) -> Result<Self> {
        let len_buf = utils::collect_up_to(bytes, delimiters::STRING_INFIX);
        let len = utils::parse_utf8_bytes::<usize>(&len_buf)?;

        let repr = bytes.take(len).collect::<Vec<_>>();

        if repr.len() == len {
            Ok(repr.into_boxed_slice())
        } else {
            Err(Error::UnexpectedEOF)
        }
    }
}

impl BEncode for &BStr {
    fn encode_into_stream(self, stream: &mut impl Write) -> std::io::Result<()> {
        stream.write_all(format!("{}", self.len()).as_bytes())?;
        stream.write_all(from_ref(&delimiters::STRING_INFIX))?;
        stream.write_all(self)?;

        Ok(())
    }
}

impl BDecode for BList {
    fn decode(bytes: &mut impl Iterator<Item = u8>) -> Result<Self> {
        if bytes.next() != Some(delimiters::LIST_PREFIX) {
            return Err(Error::InvalidFormat);
        };

        let mut peekable = bytes.by_ref().peekable();
        let mut list = vec![];

        loop {
            match peekable.peek() {
                Some(&delimiters::END_SUFFIX) => break,
                Some(_) => list.push(Entry::decode(&mut peekable)?),
                None => return Err(Error::UnexpectedEOF),
            };
        }

        Ok(list)
    }
}

impl BEncode for &BSlice {
    fn encode_into_stream(self, stream: &mut impl Write) -> std::io::Result<()> {
        stream.write_all(from_ref(&delimiters::LIST_PREFIX))?;

        for item in self {
            item.encode_into_stream(stream)?;
        }

        stream.write_all(from_ref(&delimiters::END_SUFFIX))?;

        Ok(())
    }
}

impl BDecode for BDictionary {
    fn decode(bytes: &mut impl Iterator<Item = u8>) -> Result<Self> {
        if bytes.next() != Some(delimiters::LIST_PREFIX) {
            return Err(Error::InvalidFormat);
        };

        let mut peekable = bytes.by_ref().peekable();
        let mut dictionary = HashMap::new();

        loop {
            let peek = peekable.peek();

            match peek {
                Some(&delimiters::END_SUFFIX) => break,
                Some(_) => {
                    let key = BString::decode(&mut peekable)?;
                    let value = Entry::decode(&mut peekable)?;

                    //MBDO: Treat repeated key/value pairs as error?
                    dictionary.insert(key, value);
                }
                None => return Err(Error::UnexpectedEOF),
            };
        }

        Ok(dictionary)
    }
}

impl<K: AsRef<BStr>> BEncode for &mut [(&K, &Entry)] {
    fn encode_into_stream(self, stream: &mut impl Write) -> std::io::Result<()> {
        utils::sort_key_value_entries(self);

        stream.write_all(from_ref(&delimiters::DICTIONARY_PREFIX))?;

        for (key, val) in self {
            key.as_ref().encode_into_stream(stream)?;
            val.encode_into_stream(stream)?;
        }

        stream.write_all(from_ref(&delimiters::END_SUFFIX))?;

        Ok(())
    }
}

impl BEncode for &BDictionary {
    fn encode(self) -> Box<[u8]> {
        self.into_iter().collect::<Vec<_>>().encode()
    }

    fn encode_into_stream(self, stream: &mut impl Write) -> std::io::Result<()> {
        self.into_iter()
            .collect::<Vec<_>>()
            .encode_into_stream(stream)
    }
}

pub enum Error {
    IO(std::io::Error),
    InvalidFormat,
    InvalidValue,
    UnexpectedEOF,
}

impl From<std::io::Error> for Error {
    fn from(inner: std::io::Error) -> Self {
        Self::IO(inner)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(_: std::str::Utf8Error) -> Self {
        Self::InvalidValue
    }
}

pub mod utils {
    pub fn sort_key_value_entries<K: AsRef<super::BStr>, V>(entries: &mut [(K, V)]) {
        entries.sort_by(|left, right| left.0.as_ref().cmp(right.0.as_ref()));
    }

    pub fn parse_utf8_bytes<T: std::str::FromStr>(bytes: &[u8]) -> super::Result<T> {
        std::str::from_utf8(bytes)?
            .parse::<T>()
            .map_err(|_| super::Error::InvalidValue)
    }

    pub fn collect_up_to(iter: &mut impl Iterator<Item = u8>, delimiter: u8) -> Vec<u8> {
        iter.by_ref()
            .take_while(|&b| b != delimiter)
            .collect::<Vec<_>>()
    }
}
