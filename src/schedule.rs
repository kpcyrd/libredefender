use crate::args;
use crate::errors::*;
use crate::scan;
use std::thread;
use std::time::Duration;

pub fn run(_args: args::Scheduler) -> Result<()> {
    loop {
        info!("Sleeping...");
        thread::sleep(Duration::from_secs(60));
        if let Err(err) = scan::run(args::Scan { paths: vec![] }) {
            error!("Error: {:#}", err);
        }
    }
}
