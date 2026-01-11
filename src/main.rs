mod infohash;
mod tracker;
mod util;
use crate::infohash::InfoHash;
use crate::tracker::Tracker;
use anyhow::Context;
use clap::Parser;
use std::io::{self, ErrorKind, IsTerminal, Write, stderr, stdout};
use std::process::ExitCode;
use std::time::Duration;
use tracing::Level;
use tracing_subscriber::{filter::Targets, fmt::time::OffsetTime, prelude::*};

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
struct Arguments {
    #[arg(short, long, default_value_t = 30)]
    timeout: u64,

    #[arg(long)]
    trace: bool,

    tracker: Tracker,

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
                let mut first = true;
                let mut out = stdout().lock();
                for ih in hashes {
                    if !std::mem::replace(&mut first, false) {
                        writeln!(&mut out)?;
                    }
                    if let Some(s) = scrapemap.remove(&ih) {
                        writeln!(&mut out, "{ih}:")?;
                        writeln!(&mut out, "  Complete/Seeders: {}", s.complete)?;
                        writeln!(&mut out, "  Incomplete/Leechers: {}", s.incomplete)?;
                        writeln!(&mut out, "  Downloaded: {}", s.downloaded)?;
                    } else {
                        writeln!(&mut out, "{ih}: --- not tracked ---")?;
                    }
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
