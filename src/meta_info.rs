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
/// There is a key `length` or a key `files`, but not both or neither.
/// If `length` is present then the download represents a single file,
/// otherwise it represents a set of files which go in a directory structure.
///
/// We keep those fields inside the `mode` field.
///
/// https://wiki.theory.org/BitTorrentSpecification#Info_Dictionary
///
/// https://www.bittorrent.org/beps/bep_0003.html#metainfo-files
#[allow(dead_code)]
#[derive(Debug)]
pub struct Info {
    /// Single-file or multiple-file torrent
    mode: Mode,

    /// Piece length: number of bytes in each piece (integer)
    plen: usize,

    /// Pieces: string consisting of the concatenation of all 20-byte SHA1 hash values,
    /// one per piece (byte string, i.e. not urlencoded)
    pieces: String,
}

/// Single-file or multiple-file torrent
///
/// In the single file case, length maps to the length of the file in bytes.
///
/// In the single file case, the name key is the name of a file.
/// In the multiple file case, it's the name of a directory.
#[derive(Debug)]
pub enum Mode {
    SingleFile { name: String, length: usize },
    MultipleFile { name: String, files: Files },
}

/// A list of dictionaries, one for each file
#[allow(dead_code)]
#[derive(Debug)]
pub struct Files {
    /// A list containing one or more string elements that together represent the path and filename
    ///
    /// Each element in the list corresponds to either a directory name or, in the case of the final element,
    /// the filename.
    ///
    /// For example, the file "dir1/dir2/file.ext" would consist of three string elements:
    /// "dir1", "dir2", and "file.ext".
    ///
    /// This is encoded as a bencoded list of strings such as: `l4:dir14:dir28:file.exte`
    ///
    /// A list of UTF-8 encoded strings corresponding to subdirectory names,
    /// the last of which is the actual file name (a zero length list is an error case).
    path: Vec<String>,

    /// Length of the file in bytes (integer)
    length: usize,
}

pub fn meta_info(path: &PathBuf) -> Result<MetaInfo> {
    let contents: Vec<u8> = fs::read(path)?;
    let contents: &[u8] = contents.as_ref();

    let decoded = decode_bencoded_value(contents)?;

    let announce = &decoded["announce"].to_string();
    let announce = String::from(&announce[1..announce.len() - 1]);

    let created_by = match decoded.get("created by") {
        Some(created_by) => {
            let created_by = created_by.to_string();
            String::from(&created_by[1..created_by.len() - 1])
        }
        None => "".to_string(),
    };

    let info = &decoded["info"];

    let name = info["name"].to_string();
    let name = String::from(&name[1..name.len() - 1]);

    let mode = if let Some(length) = info.get("length") {
        let length = length.to_string().parse::<usize>()?;
        Mode::SingleFile { name, length }
    } else if let Some(_files) = info.get("files") {
        let files = Files {
            path: vec![],
            length: 0,
        };
        Mode::MultipleFile { name, files }
    } else {
        panic!("Either 'length' or 'files' field must be present in the torrent file, but none is.")
    };

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
            Mode::SingleFile { name: _, length } => length,
            Mode::MultipleFile { name: _, files: _ } => &0,
        };

        write!(
            f,
            "Tracker URL: {}\nLength: {}\nInfo Hash: {}\nMode: {:?}",
            self.announce, length, self.info.pieces, self.info.mode
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
