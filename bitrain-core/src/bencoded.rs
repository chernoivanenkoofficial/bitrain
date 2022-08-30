#[cfg(feature = "custom-bencode")]
mod custom;

use std::io::{Read, Write};

#[cfg(feature = "custom-bencode")]
pub use encoding::{BDecode, BEncode};

#[cfg(feature = "use-serde")]
mod serde;
pub use self::serde::*;

#[cfg(feature = "use-serde")]
use serde_derive::{Deserialize, Serialize};

///Bencoded int type.
pub type BInt = u64;

///Bencoded string type.
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "use-serde", serde(into = "serde_bytes::ByteBuf"))]
#[cfg_attr(feature = "use-serde", serde(from = "serde_bytes::ByteBuf"))]
#[derive(Debug, Clone, PartialEq)]
pub struct BString(pub Vec<u8>);

impl BString {
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

pub trait Parser<T>: Sized {
    type Err;

    fn parse(&self, source: impl Read) -> Result<T, Self::Err>;
}

pub trait Saver<T>: Sized {
    type Err;

    fn save(&self, item: &T, target: impl Write) -> Result<(), Self::Err>;
}

#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
///Parsed `.torrent` metadata file
#[derive(Debug, Clone, PartialEq)]
pub struct Metainfo {
    ///Describes the file(s) of the torrent.
    pub info: Info,
    ///The announce URL of the tracker.
    pub announce: String,
    ///The list of tracker tiers.
    ///
    ///See <http://bittorrent.org/beps/bep_0012.html> for more info.
    #[cfg_attr(feature = "use-serde", serde(rename = "announce-list"))]
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    pub announce_list: Option<Vec<Vec<String>>>,
    ///The creation time of the torrent.
    #[cfg_attr(feature = "use-serde", serde(rename = "creation date"))]
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    pub creation_date: Option<BInt>,
    ///Free-form textual comments of the author.
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    pub comment: Option<String>,
    ///Name and version of the program used to create the metadata file.
    #[cfg_attr(feature = "use-serde", serde(rename = "created by"))]
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    pub created_by: Option<String>,
    ///The string encoding format used to generate the pieces part of the info dictionary in the metadata file.
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    pub encoding: Option<String>,
}

///Parsed `info` section of `.torrent` metadata file.
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    ///Number of bytes in each piece.
    ///
    /// # Note
    ///
    ///For the purposes of piece boundaries in the multi-file case,
    ///the file data is considered as one long continuous stream,
    ///composed of the concatenation of each file in the order listed in the files list.
    ///The number of pieces and their boundaries are then determined
    ///in the same manner as the case of a single file. Pieces may overlap file boundaries.
    #[cfg_attr(feature = "use-serde", serde(rename = "piece length"))]
    pub piece_length: BInt,
    ///Byte string consisting of the concatenation of 20-byte SHA1 hash values, one per each piece.
    pub pieces: BString,
    ///If this field is set to `Some(true)`, the client is supposed to publish its presence to get other peers
    ///only via the trackers explicitly described in the metainfo file.
    ///Otherwise, the client may obtain peer by other means, e.g. PEX peer exchange, dht.
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    pub private: Option<bool>,
    ///The filename or the name of the root directory in which to store all the files.
    pub name: String,
    ///A list of files in this torrent.
    #[cfg_attr(feature = "use-serde", serde(flatten))]
    pub files: Files,
}

#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "use-serde", serde(untagged))]
#[derive(Debug, Clone, PartialEq)]
pub enum Files {
    Multiple {
        files: Vec<FileInfo>,
    },
    Single {
        length: BInt,
        #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
        md5sum: Option<BString>,
    },
}

///Info about file in torrent.
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct FileInfo {
    ///Length of the file in bytes.
    pub length: BInt,
    ///An optional 32-character hexadecimal string corresponding to the MD5 sum of the file.
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    pub md5sum: Option<BString>,
    ///A list containing one or more string elements that together represent the path and filename.
    ///Each element in the list corresponds to either a directory name or (in the case of the final element) the filename.
    pub path: Vec<String>,
}

#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "use-serde", serde(untagged))]
#[derive(Debug, Clone, PartialEq)]
pub enum TrackerResponce {
    Success {
        #[cfg_attr(feature = "use-serde", serde(flatten))]
        info: TrackerInfo,
        peers: PeerList
    },
    Error {
        #[cfg_attr(feature = "use-serde", serde(rename = "failure reason"))]
        failure_reason: BString,
    },
}

#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct TrackerInfo {
    interval: BInt,
    #[cfg_attr(feature = "use-serde", serde(rename = "min interval"))]
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    min_interval: Option<BInt>,
    #[cfg_attr(feature = "use-serde", serde(rename = "tracker id"))]
    #[cfg_attr(feature = "use-serde", serde(skip_serializing_if = "Option::is_none"))]
    id: Option<BString>,
    complete: BInt,
    incomplete: BInt,    
}

#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "use-serde", serde(untagged))]
#[derive(Debug, Clone, PartialEq)]
pub enum PeerList {
    Canonical(Vec<PeerCanonical>),
    Compact(BString),    
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
pub struct PeerCanonical {
    #[cfg_attr(feature = "use-serde", serde(rename = "peer id"))]
    id: BString,
    ip: BString,
    port: BInt,
}