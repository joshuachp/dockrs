use std::{collections::HashMap, io};

use crate::get_port_bindings;
use bollard::{
    container::{Config, CreateContainerOptions},
    models::HostConfig,
};
use clap::{error::Result, Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
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
    Pull {
        /// The image to pull
        image: String,
        /// The tag to pull
        #[arg(short, long, default_value = "latest")]
        tag: String,
    },
    /// Show statistics about the containers
    Stats {
        /// Do not clear the screen
        #[arg(long)]
        keep_screen: bool,
    },
    /// Start one or more stopped containers
    Start {
        /// The container to start
        #[arg(required = true)]
        containers: Vec<String>,
        /// Attach STDOUT/STDERR and forward signals
        #[arg(short, long)]
        attach: bool,
        /// Attach container's STDIN
        #[arg(short, long)]
        interactive: bool,
    },
    /// Stop one or more running containers
    Stop {
        /// The container to stop
        #[arg(required = true)]
        containers: Vec<String>,
    },
    /// List containers
    Ps {
        /// Show all containers (default shows just running)
        #[arg(long, short)]
        all: bool,
        /// Display total file
        #[arg(long)]
        size: bool,
        /// Filter output based on conditions provided
        #[arg(long, short)]
        filter: Vec<String>,
    },
    /// Fetch the logs of a container
    Logs {
        /// The container to get logs from
        container: String,
        /// Follow log output
        #[arg(long, short)]
        follow: bool,
        /// Display total file
        #[arg(long, short = 'n')]
        tail: Option<usize>,
    },
    /// Remove one or more containers
    Rm {
        /// The containers to remove
        #[arg(required = true)]
        containers: Vec<String>,
        /// Force the removal of a running container (uses SIGKILL)
        #[arg(long, short)]
        force: bool,
        /// Remove anonymous volumes associated with the container
        #[arg(long, short)]
        volumes: bool,
        /// Remove the specified link
        #[arg(long, short)]
        link: bool,
    },
    /// Remove one or more images
    Rmi {
        /// The images to remove
        #[arg(required = true)]
        images: Vec<String>,
        /// Force removal of the image
        #[arg(long, short)]
        force: bool,
    },
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

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_option_from_run_args() {
        let args = RunArgs {
            image: "alpine".to_string(),
            name: Some("test".to_string()),
            network: None,
            volume: vec![],
            publish: vec![],
            expose: vec![],
            rm: false,
        };

        let options = CreateContainerOptions::<&str> {
            name: "test",
            ..Default::default()
        };

        assert_eq!(Some(options), (&args).into());
    }

    #[test]
    fn test_config_from_run_args() {
        let args = RunArgs {
            image: "alpine".to_string(),
            name: Some("test".to_string()),
            network: None,
            volume: vec![],
            publish: vec![],
            expose: vec![],
            rm: false,
        };

        let config = Config::<&str> {
            image: Some("alpine"),
            exposed_ports: Some(HashMap::new()),
            host_config: Some(HostConfig {
                binds: Some(vec![]),
                port_bindings: Some(HashMap::new()),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(config, Config::try_from(&args).unwrap());
    }
}
