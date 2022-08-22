
use super::BInt;
pub type BStr = [u8];
pub type BString = Box<[u8]>;


#[cfg(feature = "custom-bencode")]
use super::encoding::*;

#[cfg(feature = "custom-bencode")]
impl Metainfo {
    ///Parses deencoded metadata file and returns `Self`
    pub fn parse(entry: Entry) -> Result<Self> {
        let mut metainfo = entry.parse_or_err(Error::InvalidFormat("metainfo"))?;

        let info = utils::parse_required(&mut metainfo, "info", Info::parse)?;
        let announce = utils::parse_required_primitive(&mut metainfo, "announce")?;

        let announce_list = Self::parse_announce_list(utils::parse_optional_primitive(
            &mut metainfo,
            "announce-list",
        ));
        let creation_date = utils::parse_optional_primitive(&mut metainfo, "creation date")
            .map(|secs| NaiveDateTime::from_timestamp(secs, 0));
        let comment = utils::parse_optional_primitive(&mut metainfo, "comment");
        let created_by = utils::parse_optional_primitive(&mut metainfo, "created by");
        let encoding = utils::parse_optional_primitive(&mut metainfo, "encoding");

        Ok(Self {
            info,
            announce,
            announce_list,
            creation_date,
            comment,
            created_by,
            encoding,
        })
    }

    fn parse_announce_list(blist: Option<BList>) -> Option<Vec<Vec<String>>> {
        let tiers = blist?
            .into_iter()
            .filter_map(Entry::parse::<BList>)
            .map(|tier_list| {
                tier_list
                    .into_iter()
                    .map(Entry::parse::<String>)
            })
            .filter_map(Iterator::collect::<Option<Vec<_>>>)
            .collect();

        Some(tiers)
    }
}

#[cfg(feature = "custom-bencode")]
impl Info {
    pub fn parse(entry: Entry) -> Result<Self> {
        let mut info = entry.parse_or_err(Error::InvalidFormat("info"))?;

        let piece_length = utils::parse_required_primitive(&mut info, "piece length")?;
        let pieces = utils::parse_required_primitive(&mut info, "pieces")?;
        let name = utils::parse_required_primitive(&mut info, "name")?;

        let private =
            utils::parse_optional_primitive::<BInt>(&mut info, "private").map(|i| i == 1);

        let files = Self::parse_file_info(&mut info)?;

        Ok(Self {
            piece_length,
            pieces,
            private,
            name,
            files,
        })
    }

    fn parse_file_info(info: &mut BDictionary) -> Result<Vec<FileInfo>> {
        if !info.contains_key("files".as_bytes()) {
            let length = utils::parse_required_primitive(info, "length")?;
            let md5sum = utils::parse_optional_primitive(info, "md5sum");

            Ok(vec![FileInfo {
                length,
                md5sum,
                path: Vec::new(),
            }])
        } else {
            let entries = utils::parse_required_primitive::<BList>(info, "files")?;

            let files = entries
                .into_iter()
                .map(FileInfo::parse)
                .collect::<Result<Vec<_>>>()?;

            Ok(files)
        }
    }
}

#[cfg(feature = "custom-bencode")]
impl FileInfo {
    pub fn parse(entry: Entry) -> Result<Self> {
        let mut info = entry.parse_or_err(Error::InvalidFormat("files"))?;

        let path = utils::parse_required_primitive::<BList>(&mut info, "path")?
            .into_iter()
            .map(|entry| String::try_from(entry).map_err(|_| Error::InvalidFormat("path")))
            .collect::<Result<Vec<_>>>()?;
        let length = utils::parse_required_primitive(&mut info, "length")?;
        let md5sum = utils::parse_optional_primitive(&mut info, "md5sum");

        Ok(Self {
            length,
            md5sum,
            path,
        })
    }
}

mod utils {
    use super::*;

    #[cfg(feature = "custom-bencode")]
    pub fn parse_optional_primitive<T: TryFrom<Entry>>(
        dictionary: &mut BDictionary,
        key: &str,
    ) -> Option<T> {
        dictionary
            .remove(key.as_bytes())
            .map(|entry| entry.parse::<T>())
            .flatten()
    }

    #[cfg(feature = "custom-bencode")]
    pub fn parse_required_primitive<T>(dictionary: &mut BDictionary, key: &'static str) -> Result<T>
    where
        Entry: TryInto<T>,
    {
        dictionary
            .remove(key.as_bytes())
            .map(|entry| entry.parse::<T>())
            .ok_or(Error::MissingField(key))?
            .ok_or(Error::InvalidFormat(key))
    }

    #[cfg(feature = "custom-bencode")]
    pub fn parse_required<T>(
        dictionary: &mut BDictionary,
        key: &'static str,
        parser: impl FnOnce(Entry) -> Result<T>,
    ) -> Result<T> {
        dictionary
            .remove(key.as_bytes())
            .ok_or(Error::MissingField(key))
            .map(parser)?
    }
}
