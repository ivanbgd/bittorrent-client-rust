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
//! ## Usage Examples
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
//! Example response:
//! ```shell
//! Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce
//! Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165
//! ```
//!
//! ### Announce Extension Support, Send & Receive Extension Handshake
//!
//! ```shell
//! $ ./your_bittorrent.sh magnet_handshake "<magnet-link>"
//! ```
//!
//! Example response:
//! ```shell
//! Peer ID: 0102030405060708090a0b0c0d0e0f1011121314
//! Peer Metadata Extension ID: 123
//! ```
//!
//! ### Request & Receive Metadata
//!
//! ```shell
//! $ ./your_bittorrent.sh magnet_info "<magnet-link>"
//! ```
//!
//! Example response:
//! ```shell
//! Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce
//! Length: 92063
//! Info Hash: d69f91e6b2ae4c542468d1073a71d4ea13879a7f
//! Piece Length: 32768
//! Piece Hashes:
//! 6e2275e604a0766656736e81ff10b55204ad8d35
//! e876f67a2a8886e8f36b136726c30fa29703022d
//! f00d937a0213df1982bc8d097227ad9e909acc17
//! ```
//!
//! ### Download a Piece
//!
//! ```shell
//! $ ./your_bittorrent.sh magnet_download_piece -o <path_to_output_file> "<magnet-link>" <piece_index>
//! ```
//!
//! ### Magnet Links for Local Testing
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

use crate::config::Config;
use crate::constants::{
    HashType, BLOCK_SIZE, CLIENT_NAME, COMPACT, DOWNLOADED, PEER_ID, PORT, SHA1_LEN, UPLOADED,
    UT_METADATA, UT_METADATA_ID,
};
use crate::errors::{MagnetError, PeerError};
use crate::magnet::magnet_link::MagnetLink;
use crate::message::{
    ExtendedMessageHandshakeDict, ExtendedMessageHandshakePayload, ExtendedMessageId,
    ExtensionMessage, ExtensionMessageId, ExtensionPayload, Message, MessageId,
};
use crate::meta_info::Info;
use crate::peer::Peer;
use crate::peer_comm::{
    check_file_size, get_work_params, recv_piece, send_reqs, PieceParams, WorkParams,
};
use crate::tracker::peers::Peers;
use crate::tracker::{TrackerRequest, TrackerResponse};
use crate::MetadataSource;

use anyhow::{Context, Result};
use log::{debug, info, trace, warn};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;

/// Parses the provided magnet link.
///
/// ```shell
/// $ ./your_bittorrent.sh magnet_parse "<magnet-link>"
/// ```
pub fn parse_magnet_link(magnet_link: &str) -> Result<MagnetLink, MagnetError> {
    let magnet_link: MagnetLink = magnet_link.try_into()?;

    Ok(magnet_link)
}

