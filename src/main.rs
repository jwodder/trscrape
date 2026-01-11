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

    #[arg(num_args = 0..=50)]
    hashes: Vec<InfoHash>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let Arguments { tracker, hashes } = Arguments::parse();
    if !hashes.is_empty() {
        let mut scrapemap = tracker.scrape(&hashes).await?;
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
    }
    Ok(())
}
