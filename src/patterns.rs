use crate::errors::*;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
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

impl Serialize for Pattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let pattern = self.0.to_string();
        serializer.serialize_str(&pattern)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_glob() {
        let txt = "foo/**/{a,b}*";
        let p = Pattern::from_str(txt).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"foo/**/{a,b}*\"");
    }
}
