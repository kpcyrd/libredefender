use crate::errors::*;
use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::io::stdout;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Args {
    /// Only show warnings
    #[clap(short, long, global = true)]
    pub quiet: bool,
    /// More verbose logs
    #[clap(short, long, global = true, action = ArgAction::Count)]
    pub verbose: u8,
    #[clap(short = 'C', long, global = true)]
    pub colors: bool,
    #[clap(short = 'D', long, global = true)]
    pub data: Option<PathBuf>,
    #[clap(subcommand)]
    pub subcommand: Option<SubCommand>,
}

#[derive(Parser)]
pub enum SubCommand {
    /// Scan directories for signature matches
    Scan(Scan),
    /// Run a background service that scans periodically
    Scheduler(Scheduler),
    /// List threats that have been detected
    Infections(Infections),
    /// Send a test notification
    TestNotify,
    /// Load the configuration and print it as json for debugging
    DumpConfig,
    /// Generate shell completions
    Completions(Completions),
}

#[derive(Parser, Default)]
pub struct Scan {
    /// Paths that should be scanned
    pub paths: Vec<PathBuf>,
    /// Configure the number of scanning threads, defaults to number of cpu cores
    #[clap(short = 'j', long)]
    pub concurrency: Option<usize>,
}

#[derive(Parser)]
pub struct Scheduler {}

#[derive(Parser)]
pub struct Infections {
    /// Interactively offer deletion for every file
    #[clap(short, long, group = "action")]
    pub delete: bool,
    /// Delete all files without further confirmation (DANGER!)
    #[clap(long, group = "action")]
    pub delete_all: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct Completions {
    pub shell: Shell,
}

impl Completions {
    pub fn gen_completions(&self) -> Result<()> {
        clap_complete::generate(
            self.shell,
            &mut Args::command(),
            "libredefender",
            &mut stdout(),
        );
        Ok(())
    }
}
