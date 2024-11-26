//! # Magnet links
//!
//! [Magnet links](https://www.bittorrent.org/beps/bep_0009.html) allow users to download files
//! from peers without needing a torrent file.
//!
//! The magnet URI format is:
//!
//! `v1: magnet:?xt=urn:btih:<info-hash>&dn=<name>&tr=<tracker-url>&x.pe=<peer-address>`
//!
//! Unlike .torrent files, magnet links don't contain information like file length, piece length and piece hashes.
//! They only include the bare minimum information necessary to discover peers.
//! A client can then request the rest of the information from peers using the metadata exchange protocol.
//!
//! These are the query parameters in a magnet link:
//!
//! - `xt`: `urn:btih`: followed by the 40-char hex-encoded info hash
//!   (example: `urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165`).
//! - `dn`: The name of the file to be downloaded (example: `magnet1.gif`).
//! - `tr`: The tracker URL (example: `http://bittorrent-test-tracker.codecrafters.io/announce`).
//!   The value must be URL-encoded.
//!
//! ## Notes
//!
//! - We'll be using v1 of [magnet URI format](https://www.bittorrent.org/beps/bep_0009.html#magnet-uri-format).
//!   v2 is not widely used yet.
//! - `xt` (info hash) is the only required parameter, all others are optional.
//! - A magnet link can contain multiple tracker URLs, but for the purposes of this challenge it'll only contain one.
//!
//! Usage:
//! ```shell
//! $ ./your_bittorrent.sh magnet_parse <magnet-link>
//! ```
//!
//! Example:
//! ```shell
//! $ ./your_bittorrent.sh magnet_parse magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce
//! ```
//!
//! Expected response:
//! ```shell
//! Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce
//! Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165
//! ```
//!
//! More info at:
//! - https://www.bittorrent.org/beps/bep_0009.html
//! - https://en.wikipedia.org/wiki/Magnet_URI_scheme

use anyhow::Result;

use crate::errors::MagnetError;
use crate::magnet::magnet_link::MagnetLink;

/// Parses a given magnet link.
pub fn parse_magnet_link(magnet_link: &str) -> Result<MagnetLink, MagnetError> {
    let magnet_link: MagnetLink = magnet_link.try_into()?;

    Ok(magnet_link)
}

mod magnet_link {
    //! Magnet link

    use std::fmt::{Display, Formatter};

    use crate::errors::MagnetLinkError;
    use anyhow::Result;

    /// Magnet link
    #[derive(Debug)]
    pub struct MagnetLink {
        /// Exact Topic, `xt`, specifies the URN containing file hash. Mandatory.
        pub xt: String,

        /// Display name, `dn`, may be used by the client to display while waiting for metadata. Optional.
        pub dn: Option<String>,

        /// Tracker URL. Optional.
        /// Used to obtain resources for BitTorrent downloads without a need for DHT support.
        /// The value must be URL encoded.
        pub tr: Option<String>,
    }

    impl Display for MagnetLink {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            if let Some(tr) = &self.tr {
                writeln!(f, "Tracker URL: {}", *tr)?;
            };

            writeln!(f, "Info Hash: {}", self.xt)?;

            if let Some(dn) = &self.dn {
                write!(f, "Display Name: {}", *dn)
            } else {
                write!(f, "")
            }
        }
    }

    impl TryFrom<&str> for MagnetLink {
        type Error = MagnetLinkError;

        fn try_from(value: &str) -> Result<Self, Self::Error> {
            if !value.starts_with("magnet:?") {
                return Err(MagnetLinkError::NoMagnet(value.to_string()));
            }
            let (_magnet, rest) = value.split_at("magnet:?".len());
            if !rest.contains("xt=urn:bt") {
                return Err(MagnetLinkError::NoXt(value.to_string()));
            }

            let mut xt = "".to_string();
            let mut dn = None;
            let mut tr = None;

            // Magnet URIs consist of a series of one or more parameters, the order of which is not significant,
            // formatted in the same way as query strings that ordinarily terminate HTTP URLs.
            //
            // Support only v1 of the magnet URI format from this point on.
            let params = rest.split('&');
            for elt in params {
                if let Some(param) = elt.split_once('=') {
                    match param.0 {
                        "xt" => xt = param.1.split_at("urn:btih:".len()).1.to_string(),
                        "dn" => dn = Some(param.1.to_string()),
                        "tr" => tr = Some(param.1.to_string()),
                        _ => {}
                    }
                }
            }

            Ok(Self { xt, dn, tr })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::MagnetLinkError;

    #[test]
    fn parse_magnet_link_xt_dn_tr() {
        let example = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&dn=magnet1.gif&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n\
            Display Name: magnet1.gif\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_xt_tr_dn() {
        let example = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce&dn=magnet1.gif";
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n\
            Display Name: magnet1.gif\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_dn_xt_tr() {
        let example = "magnet:?dn=magnet1.gif&xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165&tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n\
            Display Name: magnet1.gif\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_tr_xt() {
        let example = "magnet:?tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce&xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165";
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\n\
            Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_xt() {
        let example = "magnet:?xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165";
        assert_eq!(
            "Info Hash: ad42ce8109f54c99613ce38f9b4d87e70f24a165\n",
            format!("{}", parse_magnet_link(example).unwrap())
        );
    }

    #[test]
    fn parse_magnet_link_no_params() {
        let example = "magnet:?";
        assert_eq!(
            MagnetError::from(MagnetLinkError::NoXt(example.to_string())),
            parse_magnet_link(example).unwrap_err()
        );
    }

    #[test]
    fn parse_magnet_link_no_xt() {
        let example = "magnet:?tr=http%3A%2F%2Fbittorrent-test-tracker.codecrafters.io%2Fannounce";
        assert_eq!(
            MagnetError::from(MagnetLinkError::NoXt(example.to_string())),
            parse_magnet_link(example).unwrap_err()
        );
    }

    #[test]
    fn parse_magnet_link_no_magnet() {
        let example = "xt=urn:btih:ad42ce8109f54c99613ce38f9b4d87e70f24a165";
        assert_eq!(
            MagnetError::from(MagnetLinkError::NoMagnet(example.to_string())),
            parse_magnet_link(example).unwrap_err()
        );
    }
}
