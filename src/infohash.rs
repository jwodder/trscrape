use crate::util::{PacketError, TryFromBuf};
use bytes::{Buf, Bytes};
use data_encoding::{DecodeError, HEXLOWER_PERMISSIVE};
use std::borrow::Cow;
use std::fmt;
use thiserror::Error;
use url::Url;

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct InfoHash([u8; InfoHash::LENGTH]);

impl InfoHash {
    pub(crate) const LENGTH: usize = 20;

    pub(crate) fn from_hex(s: &str) -> Result<InfoHash, InfoHashError> {
        HEXLOWER_PERMISSIVE
            .decode(s.as_bytes())
            .map_err(InfoHashError::InvalidHex)?
            .try_into()
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub(crate) fn add_query_param(&self, url: &mut Url) {
        add_bytes_query_param(url, "info_hash", &self.0);
    }
}

impl fmt::Display for InfoHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl std::str::FromStr for InfoHash {
    type Err = InfoHashError;

    fn from_str(s: &str) -> Result<InfoHash, InfoHashError> {
        InfoHash::from_hex(s)
    }
}

impl From<&[u8; 20]> for InfoHash {
    fn from(value: &[u8; 20]) -> InfoHash {
        InfoHash(*value)
    }
}

impl TryFrom<&[u8]> for InfoHash {
    type Error = InfoHashError;

    fn try_from(bs: &[u8]) -> Result<InfoHash, InfoHashError> {
        match bs.try_into() {
            Ok(barray) => Ok(InfoHash(barray)),
            Err(_) => Err(InfoHashError::InvalidLength(bs.len())),
        }
    }
}

impl TryFrom<Vec<u8>> for InfoHash {
    type Error = InfoHashError;

    fn try_from(bs: Vec<u8>) -> Result<InfoHash, InfoHashError> {
        match bs.try_into() {
            Ok(barray) => Ok(InfoHash(barray)),
            Err(bs) => Err(InfoHashError::InvalidLength(bs.len())),
        }
    }
}

impl TryFromBuf for InfoHash {
    fn try_from_buf(buf: &mut Bytes) -> Result<InfoHash, PacketError> {
        if buf.len() >= InfoHash::LENGTH {
            let mut data = [0u8; InfoHash::LENGTH];
            buf.copy_to_slice(&mut data);
            Ok(InfoHash(data))
        } else {
            Err(PacketError::Short)
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum InfoHashError {
    #[error("info hash is invalid hexadecimal")]
    InvalidHex(#[source] DecodeError),
    #[error("info hash is {0} bytes long, expected 20")]
    InvalidLength(usize),
}

fn add_bytes_query_param(url: &mut Url, key: &str, value: &[u8]) {
    static SENTINEL: &str = "ADD_BYTES_QUERY_PARAM";
    url.query_pairs_mut()
        .encoding_override(Some(&|s| {
            if s == SENTINEL {
                Cow::from(value.to_vec())
            } else {
                Cow::from(s.as_bytes())
            }
        }))
        .append_pair(key, SENTINEL)
        .encoding_override(None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_info_hash() {
        let info_hash = "28C55196F57753C40ACEB6FB58617E6995A7EDDB"
            .parse::<InfoHash>()
            .unwrap();
        assert_eq!(
            info_hash.as_bytes(),
            b"\x28\xC5\x51\x96\xF5\x77\x53\xC4\x0A\xCE\xB6\xFB\x58\x61\x7E\x69\x95\xA7\xED\xDB"
        );
        assert_eq!(
            info_hash.to_string(),
            "28c55196f57753c40aceb6fb58617e6995a7eddb"
        );
    }

    #[test]
    fn test_add_query_param() {
        let info_hash = "28C55196F57753C40ACEB6FB58617E6995A7EDDB"
            .parse::<InfoHash>()
            .unwrap();
        let mut url = Url::parse("http://tracker.example.com:8080/announce?here=there").unwrap();
        info_hash.add_query_param(&mut url);
        assert_eq!(
            url.as_str(),
            "http://tracker.example.com:8080/announce?here=there&info_hash=%28%C5Q%96%F5wS%C4%0A%CE%B6%FBXa%7Ei%95%A7%ED%DB"
        );
    }
}
