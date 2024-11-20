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

use std::cmp::min;
use std::net::SocketAddrV4;
use std::path::PathBuf;

use anyhow::{Context, Result};
use log::{debug, info};
use sha1::{Digest, Sha1};
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::constants::{BLOCK_SIZE, MAX_PIPELINED_REQUESTS, SHA1_LEN};
use crate::errors::PeerError;
use crate::message::{Message, MessageId, RequestPayload};
use crate::meta_info::Info;
use crate::peer::Peer;
use crate::tracker::get_peers;

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
pub async fn handshake(peer: &SocketAddrV4, info_hash: &[u8; SHA1_LEN]) -> Result<Peer, PeerError> {
    let mut peer = Peer::new(peer);
    peer.handshake(info_hash).await?;

    Ok(peer)
}

#[derive(Debug, Clone)]
struct WorkParams {
    peers: Vec<SocketAddrV4>,
    info: Info,
    file_len: usize,
    num_pcs: usize,
    is_last_piece: Option<bool>,
    block_len: usize,
    num_blocks_per_piece: usize,
    num_blocks_in_last_piece: usize,
    last_block_len: usize,
    total_num_blocks: usize,
}

/// Downloads a single piece of a file and stores it.
///
/// Arguments:
/// - output: &[`PathBuf`], path to the output file for storing the downloaded piece
/// - torrent: &[`PathBuf`], path to a torrent file
/// - piece_index: [`usize`], zero-based piece index
///
/// The last piece can be smaller than other pieces which are of same fixed size that
/// is defined in the torrent file.
///
/// Pieces are split into blocks of 16 kB or potentially less in case of the very last block,
/// and transferred as such. The blocks are assembled into a full piece when all of them
/// have been received.
///
/// Supports multiple peers. This improves download speeds because it pipelines requests
/// and avoids delays between blocks being sent to us from the peers.
/// Source (PDF): [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
/// Also see: https://wiki.theory.org/BitTorrentSpecification#Queuing
///
/// `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
pub async fn download_piece(
    output: &PathBuf,
    torrent: &PathBuf,
    piece_index: usize,
) -> Result<(), PeerError> {
    let file = File::create(output).await?;
    let mut file_writer = BufWriter::new(file);

    let work_params = get_work_params(torrent, Some(piece_index))?;

    let WorkParams {
        info,
        num_pcs,
        is_last_piece,
        block_len,
        num_blocks_per_piece,
        last_block_len,
        ..
    } = work_params.clone();

    if piece_index >= num_pcs {
        return Err(PeerError::WrongPieceIndex(piece_index, num_pcs));
    }

    // Support working with multiple peers at the same time
    let (mut work_peers, block_iters) = local_get_peers(work_params).await?;
    let num_peers = work_peers.len();

    let piece_hash = &info.pieces.0[piece_index];

    // Block index - for logging purposes only
    let mut block = 0usize;

    let block_params = BlockParams {
        block_iters,
        num_peers,
        is_last_piece: is_last_piece.unwrap(),
        block_len,
        num_blocks_per_piece,
        last_block_len,
        total_num_blocks: num_blocks_per_piece,
        piece_index,
        piece_hash,
    };

    let _piece = get_piece(&block_params, &mut work_peers, &mut block, &mut file_writer).await?;

    file_writer.flush().await?;

    info!("Success!");

    Ok(())
}

