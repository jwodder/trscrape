pub(crate) mod http;
pub(crate) mod udp;
use self::http::*;
use self::udp::*;
use crate::infohash::InfoHash;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Tracker {
    Http(HttpTracker),
    Udp(UdpTracker),
}

impl Tracker {
    pub(crate) fn url_string(&self) -> String {
        match self {
            Tracker::Http(tr) => tr.url_string(),
            Tracker::Udp(tr) => tr.url_string(),
        }
    }

    #[allow(clippy::unused_async)]
    pub(crate) async fn scrape(&self, hashes: Vec<InfoHash>) -> Result<ScrapeMap, ScrapeError> {
        todo!()
    }
}

impl fmt::Display for Tracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tracker::Http(http) => write!(f, "{http}"),
            Tracker::Udp(udp) => write!(f, "{udp}"),
        }
    }
}

impl FromStr for Tracker {
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
    #[error("no port in UDP tracker URL")]
    NoUdpPort,
}

pub(crate) type ScrapeMap = HashMap<InfoHash, Scrape>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Scrape {
    complete: u64,
    incomplete: u64,
    downloaded: u64,
}

#[derive(Debug, Error)]
pub(crate) enum ScrapeError {
    #[error("interactions with tracker did not complete in time")]
    Timeout,
    #[error("tracker replied with error message {0:?}")]
    Failure(String),
    #[error(transparent)]
    Http(#[from] HttpTrackerError),
    #[error(transparent)]
    Udp(#[from] UdpTrackerError),
}
