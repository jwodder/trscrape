/// "left" value to use when announcing to a tracker for a torrent we have only
/// the magnet link of
pub(crate) const LEFT: u64 = 65535;

/// Prefix for generated peer IDs
pub(crate) static PEER_ID_PREFIX: &str = "-TRSCR-";

/// Size of buffer for receiving incoming UDP packets.  Any packets longer than
/// this are truncated.
pub(crate) const UDP_PACKET_LEN: usize = 65535;

/// Maximum metadata size to accept
pub(crate) const MAX_INFO_LENGTH: usize = 20 << 20; // 20 MiB

/// Extended message ID to declare for receiving BEP 9 messages
pub(crate) const UT_METADATA: u8 = 42;

/// Client string to send in extended handshakes and to use as the "Created by"
/// field in Torrent files
pub(crate) static CLIENT: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

/// Maximum length of a message to accept from a peer
pub(crate) const MAX_PEER_MSG_LEN: usize = 65535;
