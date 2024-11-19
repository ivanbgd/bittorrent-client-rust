[![progress-banner](https://backend.codecrafters.io/progress/bittorrent/ca143921-9bbb-45ad-81e4-25927be2c7a8)](https://app.codecrafters.io/users/ivanbgd?r=2qF)

This is a starting point for Rust solutions to the
["Build Your Own BitTorrent" Challenge](https://app.codecrafters.io/courses/bittorrent/overview).

In this challenge, you’ll build a BitTorrent client that's capable of parsing a
.torrent file and downloading a file from a peer. Along the way, we’ll learn
about how torrent files are structured, HTTP trackers, BitTorrent’s Peer
Protocol, pipelining and more.

**Note**: If you're viewing this repo on GitHub, head over to
[codecrafters.io](https://codecrafters.io) to try the challenge.

# Passing the first stage

The entry point for your BitTorrent implementation is in `src/main.rs`. Study
and uncomment the relevant code, and push your changes to pass the first stage:

```sh
git commit -am "pass 1st stage" # any msg
git push origin master
```

Time to move on to the next stage!

# Stage 2 & beyond

Note: This section is for stages 2 and beyond.

1. Ensure you have `cargo (1.70)` installed locally
2. Run `./your_bittorrent.sh` to run your program, which is implemented in
   `src/main.rs`. This command compiles your Rust project, so it might be slow
   the first time you run it. Subsequent runs will be fast.
3. Commit your changes and run `git push origin master` to submit your solution
   to CodeCrafters. Test output will be streamed to your terminal.

# Running the Program

- `./your_bittorrent.sh decode <encoded_value>`
- `./your_bittorrent.sh info <path_to_torrent_file>`
- `./your_bittorrent.sh peers <path_to_torrent_file>`
- `./your_bittorrent.sh handshake <path_to_torrent_file> <peer_ip>:<peer_port>`
    - Peer's IP address and port can be obtained by running the `peers` command and picking any peer from the list.
- `./your_bittorrent.sh download_piece -o <path_to_output_file> <path_to_torrent_file> <piece_index>`
- `./your_bittorrent.sh download -o <path_to_output_file> <path_to_torrent_file>`

We can alternatively run it by `cargo run`, instead of `./your_bittorrent.sh`.

To enable the logging facility, first set the logging level by executing
`RUST_LOG=debug` or `RUST_LOG=info`, for example.

Sample torrent files are provided in the root of the repository,
as well as in the [test_samples](./test_samples) subdirectory.

*Note:* The sample torrent files can be used for testing the code, and in this case they work, but these are **NOT**
real-world torrent files!

# Limitations

This challenge is **NOT** production-ready!

First, it only supports single-file torrents, but even they are not fully, i.e., properly supported,
so even they don't work.

Secondly, multi-file torrents are not supported at all. This solution has placeholders to support the functionality,
but it hasn't been implemented.

We only support compact mode, but that is the recommended mode in practice anyway, so it should be enough.  
https://www.bittorrent.org/beps/bep_0023.html  
The assignment itself only supports the compact mode.

# Possible improvements

- The Bitfiled message is optional, and a peer doesn't need to send it in case it doesn't have any piece. 
