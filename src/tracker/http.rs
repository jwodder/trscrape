use super::{ScrapeError, ScrapeMap, TrackerUrlError};
use crate::infohash::InfoHash;
use crate::util::{UnbencodeError, decode_bencode};
use bendy::decoding::{Error as BendyError, FromBencode, Object};
use reqwest::Client;
use std::fmt;
use thiserror::Error;
use url::Url;

static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    ")",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HttpTracker(Url);

impl HttpTracker {
    pub(crate) fn url_string(&self) -> String {
        self.0.to_string()
    }

    pub(crate) async fn scrape(&self, hashes: &[InfoHash]) -> Result<ScrapeMap, ScrapeError> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(HttpTrackerError::BuildClient)?;
        let mut url = self.0.clone();
        // TODO: Replace "announce" in path with "scrape"
        url.set_fragment(None);
        for ih in hashes {
            ih.add_query_param(&mut url);
        }
        let buf = client
            .get(url)
            .send()
            .await
            .map_err(HttpTrackerError::SendRequest)?
            .error_for_status()
            .map_err(HttpTrackerError::HttpStatus)?
            .bytes()
            .await
            .map_err(HttpTrackerError::ReadBody)?;
        decode_bencode::<HttpScrapeResponse>(&buf)
            .map_err(HttpTrackerError::ParseResponse)?
            .result()
    }
}

impl fmt::Display for HttpTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<Tracker {}>", self.0)
    }
}

impl TryFrom<Url> for HttpTracker {
    type Error = TrackerUrlError;

    fn try_from(url: Url) -> Result<HttpTracker, TrackerUrlError> {
        // TODO: Require the path to contain "announce"
        let sch = url.scheme();
        if sch != "http" && sch != "https" {
            return Err(TrackerUrlError::UnsupportedScheme(sch.into()));
        }
        if url.host().is_none() {
            return Err(TrackerUrlError::NoHost);
        }
        Ok(HttpTracker(url))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HttpScrapeResponse {
    Success(ScrapeMap),
    Failure(String),
}

impl HttpScrapeResponse {
    fn result(self) -> Result<ScrapeMap, ScrapeError> {
        match self {
            HttpScrapeResponse::Success(scrape) => Ok(scrape),
            HttpScrapeResponse::Failure(msg) => Err(ScrapeError::Failure(msg)),
        }
    }
}

impl FromBencode for HttpScrapeResponse {
    fn decode_bencode_object(object: Object<'_, '_>) -> Result<Self, BendyError> {
        todo!()
    }
}

#[derive(Debug, Error)]
pub(crate) enum HttpTrackerError {
    #[error("failed to build HTTP client")]
    BuildClient(#[source] reqwest::Error),
    #[error("failed to send request to HTTP tracker")]
    SendRequest(#[source] reqwest::Error),
    #[error("HTTP tracker responded with HTTP error")]
    HttpStatus(#[source] reqwest::Error),
    #[error("failed to read HTTP tracker response")]
    ReadBody(#[source] reqwest::Error),
    #[error("failed to parse HTTP tracker response")]
    ParseResponse(#[source] UnbencodeError),
}
