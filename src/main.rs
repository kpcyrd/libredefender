use chrono::{DateTime, Utc};
use colored::{Color, ColoredString, Colorize};
use env_logger::Env;
use libredefender::args::{Args, SubCommand};
use libredefender::config;
use libredefender::db::Database;
use libredefender::errors::*;
use libredefender::nice;
use libredefender::scan;
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
        Some(SubCommand::Scan(mut args)) => {
            nice::setup()?;

            let mut db = Database::load().context("Failed to load database")?;

            if args.paths.is_empty() {
                info!("Empty arguments, defaulting to home directory");
                let home_dir = dirs::home_dir().context("Failed to find home directory")?;
                args.paths.push(home_dir);
            }

            let config = config::load().context("Failed to load config")?;

            scan::init()?;
            let data = db.data_mut();
            data.threats.clear();
            scan::run(config, args.paths, data)?;
            data.last_scan = Some(Utc::now());

            db.store().context("Failed to write database")?;
        }
        Some(SubCommand::Scheduler(_args)) => {
            todo!()
        }
        Some(SubCommand::Completions(args)) => args.gen_completions()?,
    }

    Ok(())
}
