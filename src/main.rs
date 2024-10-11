//! Usage: ./your_bittorrent.sh decode <encoded_value>

use std::env;

use anyhow::Result;

use bittorrent_starter_rust::decode::decode_bencoded_value;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    match command.as_str() {
        "decode" => {
            let encoded_value = &args[2];
            let decoded_value = decode_bencoded_value(encoded_value)?;
            println!("{}", decoded_value);
        }
        _ => {
            println!("Unknown command: {}", args[1]);
        }
    }

    Ok(())
}
