use std::{collections::HashMap, io};

use bollard::{
    container::{Config, CreateContainerOptions},
    models::HostConfig,
};
use clap::{error::Result, Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use dockrs::get_port_bindings;
use tracing::instrument;

#[derive(Parser)]
#[command(version, about, long_about)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Command,

    /// Enable debug logging
    #[arg(long)]
    pub debug: bool,
}

impl Cli {
    pub fn generate_completion(shell: Shell) {
        generate(shell, &mut Cli::command(), "dockers", &mut io::stdout())
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Create and run a new container from an image
    Run(RunArgs),
    /// Generate shell completion for a given shell
    Completion { shell: Shell },
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// The image to create the container from
    pub image: String,
    /// Assign a name to the container
    #[arg(long)]
    pub name: Option<String>,
    /// Connect a container to a network
    #[arg(long)]
    pub network: Option<String>,
    /// Bind mount a volume
    #[arg(short, long)]
    pub volume: Vec<String>,
    /// Publish a container's port(s) to the host
    #[arg(long, short)]
    pub publish: Vec<String>,
    /// Expose a port or a range of ports
    #[arg(long)]
    pub expose: Vec<String>,
    /// Automatically remove the container when it exits
    #[arg(long)]
    pub rm: bool,
}

impl<'a: 'b, 'b> From<&'a RunArgs> for Option<CreateContainerOptions<&'b str>> {
    #[instrument]
    fn from(args: &'a RunArgs) -> Self {
        args.name
            .as_deref()
            .map(|name| CreateContainerOptions::<&str> {
                name,
                ..Default::default()
            })
    }
}

impl<'a: 'b, 'b> TryFrom<&'a RunArgs> for Config<&'b str> {
    type Error = color_eyre::eyre::Error;

    #[instrument]
    fn try_from(args: &'a RunArgs) -> Result<Self, Self::Error> {
        let port_bindings = Some(get_port_bindings(&args.publish)?);

        let host_config = Some(HostConfig {
            binds: Some(args.volume.clone()),
            network_mode: args.network.clone(),
            port_bindings,
            ..Default::default()
        });

        let expose = args
            .expose
            .iter()
            .map(|port| (port.as_str(), HashMap::new()))
            .collect();

        Ok(Self {
            image: Some(&args.image),
            exposed_ports: Some(expose),
            host_config,
            ..Default::default()
        })
    }
}
