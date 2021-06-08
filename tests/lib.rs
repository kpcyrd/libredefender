use crossbeam_channel::Receiver;
use env_logger::Env;
use libredefender::config::ScanConfig;
use libredefender::errors::*;
use libredefender::patterns::Pattern;
use libredefender::scan;
use libredefender::scan::Scanner;
use std::env;
use std::fs;
use std::mem;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tempdir::TempDir;
use walkdir::DirEntry;

const EICAR: &str = "X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";

fn init() {
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .is_test(true)
        .try_init();
}

fn clamav_dir() -> PathBuf {
    let path = env::var("CLAMAV_PATH").unwrap_or_else(|_| "/var/lib/clamav".to_string());
    PathBuf::from(path)
}

fn run_scan(cfg: &ScanConfig, path: &Path) -> Receiver<(PathBuf, String)> {
    let (results_tx, results_rx) = crossbeam_channel::unbounded();
    let (fs_tx, fs_rx) = crossbeam_channel::bounded::<DirEntry>(128);

    let scanner = Scanner::new(&clamav_dir()).unwrap();
    let scanner = Arc::new(scanner);

    scan::ingest_directory(cfg, &fs_tx, path);
    mem::drop(fs_tx);

    for entry in fs_rx {
        if let Err(err) = scanner.scan_file(entry.path(), &results_tx) {
            error!("{:#}", err);
        }
    }
    results_rx
}

#[test]
#[ignore]
fn test_find_threat() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();

    let eicar_file_path = tmp_dir.path().join("eicar.txt");
    fs::write(eicar_file_path, EICAR).unwrap();

    let results_rx = run_scan(&ScanConfig::default(), tmp_dir.path());

    let (_path, res) = results_rx.recv().unwrap();
    assert_eq!(res, "Win.Test.EICAR_HDB-1");

    assert!(results_rx.recv().is_err());
}

#[test]
#[ignore]
fn test_find_no_threat() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();

    let eicar_file_path = tmp_dir.path().join("eicar.txt");
    fs::write(eicar_file_path, "heeeello i am no virus, i swear!").unwrap();

    let results_rx = run_scan(&ScanConfig::default(), tmp_dir.path());

    assert!(results_rx.recv().is_err());
}

#[test]
#[ignore]
fn test_find_no_threat_multiple_files() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();

    for i in 1..127 {
        let eicar_file_path = tmp_dir.path().join(format!("no_eicar_{}.txt", i));
        fs::write(eicar_file_path, "heeeello i am no virus, i swear!").unwrap();
    }

    let results_rx = run_scan(&ScanConfig::default(), tmp_dir.path());

    assert!(results_rx.recv().is_err());
}

#[test]
#[ignore]
fn test_find_threat_in_deep_recursion() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();
    let mut accu_dir = tmp_dir.path().to_owned();

    for _i in 1..255 {
        accu_dir = accu_dir.join("step");
        fs::create_dir(&accu_dir).unwrap();
    }

    let eicar_file_path = accu_dir.join("eicar.txt");
    fs::write(eicar_file_path, EICAR).unwrap();

    let results_rx = run_scan(&ScanConfig::default(), tmp_dir.path());

    let (_path, res) = results_rx.recv().unwrap();
    assert_eq!(res, "Win.Test.EICAR_HDB-1");

    assert!(results_rx.recv().is_err());
}

#[test]
#[ignore]
fn test_can_not_find_threat_with_missing_permissions_for_file() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();

    let eicar_file_path = tmp_dir.path().join("eicar.txt");
    fs::write(&eicar_file_path, EICAR).unwrap();

    fs::set_permissions(eicar_file_path, fs::Permissions::from_mode(0o0)).unwrap();

    let results_rx = run_scan(&ScanConfig::default(), tmp_dir.path());

    assert!(results_rx.recv().is_err());
}

#[test]
#[ignore]
fn test_can_not_find_threat_with_missing_permissions_for_dir() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();

    let eicar_file_path = tmp_dir.path().join("eicar.txt");
    fs::write(&eicar_file_path, EICAR).unwrap();

    fs::set_permissions(tmp_dir.path(), fs::Permissions::from_mode(0o0)).unwrap();

    let results_rx = run_scan(&ScanConfig::default(), tmp_dir.path());

    assert!(results_rx.recv().is_err());
}

#[test]
#[ignore]
fn test_skips_excluded_files_by_absolute_path() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();

    let eicar_file_path = tmp_dir.path().join("eicar.txt");
    fs::write(eicar_file_path, EICAR).unwrap();

    let skip_file_path = tmp_dir.path().join("skip_me.txt");
    fs::write(&skip_file_path, EICAR).unwrap();

    let scan_config = ScanConfig {
        skip_hidden: false,
        excludes: vec![
            Pattern::from_str(&skip_file_path.into_os_string().into_string().unwrap()).unwrap(),
        ],
        ..Default::default()
    };
    let results_rx = run_scan(&scan_config, tmp_dir.path());

    let (_path, res) = results_rx.recv().unwrap();
    assert_eq!(res, "Win.Test.EICAR_HDB-1");

    assert!(results_rx.recv().is_err());
}

#[test]
#[ignore]
fn test_skips_excluded_files_by_absolute_directory_path() {
    init();

    let tmp_dir = TempDir::new("libredefender").unwrap();

    let skip_me_dir = tmp_dir.path().join("skip_me");
    fs::create_dir(&skip_me_dir).unwrap();

    let skip_file_path = skip_me_dir.join("eicar.txt");
    fs::write(&skip_file_path, EICAR).unwrap();

    let scan_config = ScanConfig {
        skip_hidden: false,
        excludes: vec![
            Pattern::from_str(&skip_me_dir.into_os_string().into_string().unwrap()).unwrap(),
        ],
        ..Default::default()
    };
    let results_rx = run_scan(&scan_config, tmp_dir.path());

    assert!(results_rx.recv().is_err());
}
