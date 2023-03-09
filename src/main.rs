use clap::Parser;
use cli::Cli;

pub mod cli;

fn main() {
    let cli = Cli::parse();

    match cli.subcommand {
        cli::Command::Run => println!("Run"),
        cli::Command::Completion { shell } => Cli::generate_completion(shell),
    }
}
