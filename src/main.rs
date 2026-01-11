mod infohash;
mod tracker;
mod util;
use crate::infohash::InfoHash;
use crate::tracker::Tracker;
use clap::Parser;
use std::time::Duration;

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
struct Arguments {
    #[arg(short, long, default_value_t = 30)]
    timeout: u64,

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
    } = Arguments::parse();
    if !hashes.is_empty() {
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
