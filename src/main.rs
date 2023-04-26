use std::env;

use clap::Parser;
use color_eyre::{eyre::Context, Result};
use dockrs::cli::{Cli, Command};
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let mut filter = EnvFilter::try_new(env::var("RUST_LOG").as_deref().unwrap_or(""))
        .wrap_err("Failed to parse RUST_LOG env var")?;

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
        Command::Stats { keep_screen } => dockrs::stats(&docker, keep_screen).await?,
        Command::Start {
            containers,
            attach,
            interactive,
        } => dockrs::start(&docker, &containers, attach, interactive).await?,
        Command::Stop { containers } => dockrs::stop(&docker, &containers).await?,
        Command::Ps { all, size, filter } => dockrs::list(&docker, all, size, &filter).await?,
        Command::Logs {
            container,
            follow,
            tail,
        } => dockrs::logs(&docker, &container, follow, tail).await?,
        Command::Rm {
            containers,
            force,
            volumes,
            link,
        } => dockrs::rm(&docker, &containers, force, volumes, link).await?,
        Command::Rmi { images, force } => dockrs::rmi(&docker, &images, force).await?,
        Command::Events { filter } => dockrs::events(&docker, &filter).await?,
        Command::Completion { .. } => unreachable!(),
    }

    Ok(())
}
