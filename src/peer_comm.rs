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
//! Pipelining requests to a single peer can increase download speeds because it
//! avoids delays between blocks being sent to us from the peers.
//! Source (PDF): [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
//! Also see: https://wiki.theory.org/BitTorrentSpecification#Queuing
//!
//! `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
//!
//! `$ ./your_bittorrent.sh download -o /tmp/test-piece sample.torrent`

use std::cmp::min;
use std::collections::VecDeque;
use std::io::SeekFrom;
use std::net::SocketAddrV4;
use std::path::PathBuf;
use std::time::Instant;

use crate::config::Config;
use crate::constants::{BLOCK_SIZE, MAX_PIECE_SIZE, SHA1_LEN};
use crate::errors::PeerError;
use crate::message::{Message, MessageId, PiecePayload, RequestPayload};
use crate::meta_info::Info;
use crate::peer::Peer;
use crate::tracker::get_peers;

use anyhow::{Context, Result};
use log::{debug, info, trace, warn};
use rand::seq::SliceRandom;
use rand::thread_rng;
use sha1::{Digest, Sha1};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// A collection of peers that we are not currently downloading anything from.
type AvailablePeers = VecDeque<usize>;

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

/// Downloads a single piece of a file and stores it.
///
/// Arguments:
/// - config: [`Config`], application configuration,
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
/// Works with a single peer, but pipelines requests to it for increased download speed.
///
/// `$ ./your_bittorrent.sh download_piece -o /tmp/test-piece sample.torrent <piece_index>`
///
/// # Errors
/// - [`std::io::Error`], in case it can't create the output file,
/// - [`crate::errors::TrackerError`], in case it can't get peers,
/// - [`PeerError`], various variants, in case of various errors.
pub async fn download_piece(
    config: Config,
    output: &PathBuf,
    torrent: &PathBuf,
    piece_index: usize,
) -> Result<(), PeerError> {
    let work_params = get_work_params(torrent, Some(piece_index))?;

    let WorkParams {
        mut peers,
        info,
        num_pcs,
        piece_len,
        last_piece_len,
        block_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
        ..
    } = work_params;

    if piece_index >= num_pcs {
        return Err(PeerError::WrongPieceIndex(piece_index, num_pcs));
    }

    // Support working with multiple peers at the same time.
    let mut work_peers = local_get_peers(&mut peers, &info, config.max_num_peers).await?;
    let peer_idx = find_peer_for_piece(&work_peers, piece_index)?;
    let peer = &mut work_peers[peer_idx];

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
    recv_pieces(&config, &piece_params, peer, &mut file).await?;

    check_file_size(current_piece_len, output).await?;

    info!("Success!");

    Ok(())
}

