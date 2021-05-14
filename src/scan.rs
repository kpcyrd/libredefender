use crate::args;
use crate::config::{self, ScanConfig};
use crate::db::Database;
use crate::errors::*;
use chrono::TimeZone;
use chrono::{DateTime, Utc};
use clamav_rs::engine::{Engine, ScanResult};
use clamav_rs::scan_settings::ScanSettings;
use crossbeam_channel::Sender;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use walkdir::{DirEntry, WalkDir};

pub fn init() -> Result<()> {
    info!("Initializing with libclamav {}", clamav_rs::version());
    clamav_rs::initialize().map_err(|e| anyhow!("Failed to init clamav: {:#}", e))?;
    Ok(())
}

// clamav_rs::engine::Engine::scan_file expects &str instead of &Path
fn path_to_string(path: &Path) -> Result<String> {
    let s = path.to_str().context("Path contains invalid utf-8")?;
    Ok(s.to_string())
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.'))
}

#[must_use]
pub fn matches(config: &ScanConfig, e: &DirEntry) -> bool {
    let path = e.path();

    if config.skip_hidden && is_hidden(e) {
        debug!("Skipping path {}: name starts with dot", path.display());
        return false;
    }

    for exclude in &config.excludes {
        if exclude.matches(e.path()) {
            debug!(
                "Skipping path {}: matches exclude ({})",
                path.display(),
                exclude
            );
            return false;
        }
    }

    if let Some(skip_larger_than) = &config.skip_larger_than {
        if e.file_type().is_file() {
            if let Ok(md) = e.metadata() {
                let size = md.len();
                if size > skip_larger_than.as_bytes() {
                    debug!(
                        "Skipping path {}: size exceeds limit ({})",
                        path.display(),
                        size
                    );
                    return false;
                }
            }
        }
    }

    true
}

pub fn ingest_directory(cfg: &ScanConfig, tx: &Sender<DirEntry>, path: &Path) {
    let walker = WalkDir::new(path).into_iter();
    for entry in walker.filter_entry(|e| matches(cfg, e)) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("Failed to scan directory: {:#}", err);
                continue;
            }
        };

        let path = entry.path();
        let ft = entry.file_type();

        trace!("Walkdir found {}", path.display());

        if ft.is_dir() {
            debug!("Traversing directory: {}", path.display());
            continue;
        }

        if ft.is_symlink() {
            debug!("Skipping symlink: {}", path.display());
            continue;
        }

        if tx.send(entry).is_err() {
            break;
        }
    }
}

pub struct Scanner {
    engine: Engine,
    signature_count: u32,
    signatures_age: DateTime<Utc>,
}

impl Scanner {
    pub fn new(path: &Path) -> Result<Scanner> {
        let scanner = Engine::new();
        info!("Loading database from {}...", path.display());

        let path_str = path_to_string(path)?;
        let stats = scanner
            .load_databases(&path_str)
            .map_err(|e| anyhow!("Failed to load clamav database: {:#}", e))?;

        info!("Checking database age...");
        let daily_path = path.join("daily.cld");
        let mut buf = [0; 512];
        read_clamav_header(&daily_path, &mut buf)?;
        let signatures_age = parse_database_age(&buf)?;

        info!("Compiling clamav rules...");
        scanner
            .compile()
            .map_err(|e| anyhow!("Failed to compile clamav rules: {:#}", e))?;

        Ok(Scanner {
            engine: scanner,
            signature_count: stats.signature_count,
            signatures_age,
        })
    }

    #[must_use]
    pub fn signature_count(&self) -> usize {
        self.signature_count as usize
    }

    #[must_use]
    pub fn signatures_age(&self) -> DateTime<Utc> {
        self.signatures_age
    }

    pub fn scan_file(&self, path: &Path, results_tx: &Sender<(PathBuf, String)>) -> Result<()> {
        debug!("Scanning file {}...", path.display());

        let path_str = path_to_string(path)?;
        let mut settings = ScanSettings::default();
        let hit = self
            .engine
            .scan_file(&path_str, &mut settings)
            .map_err(|e| anyhow!("Failed to scan file {:?}: {:#}", path, e))?;

        match hit {
            ScanResult::Virus(name) => {
                warn!("Found threat: {} ({:?})", path.display(), name);
                results_tx.send((path.to_path_buf(), name)).ok();
            }
            ScanResult::Clean | ScanResult::Whitelisted => (),
        }

        Ok(())
    }
}

pub fn run(mut args: args::Scan) -> Result<()> {
    let config = config::load().context("Failed to load config")?;

    let mut db = Database::load().context("Failed to load database")?;

    if args.paths.is_empty() {
        info!("Empty arguments, defaulting to home directory");
        let home_dir = dirs::home_dir().context("Failed to find home directory")?;
        args.paths.push(home_dir);
    }

    let data = db.data_mut();
    data.threats.clear();

    let (results_tx, results_rx) = crossbeam_channel::unbounded();
    let (fs_tx, fs_rx) = crossbeam_channel::bounded::<DirEntry>(128);

    let scanner = Scanner::new(&config.update.path)?;
    let scanner = Arc::new(scanner);

    let cpus = num_cpus::get();
    info!("Spawning {} scanner(s)...", cpus);
    for _ in 0..cpus {
        let results_tx = results_tx.clone();
        let fs_rx = fs_rx.clone();
        let scanner = scanner.clone();
        thread::spawn(move || {
            for entry in fs_rx {
                if let Err(err) = scanner.scan_file(entry.path(), &results_tx) {
                    error!("{:#}", err);
                }
            }
            mem::drop(results_tx);
        });
    }
    mem::drop(results_tx);

    thread::spawn(move || {
        for path in args.paths {
            info!("Scanning directory {}...", path.display());
            ingest_directory(&config.scan, &fs_tx, &path);
        }
        debug!("Finished traversing directories");
    });

    data.signature_count = scanner.signature_count();
    data.signatures_age = Some(scanner.signatures_age());
    for (path, name) in results_rx {
        data.threats.entry(path).or_default().push(name);
    }
    info!("Scan finished, found {} threat(s)!", data.threats.len());

    data.last_scan = Some(Utc::now());
    db.store().context("Failed to write database")?;

    Ok(())
}

pub fn read_clamav_header(path: &Path, buf: &mut [u8]) -> Result<()> {
    if buf.len() != 512 {
        bail!("Buffer has wrong size");
    }

    let mut f = File::open(path).context("Failed to open clamav database")?;
    f.read_exact(buf)
        .context("Failed to read header from clamav database")?;

    Ok(())
}

pub fn parse_database_age(mut buf: &[u8]) -> Result<DateTime<Utc>> {
    for i in 0..8 {
        let idx = memchr::memchr(b':', buf)
            .with_context(|| anyhow!("Failed to select field number #{}", i))?;
        buf = &buf[idx + 1..];
    }

    let idx =
        memchr::memchr(b' ', buf).context("Failed to remove remaining data from timestamp")?;
    let buf = &buf[..idx];

    let num = atoi::atoi::<i64>(buf).context("Failed to parse timestamp as number")?;

    Ok(Utc.timestamp(num, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_from_header() {
        let dt = parse_database_age(
            b"ClamAV-VDB:09 May 2021 07-08 -0400:26165:3978101:63:X:X:raynman:1620558516    ",
        )
        .unwrap();
        assert_eq!(dt, Utc.ymd(2021, 5, 9).and_hms(11, 8, 36));
    }
}
