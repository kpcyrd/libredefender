use crate::errors::*;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::path::Path;

pub fn ask_confirmation(text: &str) -> Result<bool> {
    let mut stdout = io::stdout();
    write!(stdout, "{} [y/N] ", text)?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let mut input = input.chars().next().context("Stdin was closed")?;

    input.make_ascii_lowercase();
    Ok(input == 'y')
}

pub fn ensure_deleted(path: &Path) -> Result<()> {
    match fs::remove_file(&path) {
        Ok(()) => (),
        Err(err) if err.kind() == io::ErrorKind::NotFound => (),
        err => err.with_context(|| anyhow!("Failed to delete {:?}", path))?,
    }
    Ok(())
}
