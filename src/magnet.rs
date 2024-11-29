//! # Magnet links
//!
//! [Magnet links](https://www.bittorrent.org/beps/bep_0009.html) allow users to download files
//! from peers without needing a torrent file.
//!
//! The magnet URI format is:
//! - v1: `magnet:?xt=urn:btih:<info-hash>&dn=<name>&tr=<tracker-url>&x.pe=<peer-address>`
//!
//! Unlike .torrent files, magnet links don't contain information like file length, piece length and piece hashes.
//! They only include the bare minimum information necessary to discover peers.
//! A client can then request the rest of the information from peers using the metadata exchange protocol.
//!
//! These are the query parameters in a magnet link:
//!
//! - `xt`: `urn:btih`: followed by the 40-char hex-encoded info hash
//!   (example: `urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165`).
//! - `dn`: The name of the file to be downloaded (example: `magnet1.gif`).
//! - `tr`: The tracker URL (example: `http://bittorrent-test-tracker.codecrafters.io/announce`).
//!   The value must be URL-encoded.
//!
//! ## Notes
//!
//! - We'll be using v1 of [magnet URI format](https://www.bittorrent.org/beps/bep_0009.html#magnet-uri-format).
//!   v2 is not widely used yet.
//! - `xt` (info hash) is the only required parameter, all others are optional.
//! - A magnet link can contain multiple tracker URLs, but for the purposes of this challenge it'll only contain one.
//!
//! ## Additional Info
//!
//! - [Extension for Peers to Send Metadata Files](https://www.bittorrent.org/beps/bep_0009.html)
//! - https://en.wikipedia.org/wiki/Magnet_URI_scheme
//! - [Extension Protocol](https://www.bittorrent.org/beps/bep_0010.html)
//!
//! ## Usage
//!
//! ### Parse Magnet Link
//!
//! ```shell
//! $ ./your_bittorrent.sh magnet_parse "<magnet-link>"
//! ```
//!
//! #### Example
//!
//! ```shell
//! $ ./your_bittorrent.sh magnet_parse "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce"
//! ```
//!
//! Expected response:
//! ```shell
//! Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce
//! Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165
//! ```
//!
//! ### Announce Extension Support
//!
//! ```shell
//! $ ./your_bittorrent.sh magnet_handshake "<magnet-link>"
//! ```
//!
//! We can use [these magnet links](https://github.com/codecrafters-io/bittorrent-test-seeder/blob/main/torrent_files/magnet_links.txt)
//! to test program locally. They are copied below.
//! We might need to surround links with double quotes to escape special characters in terminal.
//! - magnet1.gif.torrent: `magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce`, Peer ID: `2d524e302e302e302df91cf98bdb86ce6aca9a21`
//! - magnet2.gif.torrent: `magnet:?xt=urn:btih:3f994a835e090238873498636b98a3e78d1c34ca&dn=magnet2.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce`, Peer ID: `2d524e302e302e302d4f371471616c6c5bb2078d`
//! - magnet3.gif.torrent: `magnet:?xt=urn:btih:c5fb9894bdaba464811b088d806bdd611ba490af&dn=magnet3.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce`, Peer ID: `2d524e302e302e302de33db7666c49ec504ffdcb`
//!
//! - Peer 1: `167.71.143.54:51426`
//! - Peer 2: `165.232.35.139:51459`
//! - Peer 3: `139.59.184.255:51548`

use crate::constants::{HashType, COMPACT, DOWNLOADED, PEER_ID, PORT, SHA1_LEN, UPLOADED};
use crate::errors::MagnetError;
use crate::magnet::magnet_link::MagnetLink;
use crate::peer::Peer;
use crate::tracker::peers::Peers;
use crate::tracker::{TrackerRequest, TrackerResponse};
use anyhow::Result;
use bytes::{BufMut, Bytes};

/// Parses a given magnet link.
///
/// ```shell
/// $ ./your_bittorrent.sh magnet_parse "<magnet-link>"
/// ```
pub fn parse_magnet_link(magnet_link: &str) -> Result<MagnetLink, MagnetError> {
    let magnet_link: MagnetLink = magnet_link.try_into()?;

    Ok(magnet_link)
}

/// Handshake with a peer and announce extension support.
///
/// ```shell
/// $ ./your_bittorrent.sh magnet_handshake "<magnet-link>"
/// ```
pub async fn magnet_handshake(magnet_link: &str) -> Result<Peer, MagnetError> {
    // Parse the magnet link to get the tracker URL.
    let magnet_link = parse_magnet_link(magnet_link)?;
    let info_hash = hex::decode(magnet_link.xt.clone())?;
    let urlenc_info_hash = crate::tracker::url_encode(&info_hash);
    let mut buf: HashType = [0; SHA1_LEN];
    buf.copy_from_slice(&info_hash);

    // Perform the tracker GET request to get a list of peers.
    let peers = get_peers(magnet_link.tr.as_deref(), urlenc_info_hash).await?;

    // Choose a peer.
    let mut peer = Peer::new(&peers.0[0]);
    // Establish a TCP connection with a peer, and perform a handshake.
    peer.handshake(&buf).await?;

    Ok(peer)
}

