use crate::errors::*;
use serde::{de, Deserialize, Deserializer};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug)]
pub struct Pattern(glob::Pattern);

impl Pattern {
    #[inline]
    #[must_use]
    pub fn matches(&self, path: &Path) -> bool {
        self.0.matches_path(path)
    }
}

impl fmt::Display for Pattern {
    #[inline]
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(w)
    }
}

impl FromStr for Pattern {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pattern = glob::Pattern::from_str(s)?;
        Ok(Pattern(pattern))
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}
