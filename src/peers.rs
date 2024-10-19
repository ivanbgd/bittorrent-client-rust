//! Peers
//!
//! https://www.bittorrent.org/beps/bep_0003.html#trackers
//!
//! https://www.bittorrent.org/beps/bep_0023.html
//!
//! https://wiki.theory.org/BitTorrentSpecification#Tracker_HTTP.2FHTTPS_Protocol

use std::path::PathBuf;

use anyhow::Result;
use serde_derive::Serialize;

use crate::constants::*;
use crate::decode::decode_bencoded_value;
use crate::meta_info::{meta_info, Mode};

pub fn get_peers(path: &PathBuf) -> Result<Vec<String>> {
    let meta = meta_info(path)?;
    let tracker = meta.announce;
    let info_hash = meta.info.info_hash;

    // The 20 byte sha1 hash of the bencoded form of the info value from the metainfo file.
    // This value will almost certainly have to be escaped.
    let info_hash = url_encode(&info_hash);

    let mode = meta.info.mode;
    let left = match mode {
        Mode::SingleFile { name: _, length } => length,
        Mode::MultipleFile { name: _, files: _ } => 0,
    };

    let client = reqwest::blocking::Client::new();

    let req = format!("{}/?info_hash={info_hash}", &tracker);

    let query = Query {
        peer_id: String::from(PEER_ID),
        port: PORT,
        uploaded: UPLOADED,
        downloaded: DOWNLOADED,
        left,
        compact: COMPACT,
    };

    let resp = client.get(&req).query(&query).send()?.bytes()?;
    let resp = Vec::from(resp);
    let resp_b_decoded = decode_bencoded_value(&resp)?;
    let _interval = &resp_b_decoded["interval"].to_string().parse::<usize>()?;
    let peers = &resp_b_decoded["peers"].to_string();
    let peers = peers.as_bytes();
    let peers_bytes = &peers[1..peers.len() - 1];
    let peers_len = peers_bytes.len();
    eprintln!("peers: {:?} --- {}", peers_bytes, peers_len);

    // We only support compact mode, but that is the recommended mode anyway, so it should be enough.
    // https://www.bittorrent.org/beps/bep_0023.html
    // The assignment itself only supports the compact mode.
    if peers_len % PEER_LEN != 0 {
        panic!(
            "Length of 'peers', {}, is not divisible by {} (compact mode).",
            peers_len, PEER_LEN
        );
    }
    let num_peers = peers_len / PEER_LEN;

    let mut peers: Vec<String> = Vec::with_capacity(num_peers);
    for peer in peers_bytes.chunks(PEER_LEN) {
        let port: u16 = (peer[4] as u16) << 8 | (peer[5] as u16);
        peers.push(format!(
            "{}.{}.{}.{}:{port}",
            peer[0], peer[1], peer[2], peer[3]
        ));
    }

    Ok(peers)
}

/// https://en.wikipedia.org/wiki/Percent-encoding
///
/// https://en.wikipedia.org/wiki/Percent-encoding#Types_of_URI_characters
fn url_encode(s: &str) -> String {
    let info = s.as_bytes();
    let mut res: String = String::with_capacity(2 * SHA1_LEN + SHA1_LEN);
    let mut i: usize = 0;
    while i < s.len() {
        let c1 = char::from(info[i]);
        let c2 = char::from(info[i + 1]);
        let pair = format!("{c1}{c2}");
        let urlenc = "%".to_string() + &pair;
        res.push_str(&urlenc);
        i += 2;
    }
    res
}

/// Query parameters for the HTTP GET request
///
/// *Note:* `info_hash` is deliberately omitted, because it isn't handled properly in the `reqwest::send()` function.
#[derive(Debug, Serialize)]
struct Query {
    /// A string of length 20 which this downloader uses as its id. Each downloader generates its own id at random
    /// at the start of a new download. This value will also almost certainly have to be escaped.
    peer_id: String,

    /// The port number this peer is listening on. Common behavior is for a downloader to try to listen on port 6881
    /// and if that port is taken try 6882, then 6883, etc. and give up after 6889.
    port: usize,

    /// The total amount uploaded so far, encoded in base ten ascii.
    uploaded: usize,

    /// The total amount downloaded so far, encoded in base ten ascii.
    downloaded: usize,

    /// The number of bytes this peer still has to download, encoded in base ten ascii.
    /// Note that this can't be computed from downloaded and the file length since it might be a resume,
    /// and there's a chance that some of the downloaded data failed an integrity check and had to be re-downloaded.
    left: usize,

    /// Setting this to 1 indicates that the client accepts a compact response.
    /// The peers list is replaced by a peers string with 6 bytes per peer.
    /// The first four bytes are the host (in network byte order),
    /// the last two bytes are the port (again in network byte order).
    /// It should be noted that some trackers only support compact responses (for saving bandwidth)
    /// and either refuse requests without "compact=1" or simply send a compact response unless the request
    /// contains "compact=0" (in which case they will refuse the request.)
    compact: usize,
}

// #[derive(Debug)] //, serde_derive::Deserialize)]
// struct Response {
//     interval: usize,
//     peers: String,
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore = "changes over time"]
    #[test]
    fn get_peers_sample_torrent() {
        let peers = get_peers(&PathBuf::from("sample.torrent")).unwrap();
        let mut res = String::with_capacity(peers.len());
        for peer in peers.iter() {
            res += peer;
            res += "\n";
        }
        assert_eq!(
            "165.232.35.114:51533\n165.232.38.164:51596\n165.232.41.73:51451\n",
            res
        );
    }
}
