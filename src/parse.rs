use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::decode::decode_bencoded_value;

pub fn info(path: &PathBuf) -> Result<String> {
    let contents: Vec<u8> = fs::read(path)?;
    let contents: &[u8] = contents.as_ref();

    let decoded = decode_bencoded_value(contents)?;
    let tracker = &decoded["announce"].to_string();
    let tracker_len = tracker.len();
    let tracker = &tracker[1..tracker_len - 1];
    let length = &decoded["info"]["length"].to_string();

    let mut result = String::new();
    result.push_str("Tracker URL: ");
    result.push_str(tracker);
    result.push('\n');
    result.push_str("Length: ");
    result.push_str(length);
    result.push('\n');

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info() {
        assert_eq!(
            "Tracker URL: http://bittorrent-test-tracker.codecrafters.io/announce\nLength: 92063\n",
            info(&PathBuf::from("sample.torrent")).unwrap()
        );
    }
}
