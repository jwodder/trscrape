use std::time::Duration;

pub(crate) const TRACKER_TIMEOUT: Duration = Duration::from_secs(30);

/// Size of buffer for receiving incoming UDP packets.  Any packets longer than
/// this are truncated.
pub(crate) const UDP_PACKET_LEN: usize = 65535;
