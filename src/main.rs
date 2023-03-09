use clap::Parser;
use cli::Cli;
use color_eyre::Result;
use tracing_subscriber::{prelude::*, EnvFilter};

pub mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.subcommand {
        cli::Command::Run { ref image } => dockrs::run(image).await?,
        cli::Command::Completion { shell } => Cli::generate_completion(shell),
    }

    Ok(())
}
