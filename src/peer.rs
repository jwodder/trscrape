use crate::types::PeerId;
use bendy::decoding::{Error as BendyError, FromBencode, Object, ResultExt};
use std::fmt;
use std::net::{AddrParseError, IpAddr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub(crate) struct Peer {
    pub(crate) address: SocketAddr,
    pub(crate) id: Option<PeerId>,
    pub(crate) requires_crypto: bool,
}

impl FromStr for Peer {
    type Err = AddrParseError;

    fn from_str(s: &str) -> Result<Peer, AddrParseError> {
        let address = s.parse::<SocketAddr>()?;
        Ok(Peer {
            address,
            id: None,
            requires_crypto: false,
        })
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<Peer {}>", self.address)
    }
}

impl From<SocketAddr> for Peer {
    fn from(address: SocketAddr) -> Peer {
        Peer {
            address,
            id: None,
            requires_crypto: false,
        }
    }
}

impl From<SocketAddrV4> for Peer {
    fn from(addr: SocketAddrV4) -> Peer {
        Peer {
            address: addr.into(),
            id: None,
            requires_crypto: false,
        }
    }
}

impl From<SocketAddrV6> for Peer {
    fn from(addr: SocketAddrV6) -> Peer {
        Peer {
            address: addr.into(),
            id: None,
            requires_crypto: false,
        }
    }
}

impl FromBencode for Peer {
    fn decode_bencode_object(object: Object<'_, '_>) -> Result<Peer, BendyError> {
        let mut peer_id = None;
        let mut ip = None;
        let mut port = None;
        let mut dd = object.try_into_dictionary()?;
        while let Some(kv) = dd.next_pair()? {
            match kv {
                (b"peer id", v) => {
                    let buf = v.try_into_bytes().context("peer id")?;
                    match PeerId::try_from(buf) {
                        Ok(id) => {
                            peer_id = Some(id);
                        }
                        Err(e) => return Err(BendyError::malformed_content(e).context("peer id")),
                    }
                }
                (b"ip", v) => {
                    let s = match std::str::from_utf8(v.try_into_bytes().context("peer id")?) {
                        Ok(s) => s,
                        Err(e) => return Err(BendyError::malformed_content(e).context("ip")),
                    };
                    // Note that BEP 3 technically allows non-compact `ip`
                    // values to be domain names as well, but we're not
                    // supporting that.
                    match s.parse::<IpAddr>() {
                        Ok(ipaddr) => {
                            ip = Some(ipaddr);
                        }
                        Err(e) => return Err(BendyError::malformed_content(e).context("ip")),
                    }
                }
                (b"port", v) => {
                    port = Some(u16::decode_bencode_object(v).context("port")?);
                }
                _ => (),
            }
        }
        let ip = ip.ok_or_else(|| BendyError::missing_field("ip"))?;
        let port = port.ok_or_else(|| BendyError::missing_field("port"))?;
        Ok(Peer {
            address: SocketAddr::new(ip, port),
            id: peer_id,
            requires_crypto: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::{UnbencodeError, decode_bencode};

    #[test]
    fn test_unbencode_peer() {
        let peer = decode_bencode::<Peer>(
            b"d2:ip9:127.0.0.17:peer id20:-PRE-123-abcdefghijk4:porti8080ee",
        )
        .unwrap();
        assert_eq!(
            peer.address,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(peer.id, Some(PeerId::from(b"-PRE-123-abcdefghijk")));
    }

    #[test]
    fn test_unbencode_peer_no_peer_id() {
        let peer = decode_bencode::<Peer>(b"d2:ip9:127.0.0.14:porti8080ee").unwrap();
        assert_eq!(
            peer.address,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(peer.id, None);
    }

    #[test]
    fn test_unbencode_peer_extra_field() {
        let peer = decode_bencode::<Peer>(
            b"d2:ip9:127.0.0.17:peer id20:-PRE-123-abcdefghijk4:porti8080e5:speedi65535ee",
        )
        .unwrap();
        assert_eq!(
            peer.address,
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(peer.id, Some(PeerId::from(b"-PRE-123-abcdefghijk")));
    }

    #[test]
    fn test_unbencode_peer_empty() {
        assert!(matches!(
            decode_bencode::<Peer>(b""),
            Err(UnbencodeError::NoData)
        ));
    }

    #[test]
    fn test_unbencode_peer_trailing_bencode() {
        let r = decode_bencode::<Peer>(
            b"d2:ip9:127.0.0.17:peer id20:-PRE-123-abcdefghijk4:porti8080ee2:hi",
        );
        assert!(matches!(r, Err(UnbencodeError::TrailingData)));
    }

    #[test]
    fn test_unbencode_peer_trailing_garbage() {
        let r = decode_bencode::<Peer>(
            b"d2:ip9:127.0.0.17:peer id20:-PRE-123-abcdefghijk4:porti8080eeqqq",
        );
        assert!(matches!(r, Err(UnbencodeError::TrailingData)));
    }
}
