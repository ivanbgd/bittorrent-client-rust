# A Bittorrent Client

# Running the Client

Torrent files and magnet links are supported for downloading a single file.

- `./run.sh decode <encoded_value>`
- `./run.sh info <path_to_torrent_file>`
- `./run.sh peers <path_to_torrent_file>`
- `./run.sh handshake <path_to_torrent_file> <peer_ip>:<peer_port>`
    - Peer's IP address and port can be obtained by running the `peers` command and picking any peer from the list.
- `./run.sh download_piece -o <path_to_output_file> <path_to_torrent_file> <piece_index>`
- `./run.sh download -o <path_to_output_file> <path_to_torrent_file>`
- `./run.sh magnet_parse "<magnet-link>"`
- `./run.sh magnet_handshake "<magnet-link>"`
- `./run.sh magnet_info "<magnet-link>"`
- `./run.sh magnet_download_piece -o <path_to_output_file> "<magnet-link>" <piece_index>`
- `./run.sh magnet_download -o <path_to_output_file> "<magnet-link>"`

We can alternatively run it by `cargo run --release`, instead of `./run.sh`.

To enable the provided logging facility, first set the logging level by setting the `RUST_LOG` environment variable.  
To set it for the entire terminal session, execute `export RUST_LOG=debug`, for example, first.  
Or, prepend the run command with a desired log level; for example:  
`RUST_LOG=debug ./run.sh download -o <path_to_output_file> <path_to_torrent_file>`  
Choose between:  
`RUST_LOG=[trace | debug | info | warn]`  
*Note*: Logging is fully enabled only for the `download_piece` and `download` commands. Some commands
don't have any log output.

Sample torrent files are provided in the root of the repository,
as well as in the [test_samples](./test_samples) subdirectory.

*Note:* The sample torrent files can be used for testing the code, and in this case they work, but these are **NOT**
real-world torrent files!

# Limitations

This project is **NOT** production-ready!

First, it only supports single-file torrents, but even they are not fully, i.e., properly supported,
so even they don't work.

Secondly, multi-file torrents are not supported at all. This solution has placeholders to support the functionality,
but it hasn't been implemented.

We only support compact mode, but that is the recommended mode in practice anyway, so it should be enough.  
https://www.bittorrent.org/beps/bep_0023.html  
The assignment itself only supports the compact mode.

# Improvements Over the Requirements

- Optional application configuration, through the optional [config.json](config.json) file.
- Logging.
- Timing of the whole-file download.
- We check the Bitfield message payload to see whether a peer has the piece that we need.  
  The project doesn't require this as all their peers have all the required pieces during testing,
  but in real life this is practically required.
- We pipeline requests to a single peer for increased download speed.  
  This was suggested as optional but is practically required because of the test timeout.  
  This increases the download speed from a single peer significantly.
- We also added working with multiple peers at once, on top of pipelining requests to a single peer.  
  This was also suggested as optional, but not required.  
  Still, this is something that is required in case of real-life torrents.
  Namely, we cannot rely on a single peer having all pieces in real life. In the project's test suite, they do.  
  We try to find a peer that has a piece, for every piece.

# Possible Improvements

- Make the application work for real-life single-file torrents.
- Make the application work for real-life multiple-file torrents.
- Send keep-alive messages to connected peers.
- Discover new peers at regular time intervals, but this is not required in the project.
- Whenever a piece is successfully downloaded and written to file, store the necessary information (work parameters)
  in a file that can be used to resume download of the file when the application is restarted, if the file hasn't
  been downloaded fully.
- The [BitTorrent Economics Paper](http://bittorrent.org/bittorrentecon.pdf) (PDF) has more ideas,
  but none of them are required in this project.
