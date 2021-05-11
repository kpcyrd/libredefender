use chrono::{DateTime, Utc};
use colored::{Color, ColoredString, Colorize};
use env_logger::Env;
use libredefender::args::{Args, SubCommand};
use libredefender::db::Database;
use libredefender::errors::*;
use libredefender::nice;
use libredefender::scan;
use libredefender::schedule;
use num_format::{Locale, ToFormattedString};
use std::borrow::Cow;
use structopt::StructOpt;

fn format_num(num: usize, zero_is_bad: bool) -> ColoredString {
    let color = if zero_is_bad ^ (num != 0) {
        Color::Red
    } else {
        Color::Green
    };
    num.to_formatted_string(&Locale::en).color(color).bold()
}

fn format_datetime(dt: &Option<DateTime<Utc>>) -> Cow<'_, str> {
    if let Some(dt) = dt {
        Cow::Owned(dt.format("%Y-%m-%d %H:%M:%S %Z").to_string())
    } else {
        Cow::Borrowed("-")
    }
}

fn main() -> Result<()> {
    let args = Args::from_args();

    let logging = match (args.quiet, args.verbose) {
        (true, _) => "warn",
        (false, 0) => "info",
        (false, 1) => "info,libredefender=debug",
        (false, 2) => "debug",
        (false, _) => "debug,libredefender=trace",
    };
    env_logger::init_from_env(Env::default().default_filter_or(logging));

    match args.subcommand {
        None => {
            let db = Database::load().context("Failed to load database")?;
            let data = db.data();

            println!(
                "Last scan                 {}",
                format_datetime(&data.last_scan)
            );
            println!(
                "Threats present           {}",
                format_num(data.threats.len(), false)
            );

            println!(
                "Signatures                {}",
                format_num(data.signature_count, true)
            );
            println!(
                "Signatures updated        {}",
                format_datetime(&data.signatures_age)
            );
            println!();
            println!(
                "{}",
                "Start a scan with `libredefender scan` or run `libredefender help`".green()
            );
        }
        Some(SubCommand::Scan(args)) => {
            nice::setup()?;
            scan::init()?;
            scan::run(args)?;
        }
        Some(SubCommand::Scheduler(args)) => {
            nice::setup()?;
            scan::init()?;
            schedule::run(args)?;
        }
        Some(SubCommand::Completions(args)) => args.gen_completions()?,
    }

    Ok(())
}
