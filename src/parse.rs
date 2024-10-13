use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;

use crate::decode::decode_bencoded_value;
use crate::hash::calc_sha1;
use anyhow::Result;

/// Metainfo File Structure
///
/// All data in a metainfo file is bencoded.
///
/// The content of a metainfo file (the file ending in ".torrent") is a bencoded dictionary,
/// containing the keys listed below. All character string values are UTF-8 encoded.
///
/// https://wiki.theory.org/BitTorrentSpecification#Metainfo_File_Structure
#[derive(Debug)]
pub struct MetaInfo {
    /// The "announce" URL of the tracker
    pub announce: String,

    /// Info Dictionary
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
    /// piece length: number of bytes in each piece (integer)
    plen: usize,

    /// pieces: string consisting of the concatenation of all 20-byte SHA1 hash values,
    /// one per piece (byte string, i.e. not urlencoded)
    pieces: String,

    /// Single-file or multiple-file torrent
    mode: Mode,
}

/// Single-file or multiple-file torrent
#[derive(Debug)]
pub enum Mode {
    SingleFile { name: String, length: usize },
    MultipleFile { name: String },
}

pub fn meta_info(path: &PathBuf) -> Result<MetaInfo> {
    let contents: Vec<u8> = fs::read(path)?;
    let contents: &[u8] = contents.as_ref();

    let decoded = decode_bencoded_value(contents)?;

    let announce = &decoded["announce"].to_string();
    let announce = String::from(&announce[1..announce.len() - 1]);

    let info = &decoded["info"];

    let plen = info["length"].to_string();
    let plen = plen.parse::<usize>()?;

    let pieces = calc_sha1(info)?;

    // TODO
    let mode = Mode::SingleFile {
        name: String::new(),
        length: 0,
    };

    Ok(MetaInfo {
        announce,
        info: Info { plen, pieces, mode },
    })
}

impl Display for MetaInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tracker URL: {}\nLength: {}\nInfo Hash: {}\n",
            self.announce, self.info.plen, self.info.pieces
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
