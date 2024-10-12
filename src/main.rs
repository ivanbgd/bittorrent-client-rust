//! Usage:
//! - `./your_bittorrent.sh decode <encoded_value>`
//! - `./your_bittorrent.sh info sample.torrent`

use anyhow::Result;
use clap::Parser;

use bittorrent_starter_rust::cli::{Args, Commands};
use bittorrent_starter_rust::decode::decode_bencoded_value;
use bittorrent_starter_rust::parse::meta_info;

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
    }

    Ok(())
}
