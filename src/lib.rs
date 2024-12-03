//! # A BitTorrent Client Library

use std::path::PathBuf;

pub mod bencode;
pub mod cli;
pub mod config;
pub mod constants;
pub mod errors;
pub mod magnet;
pub mod message;
pub mod meta_info;
pub mod peer;
pub mod peer_comm;
pub mod pieces;
pub mod tracker;

/// Source of metadata (the Info dictionary):
/// - Torrent file (path)
/// - Magnet link (link)
pub enum MetadataSource<'a> {
    TorrentFile(&'a PathBuf),
    MagnetLink(&'a str),
}
