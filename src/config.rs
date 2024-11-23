//! # Configuration
//!
//! Configuration used throughout the application
//!
//! The function [`get_config`] tries to read and parse the optional "config.json" file.
//!
//! If it succeeds, it returns the configuration from the JSON file.
//!
//! If it fails to find the file, it uses default values from [`crate::constants`].
//!
//! If the file is missing a field, the field will be assigned its default value from [`crate::constants`].
//!
//! If the file isn't formatted properly, the function will panic.

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

impl Default for Config {
    fn default() -> Self {
        Self {
            max_num_peers: MAX_NUM_PEERS,
            max_pipelined_requests: MAX_PIPELINED_REQUESTS,
        }
    }
}

/// Tries to read and parse the optional "config.json" file.
///
/// If it succeeds, it returns the configuration from the JSON file.
///
/// If it fails to find the file, it uses default values from [`crate::constants`].
///
/// If the file is missing a field, the field will be assigned its default value from [`crate::constants`].
///
/// # Panics
/// If the file isn't formatted properly, the function will panic.
pub fn get_config() -> Config {
    match File::open(CONFIG_FILE_PATH) {
        Ok(file) => {
            let json: serde_json::Value = serde_json::from_reader(file).unwrap_or_else(|err| {
                panic!("{CONFIG_FILE_PATH} is not formatted properly: {err}")
            });
            Config {
                max_num_peers: match json.get("max_num_peers") {
                    Some(v) => serde_json::from_value(v.clone()).unwrap_or(MAX_NUM_PEERS),
                    None => MAX_NUM_PEERS,
                },
                max_pipelined_requests: match json.get("max_pipelined_requests") {
                    Some(v) => serde_json::from_value(v.clone()).unwrap_or(MAX_PIPELINED_REQUESTS),
                    None => MAX_PIPELINED_REQUESTS,
                },
            }
        }
        Err(_) => Config::default(),
    }
}
