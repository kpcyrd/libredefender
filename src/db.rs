use crate::errors::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Database {
    path: PathBuf,
    data: Data,
}

impl Database {
    pub fn path() -> Result<PathBuf> {
        let data_dir = dirs::data_dir().context("Failed to find data directory")?;
        let path = data_dir.join("libredefender.db");
        Ok(path)
    }

    pub fn load() -> Result<Database> {
        let path = Self::path()?;
        if let Some(db) = Self::load_from(path.clone()) {
            Ok(db)
        } else {
            Ok(Database {
                path,
                data: Data::default(),
            })
        }
    }

    pub fn load_from(path: PathBuf) -> Option<Database> {
        if path.exists() {
            match Self::load_from_existing(path) {
                Ok(db) => Some(db),
                Err(err) => {
                    warn!("Failed to open existing database, using new one: {:#}", err);
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn load_from_existing(path: PathBuf) -> Result<Database> {
        let buf = fs::read(&path).context("Failed to open database")?;
        let data = serde_json::from_slice(&buf).context("Failed to read database")?;
        Ok(Database { path, data })
    }

    pub fn store(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).context("Failed to create database directory")?;
        }
        let buf = serde_json::to_vec(&self.data)?;
        fs::write(&self.path, buf).context("Failed to write database")?;
        debug!("Wrote database to {}", self.path.display());
        Ok(())
    }

    #[must_use]
    pub fn data(&self) -> &Data {
        &self.data
    }

    #[must_use]
    pub fn data_mut(&mut self) -> &mut Data {
        &mut self.data
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Data {
    pub last_scan: Option<DateTime<Utc>>,
    pub threats: HashMap<PathBuf, Vec<String>>,
    pub signature_count: usize,
    pub signatures_age: Option<DateTime<Utc>>,
}
