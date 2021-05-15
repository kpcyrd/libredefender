use crate::errors::*;
use crate::patterns::Pattern;
use crate::schedule::PreferedHours;
use human_size::{Byte, Size, SpecificSize};
use serde::{de, Deserialize, Deserializer};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub scan: ScanConfig,
    pub update: UpdateConfig,
    #[serde(default)]
    pub schedule: ScheduleConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub excludes: Vec<Pattern>,
    #[serde(default)]
    pub skip_hidden: bool,
    pub skip_larger_than: Option<HumanSize>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfig {
    pub path: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
pub struct ScheduleConfig {
    // TODO we assume daily for now
    // pub every: Option<String>,
    // pub tolerance: Option<String>,
    pub preferred_hours: Option<PreferedHours>,
}

// config::File::new expects &str instead of &Path
fn path_to_string(path: &Path) -> Result<String> {
    let s = path.to_str().context("Path contains invalid utf-8")?;
    Ok(s.to_string())
}

pub fn load() -> Result<Config> {
    let mut settings = config::Config::default();

    settings.set_default("update.path", "/var/lib/clamav")?;

    let config_dir = dirs::config_dir().context("Failed to find config dir")?;
    let path = path_to_string(&config_dir.join("libredefender.toml"))?;

    settings
        .merge(config::File::new(&path, config::FileFormat::Toml).required(false))
        .with_context(|| anyhow!("Failed to load config file {:?}", path))?;

    let config = settings
        .try_into::<Config>()
        .context("Failed to parse config")?;

    Ok(config)
}

#[derive(Debug)]
pub struct HumanSize(SpecificSize);

impl HumanSize {
    #[must_use]
    pub fn as_bytes(&self) -> u64 {
        self.0.into::<Byte>().value() as u64
    }
}

impl FromStr for HumanSize {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let size: Size = s.parse().context("Failed to parse human size")?;
        Ok(HumanSize(size))
    }
}

impl<'de> Deserialize<'de> for HumanSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}