/// Downloads a whole file and stores it.
///
/// Arguments:
/// - config: [`Config`], application configuration,
/// - output: &[`PathBuf`], path to the output file for storing the whole downloaded file
/// - torrent: &[`PathBuf`], path to a torrent file
///
/// Supports working with multiple peers at once.
///
/// `$ ./your_bittorrent.sh download -o /tmp/test.txt sample.torrent`
///
/// # Errors
/// - [`std::io::Error`], in case it can't create the output file,
/// - [`crate::errors::TrackerError`], in case it can't get peers,
/// - [`PeerError`], various variants, in case of various errors.
pub async fn download(
    config: Config,
    output: &PathBuf,
    torrent: &PathBuf,
) -> Result<(), PeerError> {
    let start = Instant::now();

    let work_params = get_work_params(torrent, None)?;
    let WorkParams {
        mut peers,
        info,
        file_len,
        num_pcs,
        piece_len,
        last_piece_len,
        block_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
    } = work_params;

    // Support working with multiple peers at the same time
    let mut work_peers: Vec<Peer> =
        local_get_peers(&mut peers, &info, config.max_num_peers).await?;

    // All piece hashes from the torrent file
    let mut missing_pieces = VecDeque::from_iter(info.pieces.0.iter().enumerate());

    let mut available_peers = AvailablePeers::from_iter(0..work_peers.len());

    let mut file = File::create(output).await?;
    file.set_len(file_len as u64).await?;

    // Send requests to peers.
    while let Some((piece_index, piece_hash)) = missing_pieces.pop_front() {
        // Find a peer that has the piece, and pop it off the collection.
        let peer_idx =
            match find_available_peer_for_piece(&work_peers, &mut available_peers, piece_index) {
                Ok(peer_idx) => peer_idx,
                Err(_) => {
                    missing_pieces.push_back((piece_index, piece_hash));
                    continue;
                }
            };
        // Don't forget to put it back! We can do it right away!
        available_peers.push_back(peer_idx);
        let peer = &mut work_peers[peer_idx];

        let piece_offset = piece_index * piece_len;

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
            piece_offset,
            peer_idx,
        };

        send_reqs(&config, &piece_params, peer).await?;
    }

    missing_pieces = VecDeque::from_iter(info.pieces.0.iter().enumerate());

    // Receive pieces from peers.
    while let Some((piece_index, piece_hash)) = missing_pieces.pop_front() {
        // Find a peer that has the piece, and pop it off the collection.
        let peer_idx =
            match find_available_peer_for_piece(&work_peers, &mut available_peers, piece_index) {
                Ok(peer_idx) => peer_idx,
                Err(_) => {
                    missing_pieces.push_back((piece_index, piece_hash));
                    continue;
                }
            };
        // Don't forget to put it back! We can do it right away!
        available_peers.push_back(peer_idx);
        let peer = &mut work_peers[peer_idx];

        let piece_offset = piece_index * piece_len;

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
            piece_offset,
            peer_idx,
        };

        recv_pieces(&config, &piece_params, peer, &mut file).await?;

        info!(
            "Piece {:2}/{num_pcs} downloaded and stored.",
            piece_index + 1
        );
        eprintln!("piece {:2}/{num_pcs} downloaded", piece_index + 1); //todo rem
    }

    debug!("Calculated file hash: {}", calc_file_hash(output).await?);
    check_file_size(file_len, output).await?;

    info!("Success! Took {:.3?} to complete.", start.elapsed());
    eprintln!("Success! Took {:.3?} to complete.", start.elapsed()); // todo: comment-out

    Ok(())
}

