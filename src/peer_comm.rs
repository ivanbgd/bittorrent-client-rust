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

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::net::SocketAddrV4;
use std::path::PathBuf;
// use std::sync::OnceLock;

use anyhow::Result;

use crate::constants::SHA1_LEN;
use crate::errors::PeerError;
use crate::message::{Message, MessageId};
use crate::peer::Peer;
use crate::tracker::get_peers;

// static INFO_HASH: OnceLock<Result<[u8; SHA1_LEN]>> = OnceLock::new();

/// Sends a handshake to a single peer, and receives a handshake from the peer, in the same format.
///
/// Arguments:
/// - peer: &[`SocketAddrV4`], peer's socket address, <peer_ip>:<peer_port> (example: 127.0.0.1:8080)
/// - info_hash: &[u8; SHA1_LEN], can be obtained and calculated from a torrent file
///
/// Returns [`Peer`] which holds a 20 bytes long SHA1 representation of the peer ID received during the handshake.
///
/// [`Peer`] implements [`Display`] so it can be printed as the 40 characters long hexadecimal
/// representation of the peer ID received during the handshake.
///
/// Works with a single peer whose socket address it takes as an argument.
///
/// The handshake is a required message and must be the first message transmitted by the client.
///
/// `$ ./your_bittorrent.sh handshake sample.torrent <peer_ip>:<peer_port>`
///
/// `Peer ID: 0102030405060708090a0b0c0d0e0f1011121314`
///
/// Exact value will be different as it is randomly generated.
pub fn handshake(peer: &SocketAddrV4, info_hash: &[u8; SHA1_LEN]) -> Result<Peer, PeerError> {
    // let info_hash = INFO_HASH
    //     .get_or_init(|| Ok(meta_info(torrent)?.info.info_hash))
    //     .as_ref()
    //     .expect("info hash should be available at this point");

    let mut peer = Peer::new(peer);
    peer.handshake(info_hash)?;
    eprintln!("{peer}"); //todo

    Ok(peer)
}

/// Downloads a piece of a file and stores it.
///
/// Arguments:
/// - output: &[`PathBuf`], path to the output file for storing the downloaded piece
/// - torrent: &[`PathBuf`], path to a torrent file
/// - piece_index: &[`usize`], zero-based piece index
///
/// Supports multiple peers. This improves download speeds because it pipelines requests
/// and avoids delays between blocks being sent to us from the peers. Source (PDF):
/// [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
///
/// `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
pub fn download_piece(
    output: &PathBuf,
    torrent: &PathBuf,
    piece_index: usize,
) -> Result<(), PeerError> {
    let output = File::create(output)?;
    let mut file_writer = BufWriter::new(output);

    // Perform the tracker GET request to get a list of peers
    let (peers, info_hash) = get_peers(torrent)?;
    let peers = peers.0;

    // TODO: Create multiple peers and work with them at lower level.
    // TODO: Perhaps choose (at most) PIPELINED_REQUESTS peers and connect to all of them.
    let peer = &peers[0];
    // Establish a TCP connection with a peer, and perform a handshake
    let peer = handshake(peer, &info_hash)?;

    // Exchange messages with the peer
    let mut stream = peer
        .stream
        .unwrap_or_else(|| panic!("Expected to get a stream from the peer {}", peer.addr));

    // let mut recv_buf: Vec<u8> = Vec::with_capacity(1 << 15);
    // stream.read_exact(&mut recv_buf)?;
    // eprintln!("{:?}", &recv_buf[..]);

    // Receive a Bitfield message
    let mut msg_len = [0u8; 4];
    stream.read_exact(&mut msg_len)?;
    let msg_len = u32::from_be_bytes(msg_len) as usize;
    let mut buf = vec![0u8; msg_len];
    stream.read_exact(&mut buf)?;
    eprintln!("{}, {:?}", msg_len, buf); // todo remove

    // Send the Interested message
    let msg = Message::new(MessageId::Interested, &[0u8; 0]);
    stream.write_all(msg.into())?;

    // Receive an Unchoke message
    let mut buf = vec![0u8; 5];
    // stream.read_exact(&mut buf)?;
    eprintln!("{:?}", buf); // todo remove

    let peer = &peers[1];
    let peer = handshake(peer, &info_hash)?;
    let peer = &peers[2];
    let peer = handshake(peer, &info_hash)?;

    Ok(())
}
