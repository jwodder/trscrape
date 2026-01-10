use bendy::decoding::{Decoder, FromBencode};
use bytes::{Buf, Bytes};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TryBytes(Bytes);

impl TryBytes {
    pub(crate) fn try_get<T: TryFromBuf>(&mut self) -> Result<T, PacketError> {
        T::try_from_buf(&mut self.0)
    }

    pub(crate) fn try_get_all<T: TryFromBuf>(mut self) -> Result<Vec<T>, PacketError> {
        let mut values = Vec::new();
        while self.0.has_remaining() {
            values.push(self.try_get()?);
        }
        Ok(values)
    }

    pub(crate) fn into_string_lossy(self) -> String {
        String::from_utf8_lossy(&self.0).into_owned()
    }
}

impl From<Bytes> for TryBytes {
    fn from(bs: Bytes) -> TryBytes {
        TryBytes(bs)
    }
}

impl From<&[u8]> for TryBytes {
    fn from(bs: &[u8]) -> TryBytes {
        TryBytes::from(Bytes::from(bs.to_vec()))
    }
}

// All integers are read in big-endian order.
pub(crate) trait TryFromBuf: Sized {
    fn try_from_buf(buf: &mut Bytes) -> Result<Self, PacketError>;
}

macro_rules! impl_tryfrombuf {
    ($t:ty, $len:literal, $arg:ident, $get:expr) => {
        impl TryFromBuf for $t {
            fn try_from_buf($arg: &mut Bytes) -> Result<Self, PacketError> {
                if $arg.remaining() >= $len {
                    Ok($get)
                } else {
                    Err(PacketError::Short)
                }
            }
        }
    };
}

impl_tryfrombuf!(u32, 4, buf, buf.get_u32());
impl_tryfrombuf!(u64, 8, buf, buf.get_u64());

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum PacketError {
    #[error("unexpected end of packet")]
    Short,
}

// Like `FromBencode::from_bencode()`, but it checks that there are no trailing
// bytes afterwards
pub(crate) fn decode_bencode<T: FromBencode>(buf: &[u8]) -> Result<T, UnbencodeError> {
    let mut decoder = Decoder::new(buf).with_max_depth(T::EXPECTED_RECURSION_DEPTH);
    let value = match decoder.next_object()? {
        Some(obj) => T::decode_bencode_object(obj)?,
        None => return Err(UnbencodeError::NoData),
    };
    if !matches!(decoder.next_object(), Ok(None)) {
        return Err(UnbencodeError::TrailingData);
    }
    Ok(value)
}

#[derive(Clone, Debug, Error)]
pub(crate) enum UnbencodeError {
    #[error(transparent)]
    Bendy(#[from] bendy::decoding::Error),
    #[error("no data in bencode packet")]
    NoData,
    #[error("trailing bytes after bencode structure")]
    TrailingData,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_get_u32() {
        let mut buf = TryBytes::from(b"0123abc".as_slice());
        assert_eq!(buf.try_get::<u32>(), Ok(0x30313233));
        assert_eq!(buf.try_get::<u32>(), Err(PacketError::Short));
    }

    #[test]
    fn test_try_get_u64() {
        let mut buf = TryBytes::from(b"01234567abcde".as_slice());
        assert_eq!(buf.try_get::<u64>(), Ok(0x3031323334353637));
        assert_eq!(buf.try_get::<u64>(), Err(PacketError::Short));
    }
}
