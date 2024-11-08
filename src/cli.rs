use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// BitTorrent client
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "snake_case")]
pub enum Commands {
    /// Decode a Bencoded value
    Decode {
        /// String, integer, list or dictionary
        encoded_value: String,
    },

    /// Print a torrent's meta info
    Info {
        /// Path to a torrent file
        torrent: PathBuf,
    },

    /// Print the peers list in compact mode
    Peers {
        /// Path to a torrent file
        torrent: PathBuf,
    },

    /// Establish a TCP connection with a peer and complete a handshake
    ///
    /// Prints the hexadecimal representation of the peer ID received during the handshake.
    Handshake {
        /// Path to a torrent file
        torrent: PathBuf,
        /// <peer_ip>:<peer_port> (example: 127.0.0.1:8080)
        peer: std::net::SocketAddrV4,
    },

    /// Download a piece and save it to disk
    // #[command(name = "download_piece")]
    DownloadPiece {
        /// Path to an output file for the piece
        #[arg(short, long)]
        output: PathBuf,

        /// Path to a torrent file
        torrent: PathBuf,

        /// Zero-based piece index
        piece_index: usize,
    },
}
