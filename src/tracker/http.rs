use super::{Scrape, ScrapeMap, TrackerError, TrackerUrlError};
use crate::infohash::InfoHash;
use crate::util::{UnbencodeError, decode_bencode};
use bendy::decoding::{Error as BendyError, FromBencode, Object, ResultExt};
use reqwest::Client;
use std::collections::HashMap;
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
    #[tracing::instrument(name = "scrape-http", skip_all, fields(tracker = %self.0))]
    pub(crate) async fn scrape(&self, hashes: &[InfoHash]) -> Result<ScrapeMap, TrackerError> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(HttpTrackerError::BuildClient)?;
        let mut url = self.0.clone();
        url.set_path(&url.path().replace("announce", "scrape"));
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

impl TryFrom<Url> for HttpTracker {
    type Error = TrackerUrlError;

    fn try_from(url: Url) -> Result<HttpTracker, TrackerUrlError> {
        let sch = url.scheme();
        if sch != "http" && sch != "https" {
            return Err(TrackerUrlError::UnsupportedScheme(sch.into()));
        }
        if url.host().is_none() {
            return Err(TrackerUrlError::NoHost);
        }
        if !url.path().contains("announce") {
            return Err(TrackerUrlError::NoAnnounce);
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
    fn result(self) -> Result<ScrapeMap, TrackerError> {
        match self {
            HttpScrapeResponse::Success(scrape) => Ok(scrape),
            HttpScrapeResponse::Failure(msg) => Err(TrackerError::Failure(msg)),
        }
    }
}

impl FromBencode for HttpScrapeResponse {
    fn decode_bencode_object(object: Object<'_, '_>) -> Result<Self, BendyError> {
        let mut files = None;
        let mut failure_reason = None;
        let mut dd = object.try_into_dictionary()?;
        while let Some(kv) = dd.next_pair()? {
            match kv {
                (b"files", val) => {
                    let mut filemap = HashMap::new();
                    let mut fdict = val.try_into_dictionary().context("files")?;
                    while let Some((k, v)) = fdict.next_pair().context("files")? {
                        let infohash = InfoHash::try_from(k)
                            .map_err(|e| BendyError::malformed_content(e).context("files.<key>"))?;
                        let mut complete = None;
                        let mut downloaded = None;
                        let mut incomplete = None;
                        let mut vdict = v.try_into_dictionary().context("files.<value>")?;
                        while let Some(kv) = vdict.next_pair().context("files.<value>")? {
                            match kv {
                                (b"complete", val) => {
                                    complete = Some(
                                        u32::decode_bencode_object(val)
                                            .context("files.*.complete")?,
                                    );
                                }
                                (b"downloaded", val) => {
                                    downloaded = Some(
                                        u32::decode_bencode_object(val)
                                            .context("files.*.downloaded")?,
                                    );
                                }
                                (b"incomplete", val) => {
                                    incomplete = Some(
                                        u32::decode_bencode_object(val)
                                            .context("files.*.incomplete")?,
                                    );
                                }
                                _ => (),
                            }
                        }
                        let complete = complete
                            .ok_or_else(|| BendyError::missing_field("files.*.complete"))?;
                        let downloaded = downloaded
                            .ok_or_else(|| BendyError::missing_field("files.*.downloaded"))?;
                        let incomplete = incomplete
                            .ok_or_else(|| BendyError::missing_field("files.*.incomplete"))?;
                        filemap.insert(
                            infohash,
                            Scrape {
                                complete,
                                incomplete,
                                downloaded,
                            },
                        );
                    }
                    files = Some(filemap);
                }
                (b"failure reason", val) => {
                    failure_reason = Some(
                        String::from_utf8_lossy(val.try_into_bytes().context("failure reason")?)
                            .into_owned(),
                    );
                }
                _ => (),
            }
        }
        match (files, failure_reason) {
            (Some(files), None) => Ok(HttpScrapeResponse::Success(files)),
            (_, Some(fr)) => Ok(HttpScrapeResponse::Failure(fr)),
            (None, None) => Err(BendyError::missing_field("files")),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BufMut, BytesMut};

    #[test]
    fn parse_scrape_response() {
        let mut buf = BytesMut::new();
        buf.put(b"d5:filesd".as_slice());
        buf.put(
            b"20:\x28\xc5\x51\x96\xf5\x77\x53\xc4\x0a\xce\xb6\xfb\x58\x61\x7e\x69\x95\xa7\xed\xdb"
                .as_slice(),
        );
        buf.put(b"d8:completei10e10:downloadedi32e10:incompletei0ee".as_slice());
        buf.put(
            b"20:\xda\x39\xa3\xee\x5e\x6b\x4b\x0d\x32\x55\xbf\xef\x95\x60\x18\x90\xaf\xd8\x07\x09"
                .as_slice(),
        );
        buf.put(b"d8:completei105e10:downloadedi1337e10:incompletei42ee".as_slice());
        buf.put(b"ee".as_slice());
        let res = decode_bencode::<HttpScrapeResponse>(&buf)
            .unwrap()
            .result()
            .unwrap();
        assert_eq!(
            res,
            HashMap::from([
                (
                    "28c55196f57753c40aceb6fb58617e6995a7eddb"
                        .parse::<InfoHash>()
                        .unwrap(),
                    Scrape {
                        complete: 10,
                        incomplete: 0,
                        downloaded: 32
                    },
                ),
                (
                    "da39a3ee5e6b4b0d3255bfef95601890afd80709"
                        .parse::<InfoHash>()
                        .unwrap(),
                    Scrape {
                        complete: 105,
                        incomplete: 42,
                        downloaded: 1337
                    },
                ),
            ])
        );
    }

    #[test]
    fn parse_scrape_failure_response() {
        let mut buf = BytesMut::new();
        buf.put(b"d14:failure reason11:Out of bitse".as_slice());
        let res = decode_bencode::<HttpScrapeResponse>(&buf).unwrap();
        assert_eq!(
            res,
            HttpScrapeResponse::Failure(String::from("Out of bits"))
        );
    }
}