/// Downloads a whole file and stores it.
///
/// Arguments:
/// - output: &[`PathBuf`], path to the output file for storing the whole downloaded file
/// - torrent: &[`PathBuf`], path to a torrent file
///
/// `$ ./your_bittorrent.sh download -o /tmp/test.txt sample.torrent`
pub async fn download(output: &PathBuf, torrent: &PathBuf) -> Result<(), PeerError> {
    let file = File::create(output).await?;
    let mut file_writer = BufWriter::new(file);

    let work_params = get_work_params(torrent, None)?;

    let WorkParams {
        peers,
        info,
        file_len,
        num_pcs,
        block_len,
        mut num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
        ..
    } = work_params.clone();

    // Support working with multiple peers at the same time
    let (mut work_peers, mut block_iters) = local_get_peers(work_params).await?;
    let mut num_peers = work_peers.len();

    // All piece hashes from the torrent file
    let pieces = &info.pieces.0;

    // // Entire contents of the file
    // let mut contents = vec![];todo

    // Block index - for logging purposes only
    let mut block = 0usize;

    // Download all pieces
    for (piece_index, piece_hash) in pieces.iter().enumerate() {
        let is_last_piece = piece_index == num_pcs - 1;
        if is_last_piece {
            num_blocks_per_piece = num_blocks_in_last_piece;
            block_iters = num_blocks_per_piece;
            num_peers = num_blocks_in_last_piece;
            num_peers = min(peers.len(), num_peers);
        }

        let block_params = BlockParams {
            block_iters,
            num_peers,
            is_last_piece,
            block_len,
            num_blocks_per_piece,
            last_block_len,
            total_num_blocks,
            piece_index,
            piece_hash,
        };

        let _piece =
            get_piece(&block_params, &mut work_peers, &mut block, &mut file_writer).await?;

        info!(
            "piece {:2}/{num_pcs} downloaded and stored",
            piece_index + 1
        );
        eprintln!("piece {:2}/{num_pcs} downloaded", piece_index + 1); //todo rem
    }

    file_writer.flush().await?;

    let file = File::open(output).await?;
    let file_size = file.metadata().await?.len() as usize;
    info!(
        "Wrote {file_size} out of expected {file_len} bytes to \"{}\".",
        output.display()
    );
    if file_len != file_size {
        return Err(PeerError::WrongLen(file_len, file_size));
    }

    info!("Success!");

    Ok(())
}

/// Calculates and returns basic work parameters
fn get_work_params(torrent: &PathBuf, piece_index: Option<usize>) -> Result<WorkParams, PeerError> {
    // Perform the tracker GET request to get a list of peers
    let (peers, info) = get_peers(torrent)?;
    let peers = peers.0;

    // The file to download is split into pieces of same fixed length,
    // which is defined in torrent file and is a power of two,
    // except potentially for the last piece which can be smaller.
    // File ultimately needs to be assembled from received pieces, but this function is not meant for that.
    // The file size is also provided in the torrent file.
    let file_len = info.length();
    let piece_len = info.plen;
    let mut last_piece_len = file_len % piece_len;
    let num_pcs = file_len / piece_len + last_piece_len.clamp(0, 1);
    if last_piece_len == 0 {
        last_piece_len = piece_len;
    }

    let is_last_piece = piece_index.map(|piece_index| piece_index == num_pcs - 1);

    debug!("piece_index = {:?}", piece_index);
    debug!("file_len = {}", file_len);
    debug!("piece_len = {}", piece_len);
    debug!("last_piece_len = {}", last_piece_len);
    debug!("num_pcs = {}", num_pcs);
    debug!("is_last_piece = {:?}", is_last_piece);

    // Pieces are split into blocks and transferred as such.
    // Pieces ultimately need to be assembled from received blocks.
    // Block size is 16 kB (`BLOCK_SIZE`), except potentially for the last block which can be smaller.
    let block_len = BLOCK_SIZE;
    let mut num_blocks_per_piece = piece_len / block_len;
    let mut last_block_len = last_piece_len % block_len;
    let num_blocks_in_last_piece = last_piece_len / block_len + last_block_len.clamp(0, 1);
    if last_block_len == 0 {
        last_block_len = block_len;
    }
    let total_num_blocks = (num_pcs - 1) * num_blocks_per_piece + num_blocks_in_last_piece;
    if is_last_piece.is_some() && is_last_piece.unwrap() {
        num_blocks_per_piece = num_blocks_in_last_piece;
    }

    debug!("block_len = {}", block_len);
    debug!("num_blocks_per_piece = {}", num_blocks_per_piece);
    debug!("num_blocks_in_last_piece = {}", num_blocks_in_last_piece);
    debug!("last_block_len = {}", last_block_len);
    debug!("total_num_blocks = {}", total_num_blocks);

    Ok(WorkParams {
        peers,
        info,
        file_len,
        num_pcs,
        is_last_piece,
        block_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
    })
}

