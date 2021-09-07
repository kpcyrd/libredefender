use crate::errors::*;
use notify_rust::{Hint, Notification, Timeout, Urgency};
use std::path::Path;
use v_htmlescape::escape;

pub fn show(path: &Path, detected_as: &str) -> Result<()> {
    let title = format!("Infection found: {:?}", detected_as);
    let body = format!("libredefender found an infected file:\n{:?}\nRun `libredefender infections -h` to take action.", path);
    Notification::new()
        .summary(&title)
        .body(&escape(&body).to_string())
        .icon("libredefender")
        .urgency(Urgency::Critical)
        .hint(Hint::Resident(true)) // this is not supported by all implementations
        .timeout(Timeout::Never) // this however is
        .show()?;
    Ok(())
}
