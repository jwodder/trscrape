#![expect(unreachable_code)]
use super::{ScrapeError, ScrapeMap, TrackerUrlError};
use crate::consts::UDP_PACKET_LEN;
use crate::infohash::InfoHash;
use crate::util::{PacketError, TryBytes};
use bytes::{BufMut, Bytes, BytesMut};
use rand::random;
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;
use thiserror::Error;
use tokio::net::{UdpSocket, lookup_host};
use tokio::time::{Instant, timeout, timeout_at};
use url::Url;

const PROTOCOL_ID: u64 = 0x41727101980;
const CONNECT_ACTION: u32 = 0;
const ANNOUNCE_ACTION: u32 = 1;
const ERROR_ACTION: u32 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UdpTracker(UdpUrl);

impl UdpTracker {
    pub(crate) fn url_string(&self) -> String {
        self.0.to_string()
    }

    pub(crate) async fn scrape(&self, hashes: Vec<InfoHash>) -> Result<ScrapeMap, ScrapeError> {
        let socket = ConnectedUdpSocket::connect(&self.0.host, self.0.port).await?;
        let mut session = UdpTrackerSession::new(self, socket);
        session.scrape(hashes).await
    }
}

impl fmt::Display for UdpTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<Tracker {}>", self.0)
    }
}

impl TryFrom<Url> for UdpTracker {
    type Error = TrackerUrlError;

    fn try_from(url: Url) -> Result<UdpTracker, TrackerUrlError> {
        UdpUrl::try_from(url).map(UdpTracker)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UdpUrl {
    host: String,
    port: u16,
    urldata: String,
}

impl fmt::Display for UdpUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "udp://")?;
        if self.host.contains(':') {
            write!(f, "[{}]", self.host)?;
        } else {
            write!(f, "{}", self.host)?;
        }
        write!(f, ":{}{}", self.port, self.urldata)?;
        Ok(())
    }
}

impl TryFrom<Url> for UdpUrl {
    type Error = TrackerUrlError;

    fn try_from(url: Url) -> Result<UdpUrl, TrackerUrlError> {
        let sch = url.scheme();
        if sch != "udp" {
            return Err(TrackerUrlError::UnsupportedScheme(sch.into()));
        }
        let Some(host) = url.host_str().map(ToOwned::to_owned) else {
            return Err(TrackerUrlError::NoHost);
        };
        let Some(port) = url.port() else {
            return Err(TrackerUrlError::NoUdpPort);
        };
        let mut urldata = String::from(url.path());
        if let Some(query) = url.query() {
            urldata.push('?');
            urldata.push_str(query);
        }
        Ok(UdpUrl {
            host,
            port,
            urldata,
        })
    }
}

pub(super) struct UdpTrackerSession {
    pub(super) tracker: UdpTracker,
    socket: ConnectedUdpSocket,
    conn: Option<Connection>,
}

impl UdpTrackerSession {
    fn new(tracker: &UdpTracker, socket: ConnectedUdpSocket) -> Self {
        UdpTrackerSession {
            tracker: tracker.clone(),
            socket,
            conn: None,
        }
    }

    pub(super) async fn scrape(&mut self, hashes: Vec<InfoHash>) -> Result<ScrapeMap, ScrapeError> {
        loop {
            let conn = self.get_connection().await?;
            let transaction_id = self.make_transaction_id();
            let msg = todo!();
            /*
            let msg = Bytes::from(UdpAnnounceRequest {
                connection_id: conn.id,
                transaction_id,
                announcement: announcement.clone(),
                urldata: self.tracker.0.urldata.clone(),
            });
            */
            let resp = match timeout_at(conn.expiration, self.chat(msg)).await {
                Ok(Ok(buf)) => Response::<UdpScrapeResponse>::from_bytes(buf, |buf| {
                    UdpScrapeResponse::from_bytes(buf, self.socket.ipv6)
                })?
                .ok()?,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => {
                    log::trace!("Connection to {} timed out; restarting", self.tracker);
                    self.reset_connection();
                    continue;
                }
            };
            if resp.transaction_id != transaction_id {
                return Err(UdpTrackerError::XactionMismatch {
                    expected: transaction_id,
                    got: resp.transaction_id,
                }
                .into());
            }
            return Ok(resp.response);
        }
    }

