use clap::Parser;
use color_eyre::Result;
use dockrs::cli::{Cli, Command};
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

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

    if let Command::Completion { shell } = cli.subcommand {
        Cli::generate_completion(shell);

        return Ok(());
    }

    let docker = dockrs::connect_to_docker()?;

    match cli.subcommand {
        Command::Run(ref run) => dockrs::run(&docker, run.into(), run.try_into()?, run.rm).await?,
        Command::Pull { image, tag } => dockrs::pull(&docker, &image, &tag).await?,
        Command::Stats => dockrs::stats(&docker).await?,
        Command::Completion { .. } => unreachable!(),
    }

    Ok(())
}
