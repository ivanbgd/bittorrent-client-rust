//! Usage:
//! - `./your_bittorrent.sh decode <encoded_value>`
//! - `./your_bittorrent.sh info <path_to_torrent_file>`
//! - `./your_bittorrent.sh peers <path_to_torrent_file>`
//! - `./your_bittorrent.sh handshake <path_to_torrent_file> <peer_ip>:<peer_port>`
//! - `./your_bittorrent.sh download_piece -o <path_to_output_file> <path_to_torrent_file> <piece_index>`

use anyhow::Result;
use clap::Parser;

use bittorrent_starter_rust::cli::{Args, Commands};
use bittorrent_starter_rust::decode::decode_bencoded_value;
use bittorrent_starter_rust::errors::ae2s;
use bittorrent_starter_rust::meta_info::meta_info;
use bittorrent_starter_rust::peer_comm::{download_piece, handshake};
use bittorrent_starter_rust::tracker::get_peers;

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();

    match &args.command {
        Commands::Decode { encoded_value } => {
            let decoded_value = decode_bencoded_value(encoded_value.as_bytes()).map_err(ae2s)?;
            println!("{}", decoded_value);
        }
        Commands::Info { torrent } => {
            let meta = meta_info(torrent)?;
            println!("{}", meta);
        }
        Commands::Peers { torrent } => {
            let (peers, _) = get_peers(torrent)?;
            println!("{}", peers);
        }
        Commands::Handshake { torrent, peer } => {
            let peer = handshake(peer, &meta_info(torrent)?.info.info_hash)?;
            println!("Peer ID: {}", peer);
        }
        Commands::DownloadPiece {
            output,
            torrent,
            piece_index,
        } => {
            download_piece(output, torrent, *piece_index)?;
        }
    }

    Ok(())
}