/// Sends a request for a single piece from a single peer.
///
/// Pieces are split into blocks of 16 kB or potentially less in case of the very last block,
/// and transferred as such. The blocks are assembled into a full piece when all of them
/// have been received.
///
/// Works with a single peer, but pipelines requests to it for increased download speed.
///
/// This improves download speeds because it pipelines requests
/// and avoids delays between blocks being sent to us from the peers.
/// Source (PDF): [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
/// Also see: https://wiki.theory.org/BitTorrentSpecification#Queuing
///
/// # Errors
/// - [`PeerError::TryFromIntError`], in case block offset cannot be calculated,
/// - [`PeerError::Other`] wrapping another error, in case it can't send a [`MessageId::Request`] message to the peer.
async fn send_reqs(
    config: &Config,
    piece_params: &PieceParams<'_>,
    peer: &mut Peer,
) -> Result<(), PeerError> {
    let PieceParams {
        num_pcs,
        piece_len,
        last_piece_len,
        block_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
        piece_index,
        piece_offset,
        peer_idx,
        ..
    } = piece_params;

    let is_last_piece = *piece_index == *num_pcs - 1;
    let mut current_piece_len = *piece_len;
    let mut current_num_blocks_per_piece = *num_blocks_per_piece;
    if is_last_piece {
        current_piece_len = *last_piece_len;
        current_num_blocks_per_piece = *num_blocks_in_last_piece;
    }
    trace!(
        "is_last_piece = {is_last_piece}, current_piece_len = {current_piece_len}, \
        current_num_blocks_per_piece = {current_num_blocks_per_piece}"
    );
    trace!("piece_index {piece_index} * piece_len {piece_len} = piece_offset {piece_offset}");

    // Index of the first block of this piece among all blocks in the torrent increased by one. Only used for logging.
    let starting_block = *piece_index * *num_blocks_per_piece + 1;

    // The combined loop counter - from both loops; represents the block ordinal number
    let mut i = 0usize;

    let num_reqs = min(config.max_pipelined_requests, current_num_blocks_per_piece);
    let block_iters = current_num_blocks_per_piece / num_reqs
        + (current_num_blocks_per_piece % num_reqs).clamp(0, 1);
    trace!("num_reqs = {num_reqs}, block_iters = {block_iters}");

    // Pipeline requests to the single peer.
    // Outer loop is by batches of blocks, while the inner loop is by requests to the single peer.
    for _block_idx in 0..block_iters {
        let mut j = 0usize;

        // Send several requests in a row to the peer, without waiting for responses at this moment.
        // We'll wait for the responses later, in another function.
        for _ in 0..num_reqs {
            // Send a Request message for each block - we don't request pieces but blocks.
            let index = *piece_index as u32;
            let begin = u32::try_from(i * *block_len)?;
            let mut length = *block_len as u32;
            if is_last_piece && i == current_num_blocks_per_piece - 1 {
                length = *last_block_len as u32;
            }
            let msg = Message::new(
                MessageId::Request,
                Some(RequestPayload::new(index, begin, length).into()),
            );
            let current_block = starting_block + i;
            debug!(
                "-> Blk req {current_block:4}/{total_num_blocks}, i = {i:4}: peer_idx = {peer_idx:2}; \
                piece_i = {index:3}, begin = {begin:6}, length = {length:5}"
            );
            // eprintln!("Blk req {current_block:3}/{total_num_blocks}, i = {i:3}: peer_idx = {peer_idx}, piece index = {index}, begin = {begin}, length = {length}"); //todo rem
            peer.feed(msg).await.context("Feed a Request message")?;

            if i == current_num_blocks_per_piece - 1 {
                break;
            }
            i += 1;
            j += 1;
        }
        i -= j;
        peer.flush()
            .await
            .context("Flush a batch of Request messages")?;
    }

    Ok(())
}

