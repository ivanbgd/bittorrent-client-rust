//! # Peer Communication Facilities
//!
//! These are CLI commands per project requirements.
//!
//! ## Peer Wire Protocol & Handshake
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-protocol
//!
//! https://wiki.theory.org/BitTorrentSpecification#Peer_wire_protocol_.28TCP.29
//!
//! `$ ./your_bittorrent.sh handshake sample.torrent <peer_ip>:<peer_port>`
//!
//! `Peer ID: 0102030405060708090a0b0c0d0e0f1011121314`
//!
//! Exact value will be different as it is randomly generated.
//!
//! *Note:* To get a peer IP & port to test this locally, run `./your_bittorrent.sh peers sample.torrent`
//! and pick any peer from the list.
//!
//! ## Peer Messages
//!
//! https://www.bittorrent.org/beps/bep_0003.html#peer-messages
//!
//! https://wiki.theory.org/BitTorrentSpecification#Messages
//!
//! `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
//!
//! *Optional*: To improve download speeds, you can consider pipelining your requests.
//! [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
//! recommends having 5 requests pending at once, to avoid a delay between blocks being sent.

use std::net::SocketAddrV4;
use std::path::PathBuf;
// use std::sync::OnceLock;

use crate::meta_info::meta_info;
use crate::peer::Peer;

// static INFO_HASH: OnceLock<[u8; SHA1_LEN]> = OnceLock::new();

/// Sends a handshake to a single peer, and receives a handshake from the peer, in the same format.
///
/// Arguments:
/// - torrent: &[`PathBuf`], path to a torrent file
/// - peer: &[`SocketAddrV4`], peer's socket address, <peer_ip>:<peer_port> (example: 127.0.0.1:8080)
///
/// Returns [`Peer`] which holds a 20 bytes long SHA1 representation of the peer ID received during the handshake.
///
/// [`Peer`] implements [`Display`] so it can be printed as the 40 characters long hexadecimal
/// representation of the peer ID received during the handshake.
///
/// Works with a single peer whose socket address it takes as an argument.
///
/// `$ ./your_bittorrent.sh handshake sample.torrent <peer_ip>:<peer_port>`
///
/// `Peer ID: 0102030405060708090a0b0c0d0e0f1011121314`
pub fn handshake(torrent: &PathBuf, peer: &SocketAddrV4) -> anyhow::Result<Peer> {
    let info_hash = meta_info(torrent)?.info.info_hash;

    let mut peer = Peer::new(peer)?;
    peer.handshake(info_hash)?;

    Ok(peer)
}

/// Downloads a piece of a file and stores it.
///
/// Arguments:
/// - output: &[`PathBuf`], path to the output file for storing the downloaded piece
/// - torrent: &[`PathBuf`], path to a torrent file
/// - piece_index: &[`usize`], zero-based piece index
///
/// Supports multiple peers.
///
/// `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
pub fn download_piece(
    output: &PathBuf,
    torrent: &PathBuf,
    piece_index: usize,
) -> anyhow::Result<()> {
    let info_hash = meta_info(torrent)?.info.info_hash;
    // let info_hash = INFO_HASH.get_or_init(|| meta_info(torrent)?.info.info_hash);

    // TODO: Create multiple peers and work with them at lower level.
    // I can use the above handshake function, but in that case I should either init INFO_HASH once, or
    // pass meta.info.info_hash to the handshake().
    // I could pass it to download_piece() as well, but then it becomes a responsibility of the main(),
    // in both cases. See what's better.

    Ok(())
}
