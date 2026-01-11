[![Project Status: Concept – Minimal or no implementation has been done yet, or the repository is only intended to be a limited example, demo, or proof-of-concept.](https://www.repostatus.org/badges/latest/concept.svg)](https://www.repostatus.org/#concept)
[![CI Status](https://github.com/jwodder/trscrape/actions/workflows/test.yml/badge.svg)](https://github.com/jwodder/trscrape/actions/workflows/test.yml)
[![codecov.io](https://codecov.io/gh/jwodder/trscrape/branch/main/graph/badge.svg)](https://codecov.io/gh/jwodder/trscrape)
[![Minimum Supported Rust Version](https://img.shields.io/badge/MSRV-1.88-orange)](https://www.rust-lang.org)
[![MIT License](https://img.shields.io/github/license/jwodder/trscrape.svg)](https://opensource.org/licenses/MIT)

[GitHub](https://github.com/jwodder/trscrape) | [Issues](https://github.com/jwodder/trscrape/issues)

`trscrape` is a [Rust][] program for "scraping" [BitTorrent][] [trackers][] for
the numbers of seeders, leechers, and completed downloads for a given set of
info hashes.  It supports both HTTP trackers (following [BEP 48][]) and UDP
trackers (following [BEP 15][]).

[Rust]: https://www.rust-lang.org
[BitTorrent]: https://en.wikipedia.org/wiki/BitTorrent
[trackers]: https://en.wikipedia.org/wiki/BitTorrent_tracker
[BEP 48]: https://www.bittorrent.org/beps/bep_0048.html
[BEP 15]: https://www.bittorrent.org/beps/bep_0015.html

Installation
============

In order to install `trscrape`, you first need to have [Rust and Cargo
installed](https://www.rust-lang.org/tools/install).  You can then build the
latest version of `trscrape` and install it in `~/.cargo/bin` by running:

    cargo install --git https://github.com/jwodder/trscrape

Usage
=====

    trscrape [<options>] <tracker> <infohash> ...

The arguments to the `trscrape` command are a tracker URL followed by up to 50
torrent info hashes (specified as 40-character hex strings).  `trscrape`
queries the given tracker for statistics on the given info hashes and outputs
the results in the following format in the same order that the info hashes were
given on the command line:

```
da39a3ee5e6b4b0d3255bfef95601890afd80709:
  Complete/Seeders: 10
  Incomplete/Leechers: 0
  Downloaded: 32

b851474b74f65cd19f981c723590e3e520242b97:
  Complete/Seeders: 105
  Incomplete/Leechers: 42
  Downloaded: 1337
```

For HTTP trackers, if a given info hash is not being tracked, the output for
that hash will look like this instead:

```
da39a3ee5e6b4b0d3255bfef95601890afd80709: --- not tracked ---
```

Options
-------

- `-t <INT>`, `--timeout <INT>` — Wait at most `<INT>` seconds for the tracker
  to respond to our scrape request [default: 30]

- `--trace` — Emit logs of network activity
