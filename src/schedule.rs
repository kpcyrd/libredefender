use crate::args;
use crate::config;
use crate::db::Database;
use crate::errors::*;
use crate::scan;
use chrono::{DateTime, Datelike, Local, NaiveTime, TimeZone, Timelike, Utc};
use rand::Rng;
use serde::{de, Deserialize, Deserializer};
use std::cmp;
use std::str::FromStr;
use std::thread;

#[derive(Debug, PartialEq)]
pub struct PreferedHours {
    start: NaiveTime,
    end: NaiveTime,
}

impl PreferedHours {
    fn until_next_start(&self, dt: DateTime<Local>) -> chrono::Duration {
        let t = dt.time();
        if self.start <= t && (self.end > t || self.end < self.start) {
            // now
            chrono::Duration::zero()
        } else if t < self.start {
            // today
            let next_start = Local.ymd(dt.year(), dt.month(), dt.day()).and_hms(
                self.start.hour(),
                self.start.minute(),
                self.start.second(),
            );
            next_start - dt
        } else {
            // tomorrow
            let next_start = Local.ymd(dt.year(), dt.month(), dt.day()).and_hms(
                self.start.hour(),
                self.start.minute(),
                self.start.second(),
            ) + chrono::Duration::hours(24);
            next_start - dt
        }
    }

    fn until_next_end(&self, dt: DateTime<Local>) -> chrono::Duration {
        let t = dt.time();
        if self.end > t {
            // today
            let next_end = Local.ymd(dt.year(), dt.month(), dt.day()).and_hms(
                self.end.hour(),
                self.end.minute(),
                self.end.second(),
            );
            next_end - dt
        } else {
            // tomorrow
            let next_end = Local.ymd(dt.year(), dt.month(), dt.day()).and_hms(
                self.end.hour(),
                self.end.minute(),
                self.end.second(),
            ) + chrono::Duration::hours(24);
            next_end - dt
        }
    }
}

impl FromStr for PreferedHours {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split('-').collect::<Vec<_>>();
        if parts.len() != 2 {
            bail!("Unexpected number of arguments");
        }

        let start = parts[0].parse().context("Not a number")?;
        let end = parts[1].parse().context("Not a number")?;

        Ok(PreferedHours { start, end })
    }
}