/// Handshake with a (randomly-chosen) peer and announce extension support.
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
    let peers = magnet_get_peers(magnet_link.tr.as_deref(), urlenc_info_hash).await?;

    // Choose a peer randomly.
    let mut list = peers.0;
    list.shuffle(&mut thread_rng());
    let peer_idx = 0;
    let mut peer = Peer::new(&list[peer_idx]);

    // <=> Establish a TCP connection with a peer, and perform a base handshake
    let supports_ext = match peer.base_handshake(&buf).await {
        Ok(v) => v,
        Err(err) => {
            warn!("Magnet handshake error: {err:#}");
            return Err(MagnetError::from(err));
        }
    };
    trace!("00 mhs Handshake with peer_idx {peer_idx}: {}", peer.addr);

    // -> Send the bitfield message (safe to ignore in this challenge, so we are skipping this)

    // <= Receive a Bitfield message
    let msg = match peer.recv_msg().await {
        Ok(msg) => msg,
        Err(err) => {
            warn!("Receive a Bitfield message: {err:#}");
            return Err(err.into());
        }
    };
    trace!("01 mhs peer_idx {peer_idx}: {msg}");
    if MessageId::Bitfield != msg.id {
        let err = PeerError::from((MessageId::Bitfield, msg.id));
        warn!("Receive a Bitfield message: {err:#}");
        return Err(err.into());
    }
    peer.bitfield = Some(
        msg.payload
            .clone()
            .expect("Expected to have received a Bitfield message"),
    );

    // If the peer supports extensions (based on the reserved bit in the base handshake):
    if supports_ext {
        // -> Send the extension handshake message
        let ext_hs_dict = ExtendedMessageHandshakeDict::new(
            Some(HashMap::from([(UT_METADATA.to_string(), UT_METADATA_ID)])),
            None,
            Some(PORT),
            Some(CLIENT_NAME.to_string()),
        );
        let payload =
            ExtendedMessageHandshakePayload::new(ExtendedMessageId::Handshake, ext_hs_dict)?;
        let msg = Message::new(MessageId::Extended, Some(payload.into()));
        debug!("-> msg = {msg}");
        peer.feed(msg)
            .await
            .context("Feed the extension handshake message")?;
        peer.flush()
            .await
            .context("Flush the extension handshake message")?;

        // <= Receive the extension handshake message
        let msg = match peer.recv_msg().await {
            Ok(msg) => msg,
            Err(err) => {
                warn!("Receive the extension handshake message: {err:#}");
                return Err(err.into());
            }
        };
        debug!("<= msg = {msg}");
        if MessageId::Extended != msg.id {
            let err = PeerError::WrongMessageId(MessageId::Extended, msg.id);
            warn!("Receive the extension handshake message: {err:#}");
            return Err(err.into());
        }
        let payload: ExtendedMessageHandshakePayload = msg
            .payload
            .expect("Expected to have received the extension handshake message")
            .try_into()?;
        if ExtendedMessageId::Handshake != payload.id {
            let err = PeerError::WrongExtendedMessageId(ExtendedMessageId::Handshake, payload.id);
            warn!("Receive the extension handshake message: {err:#}");
            return Err(err.into());
        }
        let dict: ExtendedMessageHandshakeDict = payload.dict.try_into()?;
        trace!("<= mhs payload.dict = {}", dict);

        peer.set_extension_dict(dict);
        trace!("peer.extension_dict = {:?}", peer.get_extension_dict());
    }
    // Note that the extension handshake message is only sent if the other peer supports extensions
    // (indicated by the reserved bit in the base handshake).
    // This is how backward compatibility is maintained with peers that don't support extensions.

    // -> Send the Interested message
    let msg = Message::new(MessageId::Interested, None);
    peer.feed(msg)
        .await
        .context("Feed the Interested message")?;
    peer.flush().await.context("Flush the Interested message")?;

    // <= Receive an Unchoke message
    let msg = match peer.recv_msg().await {
        Ok(msg) => msg,
        Err(err) => {
            warn!("Receive an Unchoke message: {err:#}");
            return Err(err.into());
        }
    };
    trace!("02 mhs peer_idx {peer_idx}: {msg}");
    if MessageId::Unchoke != msg.id {
        let err = PeerError::from((MessageId::Unchoke, msg.id));
        warn!("Receive an Unchoke message: {err:#}");
        return Err(err.into());
    }

    debug!("Working with single peer: {}\n", peer.addr);

    Ok(peer)
}

/// Requests torrent metadata from a peer using the metadata extension and
/// returns it, together with the peer.
///
/// This assumes that it performs a magnet handshake with a (randomly-chosen) peer.
///
/// ```shell
/// $ ./your_bittorrent.sh magnet_info "<magnet-link>"
/// ```
pub async fn request_magnet_info(magnet_link: &str) -> Result<(Info, Peer), MagnetError> {
    // <=> Perform the extension handshake and get the peer
    let mut peer = magnet_handshake(magnet_link).await?;

    let extension_id = peer.get_extension_id()?;
    let metadata_size = peer.get_metadata_size()?;
    let num_pcs = metadata_size.div_ceil(BLOCK_SIZE);

    let mut metadata: Vec<u8> = Vec::with_capacity(metadata_size);

    // This extension only transfers the **info-dictionary** part of the .torrent file.
    //
    // The metadata is handled in blocks of 16 KiB (16384 Bytes). The metadata blocks are indexed starting at 0.
    // All blocks are 16 KiB except the last block which may be smaller.
    for piece_index in 0..num_pcs as u32 {
        // -> Send the metadata request message
        let payload =
            ExtensionPayload::new_request(ExtendedMessageId::Custom(extension_id), piece_index)?;
        let msg = Message::new(MessageId::Extended, Some(payload.try_into()?));
        debug!("-> msg = {msg}");
        peer.feed(msg)
            .await
            .context("Feed the metadata request message")?;
        peer.flush()
            .await
            .context("Flush the metadata request message")?;

        // <= Receive the metadata data message
        let msg = match peer.recv_msg().await {
            Ok(msg) => msg,
            Err(err) => {
                warn!("Receive the metadata data or reject message: {err:#}");
                return Err(err.into());
            }
        };
        debug!("<= msg = {msg}");
        if MessageId::Extended != msg.id {
            let err = PeerError::WrongMessageId(MessageId::Extended, msg.id);
            warn!("Receive the metadata data or reject message: {err:#}");
            return Err(err.into());
        }

        let payload = msg
            .payload
            .expect("Expected to have received the metadata data or reject message");
        let (dict, info) = if piece_index == 0 {
            payload.split_at(payload.len() - metadata_size)
        } else {
            payload.split_at(0)
        };

        // The other peers should send our metadata ID in their responses.
        if UT_METADATA_ID != dict[0] {
            let err =
                PeerError::WrongExtendedMessageId(UT_METADATA_ID.try_into()?, dict[0].try_into()?);
            warn!("Receive the metadata data or reject message: {err:#}");
            return Err(err.into());
        }

        // The first byte is reserved for ExtendedMessageId, so skip it.
        let dict = &dict[1..];
        let dict: ExtensionMessage = dict.try_into()?;
        if dict.msg_type == ExtensionMessageId::Reject {
            let err = MagnetError::Reject(peer.addr);
            warn!("{err:#}");
            return Err(err);
        }

        metadata.extend(info);
    }

    let mut info: Info = serde_bencode::from_bytes(&metadata)?;
    info.info_hash = *Sha1::digest(&metadata).as_ref();
    info.info_hash_hex = hex::encode(info.info_hash);

    // Validate hash
    let hash_from_magnet_link = parse_magnet_link(magnet_link)?.xt;
    let calculated_info_hash = &info.info_hash_hex;
    if hash_from_magnet_link != *calculated_info_hash {
        let err = MagnetError::HashMismatch(hash_from_magnet_link, calculated_info_hash.clone());
        warn!("{err:#}");
        return Err(err);
    }

    Ok((info, peer))
}