    async fn get_connection(&mut self) -> Result<Connection, ScrapeError> {
        if let Some(c) = self.conn {
            if Instant::now() < c.expiration {
                return Ok(c);
            } else {
                log::trace!("Connection to {} expired; will reconnect", self.tracker);
            }
        }
        let conn = self.connect().await?;
        self.conn = Some(conn);
        Ok(conn)
    }

    fn reset_connection(&mut self) {
        self.conn = None;
    }

    async fn connect(&self) -> Result<Connection, ScrapeError> {
        log::trace!("Sending connection request to {}", self.tracker);
        let transaction_id = self.make_transaction_id();
        let msg = Bytes::from(UdpConnectionRequest { transaction_id });
        let raw_resp = self.chat(msg).await?;
        // TODO: Should communication be retried on parse errors and mismatched
        // transaction IDs?
        let resp = Response::<UdpConnectionResponse>::from_bytes(raw_resp, |buf| {
            UdpConnectionResponse::try_from(buf)
        })?
        .ok()?;
        if resp.transaction_id != transaction_id {
            return Err(UdpTrackerError::XactionMismatch {
                expected: transaction_id,
                got: resp.transaction_id,
            }
            .into());
        }
        log::trace!("Connected to {}", self.tracker);
        let expiration = Instant::now() + Duration::from_secs(60);
        Ok(Connection {
            id: resp.connection_id,
            expiration,
        })
    }

    async fn chat(&self, msg: Bytes) -> Result<Bytes, UdpTrackerError> {
        let mut n = 0;
        loop {
            self.socket.send(&msg).await?;
            let maxtime = Duration::from_secs(15 << n);
            if let Ok(r) = timeout(maxtime, self.socket.recv()).await {
                return r;
            } else {
                log::trace!("{} did not reply in time; resending message", self.tracker);
                if n < 8 {
                    // TODO: Should this count remember timeouts from previous
                    // connections & connection attempts?
                    n += 1;
                }
                continue;
            }
        }
    }

    fn make_transaction_id(&self) -> u32 {
        random()
    }
}

struct ConnectedUdpSocket {
    inner: UdpSocket,
    ipv6: bool,
}

impl ConnectedUdpSocket {
    async fn connect(host: &str, port: u16) -> Result<ConnectedUdpSocket, UdpTrackerError> {
        let Some(addr) = lookup_host((host, port))
            .await
            .map_err(UdpTrackerError::Lookup)?
            .next()
        else {
            return Err(UdpTrackerError::NoResolve);
        };
        let (bindaddr, ipv6) = match addr {
            SocketAddr::V4(_) => ("0.0.0.0:0", false),
            SocketAddr::V6(_) => ("[::]:0", true),
        };
        let socket = UdpSocket::bind(bindaddr)
            .await
            .map_err(UdpTrackerError::Bind)?;
        log::trace!(
            "Connected UDP socket to {} (IP address: {}), port {}",
            host,
            addr.ip(),
            port,
        );
        socket
            .connect(addr)
            .await
            .map_err(UdpTrackerError::Connect)?;
        Ok(ConnectedUdpSocket {
            inner: socket,
            ipv6,
        })
    }

    async fn send(&self, msg: &Bytes) -> Result<(), UdpTrackerError> {
        self.inner.send(msg).await.map_err(UdpTrackerError::Send)?;
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes, UdpTrackerError> {
        let mut buf = BytesMut::with_capacity(UDP_PACKET_LEN);
        self.inner
            .recv_buf(&mut buf)
            .await
            .map_err(UdpTrackerError::Recv)?;
        Ok(buf.freeze())
    }
}

// UDP tracker psuedo-connection (BEP 15)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Connection {
    id: u64,
    expiration: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Response<T> {
    Success(T),
    Failure(String),
}

impl<T> Response<T> {
    fn ok(self) -> Result<T, ScrapeError> {
        match self {
            Response::Success(res) => Ok(res),
            Response::Failure(msg) => Err(ScrapeError::Failure(msg)),
        }
    }

