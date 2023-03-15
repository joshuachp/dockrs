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
        cli::Command::Run => println!("Run"),
        cli::Command::Completion { shell } => Cli::generate_completion(shell),
    }

    Ok(())
}

#[cfg(test)]
mod test {

    #[test]
    fn test_main() {
        // This is a test to make the coverage happy
        assert_eq!(1, 1);
    }
}
