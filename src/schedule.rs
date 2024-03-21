use crate::args;
use crate::config;
use crate::db::Database;
use crate::errors::*;
use crate::scan;
use chrono::{DateTime, Datelike, Local, NaiveTime, TimeZone, Timelike, Utc};
use rand::Rng;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use starship_battery as battery;
use std::cmp;
use std::str::FromStr;
use std::thread;

#[derive(Debug, PartialEq, Eq)]
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
            let next_start = Local
                .with_ymd_and_hms(
                    dt.year(),
                    dt.month(),
                    dt.day(),
                    self.start.hour(),
                    self.start.minute(),
                    self.start.second(),
                )
                .earliest()
                .unwrap();
            next_start - dt
        } else {
            // tomorrow
            let next_start = Local
                .with_ymd_and_hms(
                    dt.year(),
                    dt.month(),
                    dt.day(),
                    self.start.hour(),
                    self.start.minute(),
                    self.start.second(),
                )
                .earliest()
                .unwrap()
                + chrono::Duration::try_hours(24).unwrap();
            next_start - dt
        }
    }

    fn until_next_end(&self, dt: DateTime<Local>) -> chrono::Duration {
        let t = dt.time();
        if self.end > t {
            // today
            let next_end = Local
                .with_ymd_and_hms(
                    dt.year(),
                    dt.month(),
                    dt.day(),
                    self.end.hour(),
                    self.end.minute(),
                    self.end.second(),
                )
                .earliest()
                .unwrap();
            next_end - dt
        } else {
            // tomorrow
            let next_end = Local
                .with_ymd_and_hms(
                    dt.year(),
                    dt.month(),
                    dt.day(),
                    self.end.hour(),
                    self.end.minute(),
                    self.end.second(),
                )
                .earliest()
                .unwrap()
                + chrono::Duration::try_hours(24).unwrap();
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

impl Serialize for PreferedHours {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hours = format!("{}-{}", self.start, self.end);
        serializer.serialize_str(&hours)
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

        let next_sleep = cmp::min(chrono::Duration::try_seconds(600).unwrap(), remaining);
        trace!("Sleeping for {:?}", next_sleep);

        thread::sleep(next_sleep.to_std()?);
    }

    Ok(())
}

pub fn run(_args: &args::Scheduler) -> Result<()> {
    let interval = chrono::Duration::try_hours(24).unwrap();

    loop {
        let now = Local::now();

        let config = match config::load(None) {
            Ok(config) => config,
            Err(err) => {
                warn!("Failed to load config, skipping this scan: {:#}", err);
                robust_sleep(interval)?;
                continue;
            }
        };

        if config.schedule.skip_on_battery {
            let battery_manager = battery::Manager::new()?;

            let batteries = battery_manager
                .batteries()
                .context("Failed to detect batteries")?
                .collect::<battery::Result<Vec<_>>>()
                .context("Failed to read battery status")?;

            // Check if there even are batteries in the system. If we don't
            // find any batteries we assume that the system has no batteries
            // and we start a scan.
            if batteries.is_empty() {
                debug!("No batteries present in system");
            } else {
                // List all batteries and check if any are in state Discharging
                let battery_discharging = batteries.iter().fold(false, |discharging, battery| {
                    let state = battery.state();
                    debug!(
                        "Found battery: {} {}, {:?}% ({:?})",
                        battery.vendor().unwrap_or("-"),
                        battery.model().unwrap_or("-"),
                        battery.state_of_charge() * 100.0,
                        state,
                    );
                    discharging || state == battery::State::Discharging
                });

                if battery_discharging {
                    info!("Battery is discharging, skipping this scan");
                    robust_sleep(interval)?;
                    continue;
                }
            }
        }

        match config.schedule.automatic_scans.as_deref() {
            Some("off") => {
                info!("Automatic scanning is disabled, skipping this scan");
                robust_sleep(interval)?;
                continue;
            }
            Some("daily") | None => (),
            value => {
                error!(
                    "Invalid value for automatic_scans, skipping this scan: {:?}",
                    value
                );
                robust_sleep(interval)?;
                continue;
            }
        }

        let db = match Database::load() {
            Ok(db) => db,
            Err(err) => {
                error!("Failed to load database: {:#}", err);
                robust_sleep(interval)?;
                continue;
            }
        };
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

                            start + chrono::Duration::try_seconds(jitter).unwrap()
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
                start: NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
                end: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
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
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 13, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::try_seconds(5 * 3600 + 23 * 60).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_end() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 13, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::try_seconds(19 * 3600 + 23 * 60).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_start_now() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 23, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::try_seconds(0).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_end_now() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 23, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("19:00:00-09:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::try_seconds(9 * 3600 + 23 * 60).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_start_now2() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 13, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("09:00:00-19:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::try_seconds(0).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_end_now2() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 13, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("09:00:00-19:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::try_seconds(5 * 3600 + 23 * 60).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_start_later() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 9, 0, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("13:37:00-23:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::try_seconds(4 * 3600 + 37 * 60).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_end_later() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 9, 0, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("13:37:00-23:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::try_seconds(14 * 3600).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_start_tomorrow() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 13, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("4:00:00-9:00:00").unwrap();
        let duration = ph.until_next_start(now);
        assert_eq!(duration, chrono::Duration::try_seconds(14 * 3600 + 23 * 60).unwrap());
    }

    #[test]
    fn test_until_next_preferred_hour_end_tomorrow() {
        let now = Local
            .with_ymd_and_hms(1970, 1, 1, 13, 37, 0)
            .single()
            .unwrap();
        let ph = PreferedHours::from_str("4:00:00-9:00:00").unwrap();
        let duration = ph.until_next_end(now);
        assert_eq!(duration, chrono::Duration::try_seconds(19 * 3600 + 23 * 60).unwrap());
    }

    #[test]
    fn test_serialize_preferred_hours() {
        let txt = "13:37:00-23:00:00";
        let p = PreferedHours::from_str(txt).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"13:37:00-23:00:00\"");
    }
}