    fn from_bytes<F>(buf: Bytes, parser: F) -> Result<Self, UdpTrackerError>
    where
        F: FnOnce(Bytes) -> Result<T, UdpTrackerError>,
    {
        let mut view = TryBytes::from(buf.slice(0..));
        if view.try_get::<u32>() == Ok(ERROR_ACTION) {
            let _transaction_id = view.try_get::<u32>()?;
            // TODO: Should we bother to check the transaction ID?
            let message = view.into_string_lossy();
            Ok(Response::Failure(message))
        } else {
            parser(buf).map(Response::Success)
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct UdpConnectionRequest {
    transaction_id: u32,
}

impl From<UdpConnectionRequest> for Bytes {
    fn from(req: UdpConnectionRequest) -> Bytes {
        let mut buf = BytesMut::with_capacity(16);
        buf.put_u64(PROTOCOL_ID);
        buf.put_u32(CONNECT_ACTION);
        buf.put_u32(req.transaction_id);
        buf.freeze()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct UdpConnectionResponse {
    transaction_id: u32,
    connection_id: u64,
}

impl TryFrom<Bytes> for UdpConnectionResponse {
    type Error = UdpTrackerError;

    fn try_from(buf: Bytes) -> Result<Self, UdpTrackerError> {
        let mut buf = TryBytes::from(buf);
        let action = buf.try_get::<u32>()?;
        if action != CONNECT_ACTION {
            return Err(UdpTrackerError::BadAction {
                expected: CONNECT_ACTION,
                got: action,
            });
        }
        let transaction_id = buf.try_get::<u32>()?;
        let connection_id = buf.try_get::<u64>()?;
        // Don't require EOF here, as "Clients ... should not assume packets to
        // be of a certain size"
        Ok(UdpConnectionResponse {
            transaction_id,
            connection_id,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UdpScrapeRequest; // TODO

impl From<UdpScrapeRequest> for Bytes {
    fn from(req: UdpScrapeRequest) -> Bytes {
        todo!()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UdpScrapeResponse {
    transaction_id: u32,
    response: ScrapeMap,
}

impl UdpScrapeResponse {
    fn from_bytes(buf: Bytes, ipv6: bool) -> Result<Self, UdpTrackerError> {
        todo!()
    }
}

#[derive(Debug, Error)]
pub(crate) enum UdpTrackerError {
    #[error("failed to resolve remote hostname")]
    Lookup(#[source] std::io::Error),
    #[error("remote hostname did not resolve to any IP addresses")]
    NoResolve,
    #[error("failed to bind UDP socket")]
    Bind(#[source] std::io::Error),
    #[error("failed to connect UDP socket")]
    Connect(#[source] std::io::Error),
    #[error("failed to send UDP packet")]
    Send(#[source] std::io::Error),
    #[error("failed to receive UDP packet")]
    Recv(#[source] std::io::Error),
    #[error("UDP tracker sent response with invalid length")]
    PacketLen(#[from] PacketError),
    #[error(
        "UDP tracker sent response with unexpected or unsupported action; expected {expected}, got {got}"
    )]
    BadAction { expected: u32, got: u32 },
    #[error(
        "response from UDP tracker did not contain expected transaction ID; expected {expected:#x}, got {got:#x}"
    )]
    XactionMismatch { expected: u32, got: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_connection_request() {
        let req = UdpConnectionRequest {
            transaction_id: 0x5C310D73,
        };
        let buf = Bytes::from(req);
        assert_eq!(
            buf,
            b"\x00\x00\x04\x17'\x10\x19\x80\x00\x00\x00\x00\\1\rs".as_slice()
        );
    }

    #[test]
    fn test_parse_connection_response() {
        let buf = Bytes::from(b"\x00\x00\x00\x00\\1\rs\\\xcb\xdf\xdb\x15|%\xba".as_slice());
        let res = UdpConnectionResponse::try_from(buf).unwrap();
        assert_eq!(res.transaction_id, 0x5C310D73);
        assert_eq!(res.connection_id, 0x5CCBDFDB157C25BA);
    }

    #[test]
    fn test_udp_url_from_url() {
        let url = "udp://tracker.opentrackr.org:1337/announce"
            .parse::<Url>()
            .unwrap();
        let uu = UdpUrl::try_from(url).unwrap();
        assert_eq!(
            uu,
            UdpUrl {
                host: "tracker.opentrackr.org".into(),
                port: 1337,
                urldata: "/announce".into(),
            }
        );
        assert_eq!(uu.to_string(), "udp://tracker.opentrackr.org:1337/announce");
    }

    #[test]
    fn test_udp_url_from_url_no_urldata() {
        let url = "udp://tracker.opentrackr.org:1337".parse::<Url>().unwrap();
        let uu = UdpUrl::try_from(url).unwrap();
        assert_eq!(
            uu,
            UdpUrl {
                host: "tracker.opentrackr.org".into(),
                port: 1337,
                urldata: String::new(),
            }
        );
        assert_eq!(uu.to_string(), "udp://tracker.opentrackr.org:1337");
    }
}
