use super::{Parser, Saver, BString};
use serde::{de::DeserializeOwned, Serialize};
use serde_bencoded::{DeError, SerError};
use std::io::{self, Read, Write};

impl From<serde_bytes::ByteBuf> for BString {
    fn from(bytes: serde_bytes::ByteBuf) -> Self {
        Self(bytes.into_vec())
    }
}

impl Into<serde_bytes::ByteBuf> for BString {
    fn into(self) -> serde_bytes::ByteBuf {
        serde_bytes::ByteBuf::from(self.0)
    }
}

/// Used for parsing and saving beencoded structures with `serde` (see [`Parser`], [`Saver`]).
///
/// ## Note
///
/// Currently parsing in stream-like fassion is not supported due to limitations of serde backend inmplementation,
/// but it can change in the future (it reads all contents of stream imediately). Until that moment, consumer should
/// keep this fact in mind when parsing huge models, although in practical environment these tend not to exceed
/// 70KB in size, which is afordable amount of runtime memory allocation in most cases.
pub struct Serde;

impl<D: DeserializeOwned> Parser<D> for Serde {
    type Err = ParseError;
    ///
    /// ## Errors
    ///
    /// For information on failure cases see [`serde_bencoded::DeError`].
    fn parse(&self, mut source: impl Read) -> Result<D, Self::Err> {
        let mut bytes = vec![];
        source.read_to_end(&mut bytes)?;

        serde_bencoded::from_bytes(&bytes).map_err(Into::into)
    }
}

#[derive(Debug)]
pub enum ParseError {
    IO(io::Error),
    De(DeError),
}

impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}
impl From<DeError> for ParseError {
    fn from(err: DeError) -> Self {
        Self::De(err)
    }
}

impl<T: Serialize> Saver<T> for Serde {
    type Err = SerError;
    /// ## Errors
    ///
    /// For information on failure cases see [`serde_bencoded::SerError`].
    fn save(&self, item: &T, target: impl Write) -> Result<(), Self::Err> {
        serde_bencoded::to_writer(item, target)
    }
}

#[cfg(test)]
mod test {
    use std::fmt::Debug;

    use super::super::*;
    use super::*;
    use hex_literal::hex;
    use rstest::*;

    static SAMPLE_TORRENT: &[u8] = include_bytes!("sample.torrent");

    #[fixture]
    fn info() -> Info {
        Info {
            piece_length: 65536,
            pieces: BString(Vec::from(hex!(
                "5cc5e652be0de6f27805b30464ff9b00f489f0c9"
            ))),
            private: Some(true),
            name: "sample.txt".to_owned(),
            files: Files::Single {
                length: 20,
                md5sum: None,
            },
        }
    }

    #[fixture]
    #[once]
    fn metainfo(info: Info) -> Metainfo {
        Metainfo {
            info,
            announce: "udp://tracker.openbittorrent.com:80".to_owned(),
            announce_list: None,
            creation_date: Some(1327049827),
            comment: None,
            created_by: None,
            encoding: None,
        }
    }

    #[rstest]
    #[case::metainfo(metainfo(info()), SAMPLE_TORRENT)]
    fn decoding<T: PartialEq + Debug>(#[case] item: T, #[case] bytes: &[u8])
    where
        Serde: Parser<T>,
        <Serde as Parser<T>>::Err: Debug,
    {
        let decoded: T = Serde.parse(bytes).unwrap();
        assert_eq!(decoded, item);
    }

    #[rstest]
    #[case::metainfo(metainfo(info()), SAMPLE_TORRENT)]
    fn encoding<T>(#[case] item: T, #[case] bytes: &[u8])
    where
        Serde: Saver<T>,
        <Serde as Saver<T>>::Err: Debug,
    {
        let mut encoded = vec![];
        Serde.save(&item, &mut encoded).unwrap();
        assert_eq!(
            &encoded,
            bytes
        );
    }
}
