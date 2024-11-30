//! # Tracker Request & Response, and Peers
//!
//! https://www.bittorrent.org/beps/bep_0003.html#trackers
//!
//! https://www.bittorrent.org/beps/bep_0023.html
//!
//! https://wiki.theory.org/BitTorrentSpecification#Tracker_HTTP.2FHTTPS_Protocol
//!
//! Currently, only the single-file torrents are supported.
//!
//! Also, only the compact mode for peers is supported, but this is the only recommended mode in practice anyway.
//! https://www.bittorrent.org/beps/bep_0023.html
//! The assignment itself only supports the compact mode.
//!
//! `$ ./your_bittorrent.sh peers sample.torrent`
//!
//! `165.232.41.73:51556`
//!
//! `165.232.38.164:51532`
//!
//! `165.232.35.114:51437`

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constants::*;
use crate::errors::TrackerError;
use crate::meta_info::{meta_info, Info};
use crate::tracker::peers::Peers;

/// Fetches and returns the peers list and the decoded [`Info`] section (dictionary) of the torrent file.
///
/// Reads a torrent file, extracts its contents (meta info), sends an HTTP GET request with query
/// parameters obtained from the meta info to the tracker (a server), from which it then gets the peers
/// list in the response, in the compact mode.
///
/// The reason for including the [`Info`] dictionary in the return value is as an optimization,
/// because it can then be reused in higher-level functions that call this function,
/// such as [`crate::peer_comm::download_piece`], for example.
/// It contains info hash that is 20 bytes long plain SHA1 hash sum of the [`Info`] dictionary from the torrent file.
///
/// # Errors
/// - [`crate::errors::MetaInfoError`]
/// - [`reqwest::Error`]
/// - [`serde_bencode::Error`]
///
/// All error types are wrapped in [`TrackerError`].
pub async fn get_peers(torrent: &PathBuf) -> Result<(Peers, Info), TrackerError> {
    let meta = meta_info(torrent)?;
    let tracker = meta.announce;

    // The 20 byte sha1 hash of the bencoded form of the info value from the metainfo file.
    // This value will almost certainly have to be escaped.
    // Plain byte representation of the SHA1 sum of the Info dictionary, 20 bytes long.
    let info_hash = &meta.info.info_hash;

    // Percent-encoded variant of the 20 byte long Info hash (not hex-encoded).
    let info_hash = url_encode(info_hash);

    // Currently, only the single-file torrents are supported.
    let left = meta.info.length();

    let client = reqwest::Client::new();

    let req = format!("{}/?info_hash={info_hash}", &tracker);

    let query = TrackerRequest {
        peer_id: String::from(PEER_ID),
        port: PORT,
        uploaded: UPLOADED,
        downloaded: DOWNLOADED,
        left,
        compact: COMPACT,
    };

    let resp = client.get(&req).query(&query).send().await?.bytes().await?;
    let resp = Vec::from(resp);
    let response: TrackerResponse = serde_bencode::from_bytes(&resp)?;

    Ok((response.peers, meta.info))
}

/// Uses the crate [`urlencoding`] for percent-encoding of the input.
///
/// Example use: Pass it a 20 byte long slice of bytes (such as a plain byte representation of the SHA1 sum
/// of the Info dictionary, so not hex-encoded).
///
/// For percent-encoding explanation see:
/// - https://en.wikipedia.org/wiki/Percent-encoding
/// - https://en.wikipedia.org/wiki/Percent-encoding#Types_of_URI_characters
pub(crate) fn url_encode(s: &[u8]) -> String {
    urlencoding::encode_binary(s).into_owned()
}

