use clap::Parser;
use cli::Cli;
use color_eyre::Result;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let mut filter = EnvFilter::from_default_env();

    let cli = Cli::parse();

    if cli.debug {
        filter = filter.add_directive(LevelFilter::TRACE.into());
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    match cli.subcommand {
        cli::Command::Run(ref run) => dockrs::run(run.into(), run.try_into()?, run.rm).await?,
        cli::Command::Completion { shell } => Cli::generate_completion(shell),
    }

    Ok(())
}