/// Fetches a single piece from a single peer and stores it.
///
/// Pieces are split into blocks of 16 kB or potentially less in case of the very last block,
/// and transferred as such. The blocks are assembled into a full piece when all of them
/// have been received.
///
/// The piece is assembled in memory and written to storage in its entirety after validating its hash.
///
/// # Errors
/// - [`PeerError::Other`] wrapping another error, in case it can't receive a [`MessageId::Piece`]
///   message from the peer,
/// - [`PeerError::WrongMessageId`], in case we don't receive a [`MessageId::Piece`] message,
/// - [`crate::errors::PiecePayloadError`], in case conversion from `&`[`Message`] to [`PiecePayload`] fails,
/// - [`PeerError::HashMismatch`], in case of bad hash value of the received piece,
/// - [`std::io::Error`], in case it can't write to file.
async fn recv_pieces(
    config: &Config,
    piece_params: &PieceParams<'_>,
    peer: &mut Peer,
    file: &mut File,
) -> Result<(), PeerError> {
    let PieceParams {
        num_pcs,
        piece_len,
        last_piece_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        total_num_blocks,
        piece_index,
        piece_hash,
        piece_offset,
        peer_idx,
        ..
    } = piece_params;

    let is_last_piece = *piece_index == *num_pcs - 1;
    let mut current_piece_len = *piece_len;
    let mut current_num_blocks_per_piece = *num_blocks_per_piece;
    if is_last_piece {
        current_piece_len = *last_piece_len;
        current_num_blocks_per_piece = *num_blocks_in_last_piece;
    }
    trace!(
        "is_last_piece = {is_last_piece}, current_piece_len = {current_piece_len}, \
        current_num_blocks_per_piece = {current_num_blocks_per_piece}"
    );
    trace!("piece_index {piece_index} * piece_len {piece_len} = piece_offset {piece_offset}");

    // The entire Piece data
    let mut data = [0u8; MAX_PIECE_SIZE];

    // For validating the hash of the received piece
    let mut hasher = Sha1::new();

    // Index of the first block of this piece among all blocks in the torrent increased by one. Only used for logging.
    let starting_block = *piece_index * *num_blocks_per_piece + 1;

    // The combined loop counter - from both loops; represents the block ordinal number
    let mut i = 0usize;

    let num_reqs = min(config.max_pipelined_requests, current_num_blocks_per_piece);
    let block_iters = current_num_blocks_per_piece / num_reqs
        + (current_num_blocks_per_piece % num_reqs).clamp(0, 1);
    trace!("num_reqs = {num_reqs}, block_iters = {block_iters}");

    // Fetch blocks from a single peer.
    // Receive a Piece message for each block we've requested. Pieces could arrive out of order in general case.
    // Outer loop is by batches of blocks, while the inner loop is by responses from the single peer.
    'outer: for _block_idx in 0..block_iters {
        for _ in 0..num_reqs {
            let msg = peer.recv_msg().await.context("Receive a Piece message")?;
            if msg.id != MessageId::Piece {
                return Err(PeerError::from((msg.id, MessageId::Piece)));
            }

            let payload: PiecePayload = (&msg).try_into()?;

            hasher.update(payload.block);

            let index = payload.index;
            // `begin` could be != `i * *block_len` in general case, because pieces could arrive out of order.
            let begin = payload.begin as usize;
            // `payload.block.len()` == `msg.len - 9`, and this is checked in `PiecePayload::try_from(&Message)`,
            // which is used above to get `payload`.
            let length = payload.block.len();
            data[begin..begin + length].copy_from_slice(payload.block);
            let current_block = starting_block + i;
            trace!(
                "<= Blk rcv {current_block:4}/{total_num_blocks}, i = {i:4}: peer_idx = {peer_idx:2}; \
                piece_i = {index:3}, begin = {begin:6}, length = {length:5}"
            );

            if i == current_num_blocks_per_piece - 1 {
                break 'outer;
            }
            i += 1;
        }
    }

    file.seek(SeekFrom::Start(*piece_offset as u64)).await?;
    file.write_all(&data[..current_piece_len]).await?;
    file.flush().await?;

    let piece = hex::encode(piece_hash);
    let hash = hex::encode(hasher.finalize());
    if piece != hash {
        return Err(PeerError::HashMismatch(piece, hash));
    }

    Ok(())
}

// /// Parameters for the [`fetch_piece`] function. TODO: rem
/// Parameters for the [`send_reqs`] & [`recv_pieces`] functions.
#[derive(Debug)]
struct PieceParams<'a> {
    num_pcs: usize,
    piece_len: usize,
    last_piece_len: usize,
    block_len: usize,
    num_blocks_per_piece: usize,
    num_blocks_in_last_piece: usize,
    last_block_len: usize,
    total_num_blocks: usize,
    piece_index: usize,
    piece_hash: &'a [u8; SHA1_LEN],
    piece_offset: usize,
    /// Here, `peer_idx` is used only for logging.
    peer_idx: usize,
}