/// Support working with multiple peers at the same time
///
/// # Returns
/// - List of peers to work with, `work_peers: Vec<Peer>`,
/// - Number of the outer loop iterations, which is per blocks, `block_iters: usize`
async fn local_get_peers(work_params: WorkParams) -> Result<(Vec<Peer>, usize), PeerError> {
    let WorkParams {
        mut peers,
        info,
        is_last_piece,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        ..
    } = work_params;

    let mut num_peers = min(MAX_PIPELINED_REQUESTS, num_blocks_per_piece);
    if is_last_piece.is_some() && is_last_piece.unwrap() {
        num_peers = num_blocks_in_last_piece;
    }
    num_peers = min(peers.len(), num_peers);
    debug!("num_peers = {num_peers}; peers.len() = {}", peers.len());

    let mut work_peers: Vec<Peer> = Vec::with_capacity(num_peers);

    // Get all peers to work with - handshake with them and store their streams.
    // The selection of peers could be randomized, but it isn't necessary; rather, this is just an idea.
    for (peer_idx, peer) in peers.iter_mut().enumerate().take(num_peers) {
        // Establish a TCP connection with a peer, and perform a handshake
        let mut peer = handshake(peer, &info.info_hash).await?;

        debug!("00 Handshake with peer_idx {peer_idx}: {}", peer.addr);

        // Exchange messages with the peer: receive Bitfield, send Interested, receive Unchoke

        // Receive a Bitfield message (todo: it is optional in general case, so we can improve this)
        let msg = peer
            .recv_msg()
            .await
            .context("Receive a Bitfield message")?;
        debug!("01 peer_idx {peer_idx}: {msg}");
        if msg.id != MessageId::Bitfield {
            return Err(PeerError::from((msg.id, MessageId::Bitfield)));
        }
        peer.bitfield = Some(
            msg.payload
                .clone()
                .expect("Expected to have received a Bitfield message"),
        );

        // Send the Interested message
        let msg = Message::new(MessageId::Interested, None);
        peer.send_msg(msg)
            .await
            .context("Send the Interested message")?;

        // Receive an Unchoke message
        let msg = peer
            .recv_msg()
            .await
            .context("Receive an Unchoke message")?;
        debug!("02 peer_idx {peer_idx}: {msg}");
        if msg.id != MessageId::Unchoke {
            return Err(PeerError::from((msg.id, MessageId::Unchoke)));
        }

        work_peers.push(peer);
    }

    // Number of the outer loop iterations, which is per blocks
    let block_iters =
        num_blocks_per_piece / work_peers.len() + num_blocks_per_piece % work_peers.len();

    debug!("work_peers.len() = {}", work_peers.len());
    debug!("block_iters = {block_iters}");

    Ok((work_peers, block_iters))
}

#[derive(Debug)]
struct BlockParams<'a> {
    // work_peers: &'a Vec<Peer>, // todo rem
    block_iters: usize,
    num_peers: usize,
    is_last_piece: bool,
    block_len: usize,
    num_blocks_per_piece: usize,
    last_block_len: usize,
    total_num_blocks: usize,
    // block: &'a mut usize, // todo rem
    piece_index: usize,
    piece_hash: &'a [u8; SHA1_LEN],
    // file_writer: BufWriter<File>, // todo rem
}

// TODO: Work on a block basis
// ///Gets a single block (sub-piece) from a peer and returns it.
// async fn get_block(...) -> Result<Block, PeerError> {...}

