pub(crate) mod http;
pub(crate) mod udp;
use self::http::*;
use self::udp::*;
use crate::infohash::InfoHash;
use crate::util::{PacketError, TryFromBuf};
use bytes::Bytes;
use std::collections::HashMap;
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Tracker {
    Http(HttpTracker),
    Udp(UdpTracker),
}

impl Tracker {
    pub(crate) async fn scrape(&self, hashes: &[InfoHash]) -> Result<ScrapeMap, TrackerError> {
        match self {
            Tracker::Http(tr) => tr.scrape(hashes).await,
            Tracker::Udp(tr) => tr.scrape(hashes).await,
        }
    }
}

impl std::str::FromStr for Tracker {
    type Err = TrackerUrlError;

    fn from_str(s: &str) -> Result<Tracker, TrackerUrlError> {
        let url = Url::parse(s)?;
        match url.scheme() {
            "http" | "https" => Ok(Tracker::Http(HttpTracker::try_from(url)?)),
            "udp" => Ok(Tracker::Udp(UdpTracker::try_from(url)?)),
            sch => Err(TrackerUrlError::UnsupportedScheme(sch.into())),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum TrackerUrlError {
    #[error("invalid tracker URL")]
    Url(#[from] url::ParseError),
    #[error("unsupported tracker URL scheme: {0:?}")]
    UnsupportedScheme(String),
    #[error("no host in tracker URL")]
    NoHost,
    #[error("no \"announce\" string in HTTP tracker URL path")]
    NoAnnounce,
    #[error("no port in UDP tracker URL")]
    NoUdpPort,
}

pub(crate) type ScrapeMap = HashMap<InfoHash, Scrape>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Scrape {
    pub(crate) complete: u32,
    pub(crate) incomplete: u32,
    pub(crate) downloaded: u32,
}

impl TryFromBuf for Scrape {
    fn try_from_buf(buf: &mut Bytes) -> Result<Self, PacketError> {
        let seeders = u32::try_from_buf(buf)?;
        let completed = u32::try_from_buf(buf)?;
        let leechers = u32::try_from_buf(buf)?;
        Ok(Scrape {
            complete: seeders,
            incomplete: leechers,
            downloaded: completed,
        })
    }
}

#[derive(Debug, Error)]
pub(crate) enum TrackerError {
    #[error("tracker replied with error message {0:?}")]
    Failure(String),
    #[error(transparent)]
    Http(#[from] HttpTrackerError),
    #[error(transparent)]
    Udp(#[from] UdpTrackerError),
}