impl<'de> Deserialize<'de> for PreferedHours {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

fn robust_sleep(sleep: chrono::Duration) -> Result<()> {
    let target_time = Utc::now() + sleep;

    let duration_seconds = sleep.num_seconds() as u64;
    let hours = duration_seconds / 60 / 60;
    let minutes = (duration_seconds / 60) % 60;
    let seconds = duration_seconds % 60;

    info!(
        "Sleeping for {}h {}m {}s ({})...",
        hours,
        minutes,
        seconds,
        target_time
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S %Z")
    );

    loop {
        let remaining = target_time.signed_duration_since(Utc::now());
        trace!("Remaining time: {:?}", remaining);
        if remaining <= chrono::Duration::zero() {
            break;
        }

        let next_sleep = cmp::min(chrono::Duration::seconds(600), remaining);
        trace!("Sleeping for {:?}", next_sleep);

        thread::sleep(next_sleep.to_std()?);
    }

    Ok(())
}

pub fn run(_args: &args::Scheduler) -> Result<()> {
    let interval = chrono::Duration::hours(24);

    loop {
        let now = Local::now();

        let config = config::load(None).context("Failed to load config")?;

        let db = Database::load().context("Failed to load database")?;
        let data = db.data();

        let sleep = data
            .last_scan
            .map_or_else(chrono::Duration::zero, |last_scan| {
                let duration_since_last_scan = now - last_scan.with_timezone(&Local);

                if duration_since_last_scan > interval {
                    chrono::Duration::zero()
                } else {
                    config.schedule.preferred_hours.map_or_else(
                        // no preferred hours
                        || interval - (now - last_scan.with_timezone(&Local)),
                        // there are preferred hours
                        |ph| {
                            let start = ph.until_next_start(now);
                            let end = ph.until_next_end(now);

                            let mut rng = rand::thread_rng();
                            let preferred_hours_duration = (end - start).num_seconds();
                            let jitter = rng.gen_range(0..preferred_hours_duration);

                            start + chrono::Duration::seconds(jitter)
                        },
                    )
                }
            });

        robust_sleep(sleep)?;

        if let Err(err) = scan::run(args::Scan::default()) {
            error!("Error: {:#}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_preferred_hours() {
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        assert_eq!(
            ph,
            PreferedHours {
                start: NaiveTime::from_hms(19, 0, 0),
                end: NaiveTime::from_hms(9, 0, 0),
            }
        );
    }

    #[test]
    fn test_parse_preferred_hours_invalid() {
        PreferedHours::from_str("a").err().unwrap();
        PreferedHours::from_str("a-").err().unwrap();
        PreferedHours::from_str("a--").err().unwrap();
        PreferedHours::from_str("a-b").err().unwrap();
        PreferedHours::from_str("1-b").err().unwrap();
        PreferedHours::from_str("a-2").err().unwrap();
        PreferedHours::from_str("1-2-").err().unwrap();
        PreferedHours::from_str("1-2b").err().unwrap();
        PreferedHours::from_str("1-2").err().unwrap();
        PreferedHours::from_str("1:-2:").err().unwrap();
    }

    #[test]
    fn test_until_next_preferred_hour_start() {
        let now = Local.ymd(1970, 1, 1).and_hms(13, 37, 0);
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::seconds(5 * 3600 + 23 * 60));
    }

    #[test]
    fn test_until_next_preferred_hour_end() {
        let now = Local.ymd(1970, 1, 1).and_hms(13, 37, 0);
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::seconds(19 * 3600 + 23 * 60));
    }

    #[test]
    fn test_until_next_preferred_hour_start_now() {
        let now = Local.ymd(1970, 1, 1).and_hms(23, 37, 0);
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::seconds(0));
    }

    #[test]
    fn test_until_next_preferred_hour_end_now() {
        let now = Local.ymd(1970, 1, 1).and_hms(23, 37, 0);
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::seconds(9 * 3600 + 23 * 60));
    }

    #[test]
    fn test_until_next_preferred_hour_start_now2() {
        let now = Local.ymd(1970, 1, 1).and_hms(13, 37, 0);
        let ph = PreferedHours::from_str("09:00:00-19:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::seconds(0));
    }

    #[test]
    fn test_until_next_preferred_hour_end_now2() {
        let now = Local.ymd(1970, 1, 1).and_hms(13, 37, 0);
        let ph = PreferedHours::from_str("09:00:00-19:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::seconds(5 * 3600 + 23 * 60));
    }

    #[test]
    fn test_until_next_preferred_hour_start_later() {
        let now = Local.ymd(1970, 1, 1).and_hms(9, 0, 0);
        let ph = PreferedHours::from_str("13:37:00-23:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::seconds(4 * 3600 + 37 * 60));
    }

    #[test]
    fn test_until_next_preferred_hour_end_later() {
        let now = Local.ymd(1970, 1, 1).and_hms(9, 0, 0);
        let ph = PreferedHours::from_str("13:37:00-23:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::seconds(14 * 3600));
    }

    #[test]
    fn test_until_next_preferred_hour_start_tomorrow() {
        let now = Local.ymd(1970, 1, 1).and_hms(13, 37, 0);
        let ph = PreferedHours::from_str("4:00:00-9:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::seconds(14 * 3600 + 23 * 60));
    }

    #[test]
    fn test_until_next_preferred_hour_end_tomorrow() {
        let now = Local.ymd(1970, 1, 1).and_hms(13, 37, 0);
        let ph = PreferedHours::from_str("4:00:00-9:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::seconds(19 * 3600 + 23 * 60));
    }
}