// TODO: Remove entire function!
/// Fetches a single piece from a single peer and returns it.
///
/// Pieces are split into blocks of 16 kB or potentially less in case of the very last block,
/// and transferred as such. The blocks are assembled into a full piece when all of them
/// have been received.
///
/// Works with a single peer, but pipelines requests to it for increased download speed.
///
/// This improves download speeds because it pipelines requests
/// and avoids delays between blocks being sent to us from the peers.
/// Source (PDF): [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf)
/// Also see: https://wiki.theory.org/BitTorrentSpecification#Queuing
///
/// The piece is assembled in memory and written to storage in its entirety after validating its hash. // TODO: remove?
/// The piece is assembled in memory and returned in its entirety after validating its hash. // TODO: remove?
///
/// # Returns
/// - [`crate::peer_comm::Piece`], in case the peer has it, and we successfully received it and validated its hash value. // TODO: remove?
///
/// # Errors
/// - [`PeerError::MissingPiece`], in case the peer doesn't have the piece, // TODO: remove?
/// - [`PeerError::TryFromIntError`], in case block offset cannot be calculated,
/// - [`PeerError`] wrapping another error, in case it can't send a [`MessageId::Request`] message to the peer,
/// - [`PeerError`] wrapping another error, in case it can't receive a [`MessageId::Piece`] message from the peer,
/// - [`PeerError::WrongMessageId`], in case we don't receive a [`MessageId::Piece`] message,
/// - [`crate::errors::PiecePayloadError`], in case conversion from `&`[`Message`] to [`PiecePayload`] fails,
/// - [`PeerError::HashMismatch`], in case of bad hash value of the received piece,
/// - [`std::io::Error`], in case it can't write to file. // TODO: remove?
async fn _fetch_piece(
    config: &Config,
    piece_params: &PieceParams<'_>,
    peer: &mut Peer,
    file: &mut File,
) -> Result<(), PeerError> {
    // ) -> Result<Piece, PeerError> {  // TODO: remove?
    let PieceParams {
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
        piece_offset,
        peer_idx,
    } = piece_params;

    let is_last_piece = *piece_index == *num_pcs - 1;
    let mut current_piece_len = *piece_len;
    let mut current_num_blocks_per_piece = *num_blocks_per_piece;
    if is_last_piece {
        current_piece_len = *last_piece_len;
        current_num_blocks_per_piece = *num_blocks_in_last_piece;
    }
    trace!(
        "is_last_piece = {is_last_piece}, current_piece_len = {current_piece_len}, \
        current_num_blocks_per_piece = {current_num_blocks_per_piece}"
    );
    trace!("piece_index {piece_index} * piece_len {piece_len} = piece_offset {piece_offset}");

    // The entire Piece data
    // let mut data = Vec::with_capacity(*num_blocks_per_piece); // todo rem
    let mut data = [0u8; MAX_PIECE_SIZE];

    // For validating the hash of the received piece
    let mut hasher = Sha1::new();

    // Index of the first block of this piece among all blocks in the torrent increased by one. Only used for logging.
    let starting_block = *piece_index * *num_blocks_per_piece + 1;

    // The combined loop counter - from both loops; represents the block ordinal number
    let mut i = 0usize;

    // Fetch blocks from a single peer

    let num_reqs = min(config.max_pipelined_requests, current_num_blocks_per_piece);
    let block_iters = current_num_blocks_per_piece / num_reqs
        + (current_num_blocks_per_piece % num_reqs).clamp(0, 1);
    trace!("num_reqs = {num_reqs}, block_iters = {block_iters}");

    // Pipeline requests to the single peer.
    // Outer loop is by batches of blocks, while the inner loop is by requests to the single peer.
    'outer: for _block_idx in 0..block_iters {
        let mut j = 0usize;

        // Send several requests in a row to the peer, without waiting for responses at this moment.
        // We'll wait for the responses later, in the following loop.
        for _ in 0..num_reqs {
            // Send a Request message for each block - we don't request pieces but blocks.
            let index = *piece_index as u32;
            let begin = u32::try_from(i * *block_len)?;
            let mut length = *block_len as u32;
            if is_last_piece && i == current_num_blocks_per_piece - 1 {
                length = *last_block_len as u32;
            }
            let msg = Message::new(
                MessageId::Request,
                Some(RequestPayload::new(index, begin, length).into()),
            );
            let current_block = starting_block + i;
            debug!(
                "-> Blk req {current_block:4}/{total_num_blocks}, i = {i:4}: peer_idx = {peer_idx:2}; \
                piece_i = {index:3}, begin = {begin:6}, length = {length:5}"
            );
            // eprintln!("Blk req {current_block:3}/{total_num_blocks}, i = {i:3}: peer_idx = {peer_idx}, piece index = {index}, begin = {begin}, length = {length}"); //todo rem
            peer.feed(msg).await.context("Feed a Request message")?;

            if i == current_num_blocks_per_piece - 1 {
                break;
            }
            i += 1;
            j += 1;
        }
        i -= j;
        peer.flush()
            .await
            .context("Flush a batch of Request messages")?;

        // Receive a Piece message for each block we've requested. Pieces could arrive out of order in general case.
        for _ in 0..num_reqs {
            let msg = peer.recv_msg().await.context("Receive a Piece message")?;
            if msg.id != MessageId::Piece {
                return Err(PeerError::from((msg.id, MessageId::Piece)));
            }

            let payload: PiecePayload = (&msg).try_into()?;

            hasher.update(payload.block);

            let index = payload.index;
            // `begin` could be != `i * *block_len` in general case, because pieces could arrive out of order.
            let begin = payload.begin as usize;
            // `payload.block.len()` == `msg.len - 9`, and this is checked in `PiecePayload::try_from(&Message)`,
            // which is used above to get `payload`.
            let length = payload.block.len();
            data[begin..begin + length].copy_from_slice(payload.block);
            let current_block = starting_block + i;
            trace!(
                "<= Blk rcv {current_block:4}/{total_num_blocks}, i = {i:4}: peer_idx = {peer_idx:2}; \
                piece_i = {index:3}, begin = {begin:6}, length = {length:5}"
            );

            if i == current_num_blocks_per_piece - 1 {
                break 'outer;
            }
            i += 1;
        }
    }

    // let mut file_writer = BufWriter::with_capacity(current_piece_len, file); // todo remove
    file.seek(SeekFrom::Start(*piece_offset as u64)).await?;
    file.write_all(&data[..current_piece_len]).await?;
    file.flush().await?;

    let piece = hex::encode(piece_hash);
    let hash = hex::encode(hasher.finalize());
    if piece != hash {
        return Err(PeerError::HashMismatch(piece, hash));
    }

    // Ok(Piece { // todo: uncomment? length is missing - the last piece is smaller.
    //     index: *piece_index,
    //     data,
    // })

    Ok(())
}

