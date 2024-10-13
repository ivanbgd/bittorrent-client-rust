use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::decode::decode_bencoded_value;
use crate::hash::calc_sha1;

/// Metainfo File Structure
///
/// All data in a metainfo file is bencoded.
///
/// The content of a metainfo file (the file ending in ".torrent") is a bencoded dictionary,
/// containing the keys listed below. All character string values are UTF-8 encoded.
///
/// https://wiki.theory.org/BitTorrentSpecification#Metainfo_File_Structure
///
/// https://www.bittorrent.org/beps/bep_0003.html#metainfo-files
#[derive(Debug)]
pub struct MetaInfo {
    /// The "announce" URL of the tracker (string)
    pub announce: String,

    /// Created by: (optional) name and version of the program used to create the .torrent (string)
    pub created_by: String,

    /// Info Dictionary: a dictionary that describes the file(s) of the torrent.
    pub info: Info,
}

/// Info Dictionary
///
/// A dictionary that describes the file(s) of the torrent. There are two possible forms:
/// one for the case of a 'single-file' torrent with no directory structure,
/// and one for the case of a 'multi-file' torrent.
///
/// We use the single-file variant.
///
/// https://wiki.theory.org/BitTorrentSpecification#Info_Dictionary
#[allow(dead_code)]
#[derive(Debug)]
pub struct Info {
    /// Single-file or multiple-file torrent
    mode: Mode,

    /// piece length: number of bytes in each piece (integer)
    plen: usize,

    /// pieces: string consisting of the concatenation of all 20-byte SHA1 hash values,
    /// one per piece (byte string, i.e. not urlencoded)
    pieces: String,
}

/// Single-file or multiple-file torrent
#[derive(Debug)]
pub enum Mode {
    SingleFile { length: usize, name: String },
    MultipleFile { name: String },
}

pub fn meta_info(path: &PathBuf) -> Result<MetaInfo> {
    let contents: Vec<u8> = fs::read(path)?;
    let contents: &[u8] = contents.as_ref();

    let decoded = decode_bencoded_value(contents)?;

    let announce = &decoded["announce"].to_string();
    let announce = String::from(&announce[1..announce.len() - 1]);

    let created_by = match &decoded.get("created by") {
        Some(created_by) => {
            let created_by = created_by.to_string();
            String::from(&created_by[1..created_by.len() - 1])
        }
        None => "".to_string(),
    };

    let info = &decoded["info"];

    let length = info["length"].to_string().parse::<usize>()?;
    let name = info["name"].to_string();
    let name = String::from(&name[1..name.len() - 1]);
    let mode = Mode::SingleFile { length, name };
    let plen = info["piece length"].to_string().parse::<usize>()?;
    let pieces = calc_sha1(info)?;

    Ok(MetaInfo {
        announce,
        created_by,
        info: Info { mode, plen, pieces },
    })
}

impl Display for MetaInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let length = match &self.info.mode {
            Mode::SingleFile { length, name: _ } => length,
            Mode::MultipleFile { name: _ } => &0,
        };

        write!(
            f,
            "Tracker URL: {}\nLength: {}\nInfo Hash: {}\n",
            self.announce, length, self.info.pieces
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_sample() {
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Length: 92063\nInfo Hash: d69f91e6b2ae4c542468d1073a71d4ea13879a7f\n",
            format!("{}", meta_info(&PathBuf::from("sample.torrent")).unwrap())
        );
    }

    #[test]
    fn info_codercat() {
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Length: 2994120\nInfo Hash: c77829d2a77d6516f88cd7a3de1a26abcbfab0db\n",
            format!(
                "{}",
                meta_info(&PathBuf::from("test_samples/codercat.gif.torrent")).unwrap()
            )
        );
    }

    #[test]
    fn info_congratulations() {
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Length: 820892\nInfo Hash: 1cad4a486798d952614c394eb15e75bec587fd08\n",
            format!(
                "{}",
                meta_info(&PathBuf::from("test_samples/congratulations.gif.torrent")).unwrap()
            )
        );
    }

    #[test]
    fn info_itsworking() {
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Length: 2549700\nInfo Hash: 70edcac2611a8829ebf467a6849f5d8408d9d8f4\n",
            format!(
                "{}",
                meta_info(&PathBuf::from("test_samples/itsworking.gif.torrent")).unwrap()
            )
        );
    }
}