/// Fetches and returns the peers list.
async fn get_peers(tracker: Option<&str>, info_hash: String) -> Result<Peers, MagnetError> {
    let tracker = match tracker {
        Some(url) => url,
        None => return Err(MagnetError::TrackerMissing),
    };

    // Currently, only the single-file torrents are supported.
    // Tracker requires a "left" parameter value greater than zero, but we don't know file size in advance.
    // We can send a made up value like 999 as a workaround.
    let left = 999;

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
    let resp = add_interval(resp);
    let response: TrackerResponse = serde_bencode::from_bytes(&resp)?;

    Ok(response.peers)
}

/// A hack that was required because the test server (tracker) didn't return the field `interval` as part of
/// the response, if ran on server. Namely, the tracker was returning the field if I ran it locally and
/// connected to their server (tracker).
fn add_interval(resp: Bytes) -> Vec<u8> {
    let resp_clone = resp.clone();
    let resp_str = String::from_utf8_lossy(&resp_clone);
    let mut resp_vec = Vec::from(resp);
    if !resp_str.contains("8:interval") {
        // Remove the last 'e' that was ending the entire dictionary.
        let _removed = resp_vec.remove(resp_vec.len() - 1);
        // Also add back the last 'e' to end the entire dictionary.
        resp_vec.put_slice(b"8:intervali60ee");
    }
    resp_vec
}

mod magnet_link {
    //! Magnet link
    //!
    //! - https://www.bittorrent.org/beps/bep_0009.html
    //! - https://en.wikipedia.org/wiki/Magnet_URI_scheme

    use crate::errors::MagnetLinkError;
    use anyhow::Result;
    use std::fmt::{Display, Formatter};

    /// Magnet link
    ///
    /// Supports `xt`, `dn` and `tr` parameters. Only `xt` is mandatory.
    #[derive(Debug)]
    pub struct MagnetLink {
        /// Exact Topic, `xt`, specifies the URN containing file hash. Mandatory.
        pub xt: String,

        /// Display Name, `dn`, may be used by the client to display while waiting for metadata. Optional.
        pub dn: Option<String>,

        /// Tracker URL. Optional.
        /// Used to obtain resources for BitTorrent downloads without a need for DHT support.
        /// The value must be URL encoded.
        pub tr: Option<String>,
    }

    impl Display for MagnetLink {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            if let Some(tr) = &self.tr {
                writeln!(f, "Tracker URL: {}", *tr)?;
            };

            writeln!(f, "Info Hash: {}", self.xt)?;

            if let Some(dn) = &self.dn {
                writeln!(f, "Display Name: {}", *dn)
            } else {
                write!(f, "")
            }
        }
    }

    impl TryFrom<&str> for MagnetLink {
        type Error = MagnetLinkError;

        fn try_from(value: &str) -> Result<Self, Self::Error> {
            if !value.starts_with("magnet:?") {
                return Err(MagnetLinkError::NoMagnet(value.to_string()));
            }
            let (_magnet, rest) = value.split_at("magnet:?".len());
            if !rest.contains("xt=urn:bt") {
                return Err(MagnetLinkError::NoXt(value.to_string()));
            }

            let mut xt = "".to_string();
            let mut dn = None;
            let mut tr = None;

            // Magnet URIs consist of a series of one or more parameters, the order of which is not significant,
            // formatted in the same way as query strings that ordinarily terminate HTTP URLs.
            //
            // Support only v1 of the magnet URI format from this point on.
            let params = rest.split('&');
            for elt in params {
                if let Some(param) = elt.split_once('=') {
                    match param.0 {
                        "xt" => xt = param.1.split_at("urn:btih:".len()).1.to_string(),
                        "dn" => dn = Some(param.1.to_string()),
                        "tr" => tr = Some(url_decode(param.1)?),
                        _ => {}
                    }
                }
            }

            Ok(Self { xt, dn, tr })
        }
    }

    fn url_decode(url: &str) -> Result<String, MagnetLinkError> {
        Ok(urlencoding::decode(url)?.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::MagnetLinkError;

    #[test]
    fn parse_magnet_link_xt_dn_tr() {
        let example = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n\
            Display Name: magnet1.gif\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_xt_tr_dn() {
        let example = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce&dn=magnet1.gif";
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n\
            Display Name: magnet1.gif\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_dn_xt_tr() {
        let example = "magnet:?dn=magnet1.gif&xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n\
            Display Name: magnet1.gif\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_dn_xt() {
        let example = "magnet:?dn=magnet1.gif&xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165";
        assert_eq!(
            "Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n\
            Display Name: magnet1.gif\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_xt() {
        let example = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165";
        assert_eq!(
            "Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_no_params() {
        let example = "magnet:?";
        assert_eq!(
            MagnetError::from(MagnetLinkError::NoXt(example.to_string())),
            parse_magnet_link(example).unwrap_err()
        );
    }

    #[test]
    fn parse_magnet_link_no_xt() {
        let example = "magnet:?tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        assert_eq!(
            MagnetError::from(MagnetLinkError::NoXt(example.to_string())),
            parse_magnet_link(example).unwrap_err()
        );
    }

    #[test]
    fn parse_magnet_link_no_magnet() {
        let example = "xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165";
        assert_eq!(
            MagnetError::from(MagnetLinkError::NoMagnet(example.to_string())),
            parse_magnet_link(example).unwrap_err()
        );
    }
}
