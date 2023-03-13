use std::{collections::HashMap, fmt::Display, ops::Deref};

#[cfg(not(feature = "mock"))]
use bollard::Docker;
use bollard::{
    container::{
        AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions,
        StartContainerOptions,
    },
    image::CreateImageOptions,
    service::PortBinding,
};
use color_eyre::{eyre::ContextCompat, Result};
use futures::StreamExt;
#[cfg(feature = "mock")]
use mock::{Docker as DockerTrait, MockDocker as Docker};
use tracing::{instrument, warn};

pub mod cli;
#[cfg(feature = "mock")]
mod mock;

pub fn connect_to_docker() -> Result<Docker> {
    let docker = Docker::connect_with_local_defaults()?;

    Ok(docker)
}

pub fn get_port_bindings<T: Deref<Target = str> + Display>(
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

#[instrument(skip(options, config))]
pub async fn run(
    options: Option<CreateContainerOptions<&str>>,
    config: Config<&str>,
    rm: bool,
) -> Result<()> {
    let docker = connect_to_docker()?;

    let container = docker.create_container(options, config).await?;

    if !container.warnings.is_empty() {
        warn!("Warnings while creating the container");
        for warning in container.warnings {
            warn!(?warning);
        }
    }

    let options = AttachContainerOptions::<&str> {
        stream: Some(true),
        stdin: Some(false),
        stdout: Some(true),
        stderr: Some(true),
        ..Default::default()
    };

    let AttachContainerResults { mut output, .. } = docker
        .attach_container(&container.id, Some(options))
        .await?;

    let join = tokio::spawn(async move {
        while let Some(chunk) = output.next().await {
            print!("{}", chunk.unwrap());
        }
    });

    docker
        .start_container(&container.id, None::<StartContainerOptions<&str>>)
        .await?;

    join.await?;

    if rm {
        docker.remove_container(&container.id, None).await?;
    }

    Ok(())
}

pub async fn pull(image: &str, tag: &str) -> Result<()> {
    let docker = connect_to_docker()?;

    let options = CreateImageOptions {
        from_image: format!("{}:{}", image, tag),
        ..Default::default()
    };

    let mut stream = docker.create_image(Some(options), None, None);

    while let Some(info) = stream.next().await {
        let info = info?;

        println!("{:?}", info);
    }

    Ok(())
}

#[cfg(test)]
mod test {

    use super::*;

    macro_rules! docker_test {
        ($mock:expr, $test:block) => {
            #[cfg(feature = "mock")]
            let ctx = $mock;

            $test

            #[cfg(feature = "mock")]
            drop(ctx);
        };
    }

    #[tokio::test]
    async fn test_run() {
        docker_test!(
            {
                use bollard::service::ContainerCreateResponse;
                use mock::MockDocker;
                use tokio::io::BufWriter;

                let create_container = ContainerCreateResponse {
                    id: "test".to_string(),
                    warnings: vec![],
                };

                let attach_container = AttachContainerResults {
                    input: Box::pin(BufWriter::new(Vec::new())),
                    output: Box::pin(futures::stream::empty()),
                };

                let mut mock = MockDocker::new();

                mock.expect_create_container()
                    .return_once(|_, _| Ok(create_container));
                mock.expect_attach_container()
                    .return_once(|_, _| Ok(attach_container));
                mock.expect_start_container().return_once(|_, _| Ok(()));
                mock.expect_remove_container().return_once(|_, _| Ok(()));

                let ctx = MockDocker::connect_with_local_defaults_context();

                ctx.expect().return_once(move || Ok(mock));

                ctx
            },
            {
                let options = None;
                let config = Config {
                    image: Some("hello-world"),
                    ..Default::default()
                };
                let rm = true;

                let result = super::run(options, config, rm).await;

                assert!(result.is_ok(), "run failed with {:?}", result);
            }
        );
    }

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