/// My own implementation of percent-encoding.
///
/// Expects a hex-encoded string as input.
///
/// Example use: Pass it a 40 byte long hex encoded string (such as a hexadecimal string representation of the SHA1 sum
/// of the Info dictionary).
///
/// For percent-encoding explanation see:
/// - https://en.wikipedia.org/wiki/Percent-encoding
/// - https://en.wikipedia.org/wiki/Percent-encoding#Types_of_URI_characters
fn _url_encode(s: &str) -> String {
    let info = s.as_bytes();
    let mut res: String = String::with_capacity(s.len() + s.len() / 2);
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
pub(crate) struct TrackerRequest {
    /// A string of length 20 which this downloader uses as its id. Each downloader generates its own id at random
    /// at the start of a new download. This value will also almost certainly have to be escaped.
    pub(crate) peer_id: String,

    /// The port number this peer is listening on. Common behavior is for a downloader to try to listen on port 6881
    /// and if that port is taken try 6882, then 6883, etc. and give up after 6889.
    pub(crate) port: u16,

    /// The total amount uploaded so far, encoded in base ten ascii.
    pub(crate) uploaded: usize,

    /// The total amount downloaded so far, encoded in base ten ascii.
    pub(crate) downloaded: usize,

    /// The number of bytes this peer still has to download, encoded in base ten ascii.
    /// Note that this can't be computed from downloaded and the file length since it might be a resume,
    /// and there's a chance that some of the downloaded data failed an integrity check and had to be re-downloaded.
    pub(crate) left: usize,

    /// Setting this to 1 indicates that the client accepts a compact response.
    /// The peers list is replaced by a peers string with 6 bytes per peer.
    /// The first four bytes are the host (in network byte order),
    /// the last two bytes are the port (again in network byte order).
    /// It should be noted that some trackers only support compact responses (for saving bandwidth)
    /// and either refuse requests without "compact=1" or simply send a compact response unless the request
    /// contains "compact=0" (in which case they will refuse the request.)
    pub(crate) compact: u8,
}

/// The tracker responds with "text/plain" document consisting of a bencoded dictionary with the following keys:
/// - `interval`: Interval in seconds that the client should wait between sending regular requests to the tracker
/// - `peers`: (binary model) A string consisting of multiples of 6 bytes.
///   First 4 bytes are the IP address and last 2 bytes are the port number. All in network (big endian) notation.
///   See [BEP 23](https://www.bittorrent.org/beps/bep_0023.html).
///
/// Tracker responses are b-encoded dictionaries. If a tracker response has a key failure reason, then that maps
/// to a human-readable string which explains why the query failed, and no other keys are required.
/// Otherwise, it must have two keys: `interval`, which maps to the number of seconds the downloader should wait
/// between regular re-requests, and `peers`. `peers` is a compact representation of the peer list.
#[derive(Debug, Deserialize)]
pub struct TrackerResponse {
    /// Interval in seconds that the client should wait between sending regular requests to the tracker.
    ///
    /// We can ignore this field for the purposes of this challenge.
    ///
    /// We are wrapping it in `Option<>` because sometimes their test server (tracker) doesn't send it.
    pub interval: Option<usize>,

    /// (binary model) A string consisting of multiples of 6 bytes.
    /// First 4 bytes are the IP address and last 2 bytes are the port number. All in network (big endian) notation.
    pub peers: Peers,
}

pub mod peers {
    //! A compact representation of the peer list

    use std::fmt::{Display, Formatter};
    use std::net::{Ipv4Addr, SocketAddrV4};

    use serde::de::{Deserialize, Deserializer, Error, Visitor};
    use serde::ser::{Serialize, Serializer};

    use crate::constants::{PEER_DISPLAY_LEN, PEER_LEN};

    /// Wrapper around a vector of peers' socket addresses
    #[derive(Debug)]
    pub struct Peers(pub Vec<SocketAddrV4>);

    impl Display for Peers {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let peers = &self.0;
            let res_len = PEER_DISPLAY_LEN * peers.len();
            let mut res = String::with_capacity(res_len);
            for peer in peers {
                res += &peer.to_string();
                res += "\n";
            }

            write!(f, "{}", res)
        }
    }

    struct PeersVisitor;

    impl<'de> Visitor<'de> for PeersVisitor {
        type Value = Peers;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            write!(
                formatter,
                "a byte string whose length is a multiple of {} (compact mode)",
                PEER_LEN
            )
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            let peers_len = v.len();
            // We only support compact mode, but that is the recommended mode anyway, so it should be enough.
            if peers_len % PEER_LEN == 0 {
                Ok(Peers(
                    v.chunks_exact(PEER_LEN)
                        .map(|peer| {
                            SocketAddrV4::new(
                                Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]),
                                u16::from_be_bytes([peer[4], peer[5]]),
                            )
                        })
                        .collect(),
                ))
            } else {
                Err(E::custom(format!(
                    "length of 'peers', {}, is not divisible by {} (compact mode)",
                    peers_len, PEER_LEN
                )))
            }
        }
    }

    impl<'de> Deserialize<'de> for Peers {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(PeersVisitor)
        }
    }

    // Not required in our case, but implemented for reference
    impl Serialize for Peers {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut seq = Vec::with_capacity(PEER_LEN * self.0.len());
            for peer in &self.0 {
                seq.extend(peer.ip().octets());
                seq.extend(peer.port().to_be_bytes());
            }

            serializer.serialize_bytes(&seq)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore = "changes over time"]
    #[tokio::test]
    async fn get_peers_sample_torrent() {
        let peers = get_peers(&PathBuf::from("sample.torrent")).await.unwrap();
        let mut res = String::with_capacity(PEER_DISPLAY_LEN * peers.0 .0.len());
        for peer in &peers.0 .0 {
            res += &peer.to_string();
            res += "\n";
        }
        assert_eq!(
            "165.232.41.73:51556\n165.232.38.164:51532\n165.232.35.114:51437\n",
            res
        );
    }
}
