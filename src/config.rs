//! # Configuration
//!
//! Configuration used throughout the application
//!
//! The function `get_config` tries to read and parse the optional "config.json" file.
//! If it succeeds, it returns the configuration from the JSON file.
//! If it fails to find the file, it uses default values from [`crate::constants`].
//! If the file isn't formatted properly, or if a field is missing, the function will panic.

use std::fs::File;

use crate::constants::{CONFIG_FILE_PATH, MAX_NUM_PEERS, MAX_PIPELINED_REQUESTS};

/// The application configuration
///
/// It can be stored in the optional [`CONFIG_FILE_PATH`] file, or if the file is not present,
/// default values will be read from [`crate::constants`].
pub struct Config {
    pub max_num_peers: usize,
    pub max_pipelined_requests: usize,
}

/// Tries to read and parse the optional "config.json" file.
/// If it succeeds, it returns the configuration from the JSON file.
/// If it fails to find the file, it uses default values from [`crate::constants`].
///
/// # Panics
/// If the file isn't formatted properly, or if a field is missing, the function will panic.
pub fn get_config() -> Config {
    match File::open(CONFIG_FILE_PATH) {
        Ok(file) => {
            let json: serde_json::Value = serde_json::from_reader(file).unwrap_or_else(|err| {
                panic!("{CONFIG_FILE_PATH} is not formatted properly: {err}")
            });
            Config {
                max_num_peers: serde_json::from_value(
                    json.get("max_num_peers")
                        .expect("missing the max_num_peers key")
                        .clone(),
                )
                .unwrap(),
                max_pipelined_requests: serde_json::from_value(
                    json.get("max_pipelined_requests")
                        .expect("missing the max_pipelined_requests key")
                        .clone(),
                )
                .unwrap(),
            }
        }
        Err(_) => Config {
            max_num_peers: MAX_NUM_PEERS,
            max_pipelined_requests: MAX_PIPELINED_REQUESTS,
        },
    }
}
