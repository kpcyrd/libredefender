use crate::errors::*;
use std::io::stdout;
use std::path::PathBuf;
use structopt::clap::{AppSettings, Shell};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
pub struct Args {
    /// Only show warnings
    #[structopt(short, long, global = true)]
    pub quiet: bool,
    /// More verbose logs
    #[structopt(short, long, global = true, parse(from_occurrences))]
    pub verbose: u8,
    #[structopt(short = "C", long, global = true)]
    pub colors: bool,
    #[structopt(short = "D", long, global = true)]
    pub data: Option<PathBuf>,
    #[structopt(subcommand)]
    pub subcommand: Option<SubCommand>,
}

#[derive(StructOpt)]
pub enum SubCommand {
    /// Scan directories for signature matches
    Scan(Scan),
    /// Run a background service that scans periodically
    Scheduler(Scheduler),
    /// List threats that have been detected
    Infections(Infections),
    /// Generate shell completions
    Completions(Completions),
}

#[derive(StructOpt)]
pub struct Scan {
    /// Paths that should be scanned
    pub paths: Vec<PathBuf>,
}

#[derive(StructOpt)]
pub struct Scheduler {}

#[derive(StructOpt)]
pub struct Infections {
    /// Interactively offer deletion for every file
    #[structopt(short, long, group = "action")]
    pub delete: bool,
    /// Delete all files without further confirmation (DANGER!)
    #[structopt(long, group = "action")]
    pub delete_all: bool,
}

#[derive(Debug, Clone, StructOpt)]
pub struct Completions {
    #[structopt(possible_values=&Shell::variants())]
    pub shell: Shell,
}

impl Completions {
    pub fn gen_completions(&self) -> Result<()> {
        Args::clap().gen_completions_to("libredefender", self.shell, &mut stdout());
        Ok(())
    }
}
