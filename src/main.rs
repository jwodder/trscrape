mod infohash;
mod tracker;
mod util;
use crate::infohash::InfoHash;
use crate::tracker::{Scrape, Tracker};
use anyhow::Context;
use clap::Parser;
use std::io::{self, ErrorKind, IsTerminal, Write, stderr, stdout};
use std::process::ExitCode;
use std::time::Duration;
use tracing::Level;
use tracing_subscriber::{filter::Targets, fmt::time::OffsetTime, prelude::*};

/// Scrape BitTorrent trackers for swarm statistics
///
/// Visit <https://github.com/jwodder/trscrape> for more information.
#[derive(Clone, Debug, Eq, Parser, PartialEq)]
struct Arguments {
    /// Output JSON lines
    #[arg(short = 'J', long)]
    json: bool,

    /// Wait at most INT seconds for the tracker to respond to our scrape
    /// request
    #[arg(short, long, default_value_t = 30, value_name = "INT")]
    timeout: u64,

    /// Emit logs of network activity
    #[arg(long)]
    trace: bool,

    /// The URL of an HTTP or UDP tracker to scrape
    tracker: Tracker,

    /// Up to 50 info hashes of torrents to scrape, given as 40-character hex
    /// strings
    #[arg(num_args = 0..=50)]
    hashes: Vec<InfoHash>,
}

fn main() -> ExitCode {
    let args = Arguments::parse();
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if let Some(ioerr) = e.downcast_ref::<io::Error>()
                && ioerr.kind() == ErrorKind::BrokenPipe
            {
                ExitCode::SUCCESS
            } else {
                let _ = writeln!(stderr().lock(), "trscrape: {e}");
                ExitCode::FAILURE
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn run(args: Arguments) -> anyhow::Result<()> {
    let Arguments {
        tracker,
        hashes,
        timeout,
        trace,
        json,
    } = args;
    if !hashes.is_empty() {
        if trace {
            let timer = OffsetTime::local_rfc_3339()
                .context("failed to determine local timezone offset")?;
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_timer(timer)
                        .with_ansi(stderr().is_terminal())
                        .with_writer(stderr),
                )
                .with(
                    Targets::new()
                        .with_target(env!("CARGO_CRATE_NAME"), Level::TRACE)
                        .with_target("reqwest", Level::TRACE)
                        .with_target("tower_http", Level::TRACE)
                        .with_default(Level::INFO),
                )
                .init();
        }
        match tokio::time::timeout(Duration::from_secs(timeout), tracker.scrape(&hashes)).await {
            Ok(Ok(mut scrapemap)) => {
                let mut printer = if json {
                    Printer::json()
                } else {
                    Printer::text()
                };
                for ih in hashes {
                    printer.print(ih, scrapemap.remove(&ih))?;
                }
                Ok(())
            }
            Ok(Err(e)) => Err(e.into()),
            Err(_) => anyhow::bail!("tracker scrape action timed out"),
        }
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Printer {
    Text { first: bool },
    Json,
}

impl Printer {
    fn text() -> Printer {
        Printer::Text { first: true }
    }

    fn json() -> Printer {
        Printer::Json
    }

    fn print(&mut self, info_hash: InfoHash, scrape: Option<Scrape>) -> io::Result<()> {
        let mut out = stdout().lock();
        match self {
            Printer::Text { first } => {
                if !std::mem::replace(first, false) {
                    writeln!(&mut out)?;
                }
                if let Some(s) = scrape {
                    writeln!(&mut out, "{info_hash}:")?;
                    writeln!(&mut out, "  Complete/Seeders: {}", s.complete)?;
                    writeln!(&mut out, "  Incomplete/Leechers: {}", s.incomplete)?;
                    writeln!(&mut out, "  Downloaded: {}", s.downloaded)?;
                } else {
                    writeln!(&mut out, "{info_hash}: --- not tracked ---")?;
                }
                Ok(())
            }
            Printer::Json => {
                if let Some(Scrape {
                    complete,
                    incomplete,
                    downloaded,
                }) = scrape
                {
                    writeln!(
                        &mut out,
                        r#"{{"info_hash": "{info_hash}", "scrape": {{"complete": {complete}, "incomplete": {incomplete}, "downloaded": {downloaded}}}}}"#
                    )
                } else {
                    writeln!(
                        &mut out,
                        r#"{{"info_hash": "{info_hash}", "scrape": null}}"#
                    )
                }
            }
        }
    }
}
