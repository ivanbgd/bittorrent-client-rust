//! Usage:
//! - `./your_bittorrent.sh decode <encoded_value>`
//! - `./your_bittorrent.sh info <path_to_torrent_file>`
//! - `./your_bittorrent.sh peers <path_to_torrent_file>`
//! - `./your_bittorrent.sh handshake <path_to_torrent_file> <peer_ip>:<peer_port>`

use anyhow::Result;
use clap::Parser;

use bittorrent_starter_rust::cli::{Args, Commands};
use bittorrent_starter_rust::decode::decode_bencoded_value;
use bittorrent_starter_rust::handshake::handshake;
use bittorrent_starter_rust::meta_info::meta_info;
use bittorrent_starter_rust::tracker::get_peers;

fn main() -> Result<()> {
    let args = Args::parse();

    match &args.command {
        Commands::Decode { encoded_value } => {
            let decoded_value = decode_bencoded_value(encoded_value.as_bytes())?;
            println!("{}", decoded_value);
        }
        Commands::Info { path } => {
            let meta = meta_info(path)?;
            println!("{}", meta);
        }
        Commands::Peers { path } => {
            let peers = get_peers(path)?;
            println!("{}", peers);
        }
        Commands::Handshake { path, peer } => {
            let peer_id = handshake(path, peer)?;
            println!("Peer ID: {}", peer_id);
        }
    }

    Ok(())
}