#[derive(Debug)]
struct WorkParams {
    peers: Vec<SocketAddrV4>,
    info: Info,
    file_len: usize,
    num_pcs: usize,
    piece_len: usize,
    last_piece_len: usize,
    block_len: usize,
    num_blocks_per_piece: usize,
    num_blocks_in_last_piece: usize,
    last_block_len: usize,
    total_num_blocks: usize,
}

/// Calculates and returns basic work parameters.
///
/// # Errors
/// - [`crate::errors::TrackerError`], in case it can't get the list of peers.
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
    let num_pcs = file_len.div_ceil(piece_len);
    if last_piece_len == 0 {
        last_piece_len = piece_len;
    }

    debug!("piece_index = {:?}", piece_index);
    debug!("file_len = {}", file_len);
    debug!("num_pcs = {}", num_pcs);
    debug!("piece_len = {}", piece_len);
    debug!("last_piece_len = {}", last_piece_len);

    // Pieces are split into blocks and transferred as such.
    // Pieces ultimately need to be assembled from received blocks.
    // Block size is 16 kB (`BLOCK_SIZE`), except potentially for the last block which can be smaller.
    let block_len = BLOCK_SIZE;
    let num_blocks_per_piece = piece_len / block_len;
    let mut last_block_len = last_piece_len % block_len;
    let num_blocks_in_last_piece = last_piece_len / block_len + last_block_len.clamp(0, 1);
    if last_block_len == 0 {
        last_block_len = block_len;
    }
    let total_num_blocks = (num_pcs - 1) * num_blocks_per_piece + num_blocks_in_last_piece;

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
        piece_len,
        last_piece_len,
        block_len,
        num_blocks_per_piece,
        num_blocks_in_last_piece,
        last_block_len,
        total_num_blocks,
    })
}

