use std::{collections::HashMap, fmt::Display, io, ops::Deref};

use bollard::{
    container::{Config, CreateContainerOptions},
    models::HostConfig,
    service::PortBinding,
};
use clap::{error::Result, Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use color_eyre::eyre::ContextCompat;
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

fn get_port_bindings<T: Deref<Target = str> + Display>(
    input: &[T],
) -> color_eyre::Result<HashMap<String, Option<Vec<PortBinding>>>> {
    let mut bindings = HashMap::new();

    for input in input {
        let mut parts = input.rsplitn(3, ':');

        let first = parts
            .next()
            .wrap_err_with(|| format!("Invalid port binding {}", input))?;

        let second = parts.next();
        let third = parts.next();

        let entry = bindings
            .entry(first.to_string())
            .or_insert_with(|| Some(vec![]));

        match (second, third) {
            (None, None) => {}
            (Some(second), None) => {
                entry.as_mut().unwrap().push(PortBinding {
                    host_ip: None,
                    host_port: Some(second.to_string()),
                });
            }
            (Some(second), Some(third)) => {
                entry.as_mut().unwrap().push(PortBinding {
                    host_ip: Some(third.to_string()),
                    host_port: Some(second.to_string()),
                });
            }
            _ => unreachable!("Invalid port binding {}", input),
        }
    }

    Ok(bindings)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_port_binding() {
        let input = ["80", "443:8080", "127.0.0.1:80:8080"];

        let binginds = get_port_bindings(&input).unwrap();

        let expected = [
            ("80".to_string(), Some(vec![])),
            (
                "8080".to_string(),
                Some(vec![
                    PortBinding {
                        host_ip: None,
                        host_port: Some("443".to_string()),
                    },
                    PortBinding {
                        host_ip: Some("127.0.0.1".to_string()),
                        host_port: Some("80".to_string()),
                    },
                ]),
            ),
        ];
        let expected = HashMap::from(expected);

        assert_eq!(binginds, expected);
    }
}
