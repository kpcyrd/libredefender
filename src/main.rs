#![allow(
    clippy::wildcard_imports,
    clippy::non_ascii_literal,
    clippy::missing_errors_doc
)]

use chrono::{DateTime, Local, Utc};
use chrono_humanize::HumanTime;
use colored::{Color, ColoredString, Colorize};
use env_logger::Env;
use libredefender::args::{Args, SubCommand};
use libredefender::db::Database;
use libredefender::errors::*;
use libredefender::nice;
use libredefender::notify;
use libredefender::scan;
use libredefender::schedule;
use libredefender::utils;
use num_format::{Locale, ToFormattedString};
use std::borrow::Cow;
use std::path::Path;
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
        let elapsed_since = dt.signed_duration_since(Utc::now());
        Cow::Owned(format!(
            "{} {}",
            dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S %Z"),
            format!("({})", HumanTime::from(elapsed_since)).bold()
        ))
    } else {
        Cow::Borrowed("-")
    }
}

fn print_line(line: &str, good: bool) {
    if good {
        println!(" ✅ {}", line);
    } else {
        println!(" ❌ {}", line);
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

    if args.colors {
        colored::control::set_override(true);
    }

    match args.subcommand {
        None => {
            let db = Database::load().context("Failed to load database")?;
            let data = db.data();

            print_line(
                &format!(
                    "Last scan                 {}",
                    format_datetime(&data.last_scan)
                ),
                data.last_scan.is_some(),
            );
            print_line(
                &format!(
                    "Threats present           {}",
                    format_num(data.threats.len(), false)
                ),
                data.threats.is_empty(),
            );

            print_line(
                &format!(
                    "Signatures                {}",
                    format_num(data.signature_count, true)
                ),
                data.signature_count > 0,
            );
            print_line(
                &format!(
                    "Signatures updated        {}",
                    format_datetime(&data.signatures_age)
                ),
                data.signatures_age.is_some(),
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
            schedule::run(&args)?;
        }
        Some(SubCommand::Infections(args)) => {
            let mut db = Database::load().context("Failed to load database")?;
            let data = db.data_mut();

            let mut deleted = Vec::new();

            for (path, names) in &data.threats {
                if args.delete || args.delete_all {
                    let should_delete = if args.delete_all {
                        true
                    } else {
                        utils::ask_confirmation(&format!("Delete {:?} at {:?}", names, path))?
                    };

                    if should_delete {
                        info!("Deleting {:?} at {:?}", names, path);
                        if let Err(err) = utils::ensure_deleted(path) {
                            error!("Failed to delete {:?}: {:#}", path, err);
                        } else {
                            deleted.push(path.clone());
                        }
                    }
                } else {
                    for name in names {
                        println!(
                            "{} => {}",
                            name.red().bold(),
                            format!("{:?}", path).yellow(),
                        );
                    }
                }
            }

            if !deleted.is_empty() {
                for path in deleted {
                    data.threats.remove(&path);
                }
                db.store().context("Failed to write database")?;
            }
        }
        Some(SubCommand::TestNotify) => notify::show(Path::new("/just/a/test"), "just/testing")?,
        Some(SubCommand::Completions(args)) => args.gen_completions()?,
    }

    Ok(())
}