/// Retrieves a certain number of peers from the tracker.
///
/// Tries to handshake with them, and if it succeeds, adds the peers to the list that it returns.
///
/// Skips peers that it isn't able to handshake with, or from which it doesn't receive a Bitfield or
/// an Unchoke message. A peer which doesn't send the Bitfield message doesn't have any piece anyway.
///
/// Supports working with multiple peers at the same time.
///
/// # Returns
/// - `work_peers`: `Vec<Peer>`, list of peers to work with
///
/// # Errors
/// - [`PeerError::Other`] wrapping another error, in case it can't send a [`MessageId::Interested`]
///   message to the peer,
/// - [`PeerError::NoPeers`], in case it doesn't find a peer to work with.
async fn local_get_peers(
    peers: &mut [SocketAddrV4],
    info: &Info,
    max_num_peers: usize,
) -> Result<Vec<Peer>, PeerError> {
    let peers_len = peers.len();
    let num_peers = min(max_num_peers, peers_len);
    debug!("max_num_peers = {max_num_peers}, peers_len = {peers_len}; num_peers = {num_peers}");

    let mut work_peers: Vec<Peer> = Vec::with_capacity(num_peers);

    // Get all peers to work with - handshake with them and store them.
    // Exchange messages with each peer: receive Bitfield, send Interested, receive Unchoke.
    //
    // Implementation notes:
    // - The selection of peers could be randomized, but it isn't required; rather, this is just an idea.
    // - Don't store peer_idx inside Peer, at least not as-is, because we could skip adding a peer in this loop.
    for (peer_idx, peer) in peers.iter_mut().enumerate().take(peers_len) {
        // Establish a TCP connection with a peer, and perform a handshake
        let mut peer = match handshake(peer, &info.info_hash).await {
            Ok(peer) => peer,
            Err(err) => {
                warn!("Handshake error: {err:#}");
                continue;
            }
        };
        trace!("00 Handshake with peer_idx {peer_idx}: {}", peer.addr);

        // Receive a Bitfield message
        // This message is optional, and need not be sent if a peer has no pieces.
        // If the peer doesn't have any pieces, we can skip it.
        let msg = match peer.recv_msg().await {
            Ok(msg) => msg,
            Err(err) => {
                warn!("Receive a Bitfield message: {err:#}");
                continue;
            }
        };
        trace!("01 peer_idx {peer_idx}: {msg}");
        if msg.id != MessageId::Bitfield {
            let err = PeerError::from((msg.id, MessageId::Bitfield));
            warn!("Receive a Bitfield message: {err:#}");
            continue;
        }
        peer.bitfield = Some(
            msg.payload
                .clone()
                .expect("Expected to have received a Bitfield message"),
        );

        // Send the Interested message
        let msg = Message::new(MessageId::Interested, None);
        peer.feed(msg)
            .await
            .context("Feed the Interested message")?;
        peer.flush().await.context("Flush the Interested message")?;

        // Receive an Unchoke message
        let msg = match peer.recv_msg().await {
            Ok(msg) => msg,
            Err(err) => {
                warn!("Receive an Unchoke message: {err:#}");
                continue;
            }
        };
        trace!("02 peer_idx {peer_idx}: {msg}");
        if msg.id != MessageId::Unchoke {
            let err = PeerError::from((msg.id, MessageId::Unchoke));
            warn!("Receive an Unchoke message: {err:#}");
            continue;
        }

        work_peers.push(peer);

        if work_peers.len() == num_peers {
            break;
        }
    }

    debug!("work_peers.len() = {}", work_peers.len());
    if work_peers.is_empty() {
        return Err(PeerError::NoPeers);
    }
    debug!("");

    Ok(work_peers)
}

// TODO: remove?
// /// Piece index and data
// #[derive(Debug)]
// struct Piece<'a> {
//     index: usize,
//     data: &'a [u8],
// }

