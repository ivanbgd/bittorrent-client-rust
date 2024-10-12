//! Usage: ./your_bittorrent.sh decode <encoded_value>

use anyhow::Result;
use std::env;
use std::path::PathBuf;

use bittorrent_starter_rust::decode::decode_bencoded_value;
use bittorrent_starter_rust::parse::info;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    match command.as_str() {
        "decode" => {
            let encoded_value = &args[2];
            let decoded_value = decode_bencoded_value(encoded_value.as_bytes())?;
            println!("{}", decoded_value);
        }
        "info" => {
            let path = PathBuf::from(&args[2]);
            let meta = info(&path)?;
            println!("{}", meta);
        }
        _ => {
            println!("Unknown command: {}", args[1]);
        }
    }

    Ok(())
}