/// Downloads a single piece of a file and stores it.
///
/// Arguments:
/// - config: [`Config`], application configuration,
/// - output: &[`PathBuf`], path to the output file for storing the downloaded piece
/// - magnet_link: &[`str`], magnet link
/// - piece_index: [`usize`], zero-based piece index
///
/// The last piece can be smaller than other pieces which are of same fixed size that
/// is defined in the torrent file.
///
/// Pieces are split into blocks of 16 kB or potentially less in case of the very last block,
/// and transferred as such. The blocks are assembled into a full piece when all of them
/// have been received.
///
/// Works with a single (randomly-chosen) peer, but pipelines requests to it for increased download speed.
///
/// `$ ./your_bittorrent.sh magnet_download_piece -o /tmp/test-piece sample.torrent "<magnet-link>" <piece_index>`
pub async fn magnet_download_piece(
    config: Config,
    output: &PathBuf,
    magnet_link: &str,
    piece_index: usize,
) -> Result<(), MagnetError> {
    let work_params =
        get_work_params(MetadataSource::MagnetLink(magnet_link), Some(piece_index)).await?;

    let WorkParams {
        info,
        piece_len,
        last_piece_len,
        block_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
        ..
    } = work_params;

    let num_pcs = info.pieces.0.len();

    if piece_index >= num_pcs {
        return Err(MagnetError::from(PeerError::WrongPieceIndex(
            piece_index,
            num_pcs,
        )));
    }

    let peer_idx = 0;

    // This is silly, but let's just do it for the sake of the challenge and its tests passing.
    // The challenge had us develop functions in some order, that I'm now reusing, but I'd design
    // this differently from the beginning if this entire repository weren't originally meant
    // to be a solution for the challenge.
    // So, let's handshake with some peer again!
    // Namely, we choose a peer randomly, so it might not be the same peer as the one we used to fetch metadata,
    // but that's not a problem, because all peers have the same metadata, because it's the same torrent.

    // <=> Perform the extension handshake and get the peer
    let peer = &mut magnet_handshake(magnet_link).await?;

    let piece_hash = &info.pieces.0[piece_index];

    let is_last_piece = piece_index == num_pcs - 1;
    let mut current_piece_len = piece_len;
    if is_last_piece {
        current_piece_len = last_piece_len;
    }

    let piece_params = PieceParams {
        num_pcs,
        piece_len,
        last_piece_len,
        block_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
        piece_index,
        piece_hash,
        piece_offset: 0,
        peer_idx,
    };

    let mut file = File::create(output).await?;

    send_reqs(&config, &piece_params, peer).await?;
    let written_total = recv_piece(&config, &piece_params, peer, &mut file).await?;

    check_file_size(current_piece_len, written_total, output).await?;

    info!("Success!");

    Ok(())
}

/// Fetches and returns the peers list.
async fn magnet_get_peers(tracker: Option<&str>, info_hash: String) -> Result<Peers, MagnetError> {
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
    let resp = Vec::from(resp);
    let response: TrackerResponse = serde_bencode::from_bytes(&resp)?;

    if response.peers.0.is_empty() {
        return Err(MagnetError::NoPeersFound);
    }

    Ok(response.peers)
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