/// Compares the expected file size with the written file size.
///
/// # Errors
/// - [`std::io::Result`], in case it can't open file or reads its metadata,
/// - [`PeerError::WrongLen`], in case the sizes don't match.
async fn check_file_size(expected_len: usize, path: &PathBuf) -> Result<(), PeerError> {
    let file = File::open(path).await?;
    let file_size = file.metadata().await?.len() as usize;
    info!(
        "Wrote {file_size} out of expected {expected_len} bytes to \"{}\".",
        path.display()
    );
    if expected_len != file_size {
        return Err(PeerError::WrongLen(expected_len, file_size));
    }
    Ok(())
}

/// Calculates the downloaded file hash and returns it.
///
/// # Errors
/// - [`std::io::Result`], in case it can't open or read file.
async fn calc_file_hash(path: &PathBuf) -> Result<String, PeerError> {
    let mut file = File::open(path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    let hash: [u8; SHA1_LEN] = *Sha1::digest(buffer).as_ref();
    let hash = hex::encode(hash);
    Ok(hash)
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

/// Checks all peers for the given piece and tries to find one that has it.
///
/// Returns a random peer for which it determines that it has the piece.
///
/// # Returns
/// - `peer_idx`: `usize`, index of a random peer in the list of peers, `work_peers`, that has the piece
///
/// # Errors
/// - [`PeerError::NoPeerHasPiece`], in case no peer has the piece.
fn find_peer_for_piece(work_peers: &[Peer], piece_index: usize) -> Result<usize, PeerError> {
    let mut list = vec![];

    for (peer_idx, peer) in work_peers.iter().enumerate() {
        if peer_has_piece(peer, piece_index) {
            list.push(peer_idx);
        } else {
            trace!("{}", PeerError::MissingPiece(peer.addr, piece_index));
        }
    }

    if list.is_empty() {
        return Err(PeerError::NoPeerHasPiece(piece_index));
    }

    list.shuffle(&mut thread_rng());

    Ok(list[0])
}

/// Checks all peers for the given piece and tries to find ones that have it.
///
/// Returns a list of peers for which it determines that they have the piece.
///
/// # Returns
/// - `Vec<usize>`, a list of indices of peers in the list of peers, `work_peers`, that have the piece
///
/// # Errors
/// - [`PeerError::NoPeerHasPiece`], in case no peer has the piece.
fn _find_peers_for_piece(work_peers: &[Peer], piece_index: usize) -> Result<Vec<usize>, PeerError> {
    let mut list = vec![];

    for (peer_idx, peer) in work_peers.iter().enumerate() {
        if peer_has_piece(peer, piece_index) {
            list.push(peer_idx);
        }
    }

    if list.is_empty() {
        return Err(PeerError::NoPeerHasPiece(piece_index));
    }

    Ok(list)
}

/// Checks all currently available peers for the given piece and tries to find one that has it.
///
/// Returns a peer for which it determines that it has the piece, by **popping it off**
/// the collection of the available peers that we pass in.
///
/// Don't forget to put the peer back in the collection of the available peers after
/// it has successfully downloaded a piece or if the download of a piece fails!
///
/// # Returns
/// - `peer_idx`: `usize`, index of a peer in the list of peers, `work_peers`, that has the piece
///
/// # Errors
/// - [`PeerError::PeerError::NoCurrentlyAvailablePeerOrPiece`], in case there are no currently available peers.
/// - [`PeerError::PeerError::NoCurrentlyAvailablePeerOrPiece`], in case no currently available peer has the piece.
fn find_available_peer_for_piece(
    work_peers: &[Peer],
    available_peers: &mut AvailablePeers,
    piece_index: usize,
) -> Result<usize, PeerError> {
    if available_peers.is_empty() {
        return Err(PeerError::NoCurrentlyAvailablePeerOrPiece(piece_index));
    }

    for peer_idx in available_peers.iter() {
        let peer = &work_peers[*peer_idx];
        if peer_has_piece(peer, piece_index) {
            return Ok(available_peers
                .pop_front()
                .expect("The available peers list shouldn't be empty."));
        }
    }

    Err(PeerError::NoCurrentlyAvailablePeerOrPiece(piece_index))
}