// /// Gets a single piece from multiple peers and returns it. // todo rem
/// Gets a single piece from a single peer and returns it.
// todo: add multiple peers
// todo: don't write to file in the function
async fn get_piece(
    block_params: &BlockParams<'_>,
    work_peers: &mut [Peer],
    block: &mut usize,
    file_writer: &mut BufWriter<File>,
) -> Result<Piece, PeerError> {
    let BlockParams {
        block_iters,
        num_peers,
        is_last_piece,
        block_len,
        num_blocks_per_piece,
        last_block_len,
        total_num_blocks,
        piece_index,
        piece_hash,
    } = block_params;

    // For validating the hash of the received piece
    let mut hasher = Sha1::new();

    // The combined loop counter - from both loops; represents the block ordinal number
    let mut i = 0usize;

    let data = vec![];

    // Fetch blocks from a single peer - todo: change comment to multiple peers

    ///////////////////////////////////////////////////////////////////////////////
    // TODO: Use block indices and assemble them in order.
    // What we currently have works, because we work with only one peer, but messages can in general
    // be received out of order over the network, in either way.

    // Check all peers for the given piece and try to find one that has it: the single-peer variant
    // todo for multi-peer (we still work with only one peer)
    let mut pi = 0;
    let (peer_idx, peer, found) = loop {
        let p = &mut work_peers[pi];
        if peer_has_piece(p, *piece_index) {
            break (pi, p, true);
        }
        info!("{}", PeerError::MissingPiece(p.addr, *piece_index));
        if pi == work_peers.len() - 1 {
            break (0, &mut work_peers[0], false);
        }
        pi += 1;
    };
    if !found {
        return Err(PeerError::NoPeerHasPiece(*piece_index));
    }

    // todo rem
    // if !does_peer_have_piece(peer, *piece_index) {
    //     // TODO: Don't return, but mark the piece for retry, and do retry somehow - with another peer.
    //     return Err(PeerError::MissingPiece(peer.addr, *piece_index));
    // }

    // let num_reqs = min(MAX_PIPELINED_REQUESTS, *num_blocks_per_piece);
    // let num_reqs = MAX_PIPELINED_REQUESTS;
    let num_reqs = *num_blocks_per_piece;

    // Pipeline requests to a single peer.
    // Outer loop is by blocks, while the inner loop is by requests to the single peer.
    'outer: for block_idx in 0..*block_iters {
        let mut j = 0usize;

        // Send several requests in a row to the peer, without waiting for responses at this moment.
        // We'll wait for the responses later, in the following loop.
        for _ in 0..num_reqs {
            *block += 1;

            // Send a Request message for each block - we don't request pieces but blocks.
            let index = *piece_index as u32;
            let begin = u32::try_from(i * *block_len)?;
            let mut length = *block_len as u32;
            if *is_last_piece && i == *num_blocks_per_piece - 1 {
                length = *last_block_len as u32;
            }
            let msg = Message::new(
                MessageId::Request,
                Some(RequestPayload::new(index, begin, length).into()),
            );
            debug!(
                "block {block:3}/{total_num_blocks}, i = {i:3}: block_idx = {block_idx}, \
                 peer_idx = {peer_idx}; pc idx = {index}, begin = {begin}, length = {length}"
            );
            eprintln!("block {block:3}/{total_num_blocks}, i = {i:3}: piece index = {index}, begin = {begin}, length = {length}"); //todo rem
            peer.send_msg(msg).await.context("Request")?;

            if i == *num_blocks_per_piece - 1 {
                break;
            }
            i += 1;
            j += 1;
        }

        i -= j;

        // Receive a Piece message for each block we've requested in a row.
        for _ in 0..num_reqs {
            let msg = peer.recv_msg().await.context("Piece")?;
            if msg.id != MessageId::Piece {
                return Err(PeerError::from((msg.id, MessageId::Piece)));
            }

            let payload = &msg.payload.expect("Expected to have received some payload")[8..];
            hasher.update(payload); //todo
            file_writer.write_all(payload).await?; //todo

            if i == *num_blocks_per_piece - 1 {
                break 'outer;
            }
            i += 1;
        }
    }
    ///////////////////////////////////////////////////////////////////////////////

    // TODO: pipeline by blocks but with multiple peers this time (or by pieces?)
    // todo: three nested loops?

    // // Outer loop is by blocks, while the inner loop is by peers.
    // // I am not sure that this brings any speed improvements; it might. todo
    // 'outer: for block_idx in 0..*block_iters {
    //     for (peer_idx, peer) in work_peers.iter_mut().enumerate().take(*num_peers) {
    //         *block += 1;
    //
    //         if !check_bitfield(peer, *piece_index) {
    //             // TODO: Don't return, but mark the piece for retry, and do retry somehow.
    //             return Err(PeerError::MissingPiece(peer.addr, *piece_index));
    //         }
    //
    //         // Exchange messages with the peer
    //
    //         // Send a Request message for each block - we don't request pieces but blocks.
    //         let index = *piece_index as u32;
    //         let begin = u32::try_from(i * *block_len)?;
    //         let mut length = *block_len as u32;
    //         if *is_last_piece && i == *num_blocks_per_piece - 1 {
    //             length = *last_block_len as u32;
    //         }
    //         let msg = Message::new(
    //             MessageId::Request,
    //             Some(RequestPayload::new(index, begin, length).into()),
    //         );
    //         debug!(
    //             "block {block:3}/{total_num_blocks}, i = {i:3}: block_idx = {block_idx}, \
    //              peer_idx = {peer_idx}; pc idx = {index}, begin = {begin}, length = {length}"
    //         );
    //         eprintln!("block {block:3}/{total_num_blocks}, i = {i:3}: piece index = {index}, begin = {begin}, length = {length}"); //todo rem
    //         peer.send_msg(msg).await.context("Request")?;
    //
    //         // Wait for a Piece message for each block we've requested.
    //         let msg = peer.recv_msg().await.context("Piece")?;
    //         if msg.id != MessageId::Piece {
    //             return Err(PeerError::from((msg.id, MessageId::Piece)));
    //         }
    //
    //         let payload = &msg.payload.expect("Expected to have received some payload")[8..];
    //         hasher.update(payload); //todo
    //         file_writer.write_all(payload).await?; //todo
    //         // contents.extend(payload);todo rem
    //
    //         if i == *num_blocks_per_piece - 1 {
    //             break 'outer;
    //         }
    //
    //         i += 1;
    //     }
    // }

    let piece = hex::encode(piece_hash);
    let hash = hex::encode(hasher.finalize());
    if piece != hash {
        return Err(PeerError::HashMismatch(piece, hash));
    }

    // Ok(Block {
    //     piece: *piece_index,
    //     block: *block,
    //     data,
    // })
    Ok(Piece {
        piece: *piece_index,
        data,
    })
}

