use std::io;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};

#[derive(Parser)]
#[command(version, about, long_about)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Command,
}

impl Cli {
    pub fn generate_completion(shell: Shell) {
        generate(shell, &mut Cli::command(), "dockers", &mut io::stdout())
    }
}

#[derive(Subcommand)]
pub enum Command {
    Run {
        /// The image to run
        image: String,
    },
    /// Generate shell completion for a given shell
    Completion { shell: Shell },
}
