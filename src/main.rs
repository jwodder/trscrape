mod infohash;
mod tracker;
mod util;
use crate::infohash::InfoHash;
use crate::tracker::Tracker;
use anyhow::Context;
use clap::Parser;
use std::io::{IsTerminal, stderr};
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let Arguments {
        tracker,
        hashes,
        timeout,
        trace,
    } = Arguments::parse();
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
                for ih in hashes {
                    if !std::mem::replace(&mut first, false) {
                        println!();
                    }
                    if let Some(s) = scrapemap.remove(&ih) {
                        println!("{ih}:");
                        println!("  Complete/Seeders: {}", s.complete);
                        println!("  Incomplete/Leechers: {}", s.incomplete);
                        println!("  Downloaded: {}", s.downloaded);
                    } else {
                        println!("{ih}: --- not tracked ---");
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
