use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::constants::SHA1_LEN;
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
/// We hold those fields inside the `mode` field.
///
/// We also add `info_hash` to hold SHA1 sum of the Info dictionary.
///
/// https://wiki.theory.org/BitTorrentSpecification#Info_Dictionary
///
/// https://www.bittorrent.org/beps/bep_0003.html#metainfo-files
#[derive(Debug)]
pub struct Info {
    /// Single-file or multiple-file torrent
    mode: Mode,

    /// Piece length: number of bytes in each piece (integer)
    plen: usize,

    /// `pieces`: string consisting of the concatenation of all 20-byte SHA1 hash values,
    /// one per piece (byte string, i.e. not urlencoded)
    ///
    /// We keep it as a vector of strings, where each element corresponds to one piece.
    /// So, each element is a 20-byte SHA1 hash sum.
    /// This is more efficient, because we should parse the torrent file only once,
    /// and then we can use all these fields multiple times without new calculations.
    pieces: Vec<String>,

    /// SHA1 sum of the Info dictionary
    info_hash: String,
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
    let contents_vec: Vec<u8> = fs::read(path)?;
    let contents: &[u8] = contents_vec.as_ref();

    let contents_b_decoded = decode_bencoded_value(contents)?;

    let announce = &contents_b_decoded["announce"].to_string();
    let announce = String::from(&announce[1..announce.len() - 1]);

    let created_by = match contents_b_decoded.get("created by") {
        Some(created_by) => {
            let created_by = created_by.to_string();
            String::from(&created_by[1..created_by.len() - 1])
        }
        None => "".to_string(),
    };

    let info = &contents_b_decoded["info"];

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

    let pieces = &info["pieces"];
    let pieces_b_encoded = &serde_bencode::to_bytes(pieces)?;
    let start = pieces_b_encoded
        .iter()
        .position(|elt| elt.eq(&b':')) // b':' == 0x3a == 58
        .expect("expected a ':'")
        + 1;
    let pieces_string = pieces_b_encoded[start..].to_vec();
    let pcs_len = pieces_string.len();
    if pcs_len % SHA1_LEN != 0 {
        panic!(
            "Length of 'pieces', {}, is not divisible by SHA1 sum length, which is {}.",
            pcs_len, SHA1_LEN
        );
    }
    let hex_pieces_string = hex::encode(pieces_string);
    let num_pcs = pcs_len / SHA1_LEN;
    let mut pieces: Vec<String> = Vec::with_capacity(num_pcs);
    let mut cur = hex_pieces_string.as_str();
    while !cur.is_empty() {
        let (piece, remainder) = cur.split_at(2 * SHA1_LEN);
        pieces.push(piece.parse()?);
        cur = remainder;
    }

    let info_hash = calc_sha1(info)?;

    Ok(MetaInfo {
        announce,
        created_by,
        info: Info {
            mode,
            plen,
            pieces,
            info_hash,
        },
    })
}

impl Display for MetaInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let length = match &self.info.mode {
            Mode::SingleFile { name: _, length } => length,
            Mode::MultipleFile { name: _, files: _ } => &0,
        };

        let pieces = &self.info.pieces;
        let hashes_len = pieces.len() * 2 * SHA1_LEN + pieces.len();
        let mut piece_hashes: String = String::with_capacity(hashes_len);
        for piece in pieces {
            piece_hashes += piece;
            piece_hashes += "\n";
        }

        write!(
            f,
            "Tracker URL: {}\nLength: {}\nInfo Hash: {}\nPiece Length: {}\nPiece Hashes:\n{}",
            self.announce, length, self.info.info_hash, self.info.plen, piece_hashes
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
            Length: 92063\nInfo Hash: d69f91e6b2ae4c542468d1073a71d4ea13879a7f\n\
            Piece Length: 32768\nPiece Hashes:\n\
            e876f67a2a8886e8f36b136726c30fa29703022d\n\
            6e2275e604a0766656736e81ff10b55204ad8d35\n\
            f00d937a0213df1982bc8d097227ad9e909acc17\n",
            format!("{}", meta_info(&PathBuf::from("sample.torrent")).unwrap())
        );
    }

    #[test]
    fn info_codercat() {
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Length: 2994120\nInfo Hash: c77829d2a77d6516f88cd7a3de1a26abcbfab0db\n\
            Piece Length: 262144\nPiece Hashes:\n\
            3c34309faebf01e49c0f63c90b7edcc2259b6ad0\n\
            b8519b2ea9bb373ff567f644428156c98a1d00fc\n\
            9dc81366587536f48c2098a1d79692f2590fd9a6\n\
            033c61e717f8c0d1e55850680eb451e3543b6203\n\
            6f54e746ec369f65f32d45f77b1f1c37621fb965\n\
            c656704b78107ed553bd0813f92fef780267c07b\n\
            7431b8683137d20ff594b1f1bf3f8835165d68fb\n\
            0432bd8e779608d27782b779c7738062e9b50ab5\n\
            d6bc0409a0f3a9503857669d47fe752d4577ea00\n\
            a86ee6abbc30cddb800a0b62d7a296111166d839\n\
            783f52b70f0c902d56196bd3ee7f379b5db57e3b\n\
            3d8db9e34db63b4ba1be27930911aa37b3f997dd\n",
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
            Length: 820892\nInfo Hash: 1cad4a486798d952614c394eb15e75bec587fd08\n\
            Piece Length: 262144\nPiece Hashes:\n\
            3d42a20edb1cf840cd3528d3a9e921db6338a463\n\
            69f885b3988a52ffb03591985402b6d5285940ab\n\
            76869e6c9c1f101f94f39de153e468be6a638f4f\n\
            bded68d02de011a2b687f75b5833f46cce8e3e9c\n",
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
            Length: 2549700\nInfo Hash: 70edcac2611a8829ebf467a6849f5d8408d9d8f4\n\
            Piece Length: 262144\nPiece Hashes:\n\
            01cc17bbe60fa5a52f64bd5f5b64d99286d50aa5\n\
            838f703cf7f6f08d1c497ed390df78f90d5f7566\n\
            45bf10974b5816491e30628b78a382ca36c4e05f\n\
            84be4bd855b34bcedc0c6e98f66d3e7c63353d1e\n\
            86427ac94d6e4f21a6d0d6c8b7ffa4c393c3b131\n\
            7c70cd5f44d1ac5505cb855d526ceb0f5f1cd5e3\n\
            3796ab05af1fa874173a0a6c1298625ad47b4fe6\n\
            272a8ff8fc865b053d974a78681414b38077d7b1\n\
            b07128d3a6018062bfe779db96d3a93c05fb81d4\n\
            7affc94f0985b985eb888a36ec92652821a21be4\n",
            format!(
                "{}",
                meta_info(&PathBuf::from("test_samples/itsworking.gif.torrent")).unwrap()
            )
        );
    }
}
