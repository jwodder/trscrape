#![expect(dead_code, unused_variables)]
mod consts;
mod infohash;
mod tracker;
mod util;
use crate::infohash::InfoHash;
use crate::tracker::Tracker;
use clap::Parser;

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
struct Arguments {
    tracker: Tracker,

    #[arg(required = true)]
    hashes: Vec<InfoHash>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let Arguments { tracker, hashes } = Arguments::parse();
    let mut scrapemap = tracker.scrape(&hashes).await?;
    for ih in hashes {
        if let Some(s) = scrapemap.remove(&ih) {
            println!("{ih}:");
            println!("  Complete/Seeders: {}", s.complete);
            println!("  Incomplete/Leechers: {}", s.incomplete);
            println!("  Downloaded: {}", s.downloaded);
            println!();
        } else {
            println!("{ih}: --- not tracked ---");
        }
    }
    Ok(())
}