struct Piece {
    piece: usize,
    data: Vec<u8>,
}

struct Block {
    piece: usize,
    block: usize,
    data: Vec<u8>,
}

/// Checks if the peer has the required piece.
///
/// Returns `true` if it does, and `false` if it does not.
///
/// The challenge doesn't require this as all their peers have all the required pieces.
///
/// The Bitfield message is variable length. The high bit in the first byte corresponds to piece index 0.
/// Bits that are cleared indicated a missing piece, and set bits indicate a valid and available piece.
/// Spare bits at the end are set to zero.
fn peer_has_piece(peer: &Peer, piece_index: usize) -> bool {
    let bitfield = peer.bitfield.as_ref().unwrap_or_else(|| {
        panic!(
            "Expected the peer {} to have its bitfield field populated.",
            peer.addr
        )
    });
    let idx = piece_index / 8;
    let byte = bitfield[idx];
    let sr: u8 = 7 - ((piece_index % 8) as u8);
    let piece_bit: u8 = byte >> sr;
    if piece_bit & 1 != 1 {
        return false;
    }
    true
}

// // TODO: rem
// pub struct Piece {
//     index: usize,
//     data: Vec<u8>,
//     correct_hash: String, // todo: not necessary?
//     calc_hash: String,    // todo: not necessary?
// }

// // TODO: rem
// /// A helper function that is used for exchanging messages with the peers
// /// for getting blocks (sub-pieces) of data from them.
// ///
// /// Works with multiple peers and in a pipelined fashion, for improved download speed.
// ///
// /// Sends requests to peers, and gets responses from them.
// async fn get_blocks(file: File) -> Result<(), PeerError> {
//     Ok(())
// }
