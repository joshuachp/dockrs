use std::{collections::HashMap, fmt::Display, ops::Deref};

use bollard::{
    container::{
        AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions, LogOutput,
        LogsOptions, RemoveContainerOptions, StartContainerOptions,
    },
    image::{CreateImageOptions, RemoveImageOptions},
    service::PortBinding,
    system::EventsOptions,
};
use chrono::Local;
use color_eyre::{eyre::ensure, eyre::ContextCompat, Result};
use futures::{future::join_all, StreamExt};

#[cfg(not(feature = "mock"))]
use bollard::Docker;
#[cfg(feature = "mock")]
use mock::{DockerTrait, MockDocker as Docker};

pub use list::list;
pub use stats::stats;

pub mod cli;
mod list;
#[cfg(feature = "mock")]
mod mock;
mod stats;

use tokio::{
    io::{stderr, stdout, AsyncBufReadExt, AsyncWriteExt},
    task::JoinHandle,
};
use tracing::{error, instrument, warn};

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

pub fn parse_filter(input: &str) -> Result<(&str, &str)> {
    let mut parts = input.splitn(2, '=');

    let filter = parts
        .next()
        .wrap_err_with(|| format!("Invalid filter {}", input))?;

    let value = parts
        .next()
        .wrap_err_with(|| format!("Invalid filter {}", input))?;

    Ok((filter, value))
}

async fn attach_container(
    docker: &Docker,
    container: &str,
    interactive: bool,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let options = AttachContainerOptions::<&str> {
        stream: Some(true),
        stdin: Some(interactive),
        stdout: Some(true),
        stderr: Some(true),
        ..Default::default()
    };

    let AttachContainerResults {
        mut output,
        mut input,
    } = docker.attach_container(container, Some(options)).await?;

    let join: JoinHandle<Result<()>> = tokio::spawn(async move {
        let mut stdout = stdout();
        let mut stderr = stderr();

        while let Some(chunk) = output.next().await {
            match chunk? {
                LogOutput::StdOut { message } => stdout.write(&message).await?,
                LogOutput::StdErr { message } => stderr.write(&message).await?,
                LogOutput::StdIn { .. } => unreachable!("We didn't ask for stdin"),
                LogOutput::Console { message } => stdout.write(&message).await?,
            };

            stdout.flush().await?;
            stderr.flush().await?;
        }

        Ok(())
    });

    if interactive {
        let out: JoinHandle<Result<()>> = tokio::spawn(async move {
            let mut buf = String::new();
            let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());

            loop {
                stdin.read_line(&mut buf).await?;
                input.write_all(buf.as_bytes()).await?;
            }
        });

        return Ok(vec![join, out]);
    }

    Ok(vec![join])
}

#[instrument(skip(options, config))]
pub async fn run(
    docker: &Docker,
    options: Option<CreateContainerOptions<&str>>,
    config: Config<&str>,
    rm: bool,
) -> Result<()> {
    let container = docker.create_container(options, config).await?;

    if !container.warnings.is_empty() {
        warn!("Warnings while creating the container");
        for warning in container.warnings {
            warn!(?warning);
        }
    }

    let join = attach_container(docker, &container.id, false).await?;

    docker
        .start_container(&container.id, None::<StartContainerOptions<&str>>)
        .await?;

    join_all(join).await;

    if rm {
        docker.remove_container(&container.id, None).await?;
    }

    Ok(())
}

pub async fn pull(docker: &Docker, image: &str, tag: &str) -> Result<()> {
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

#[instrument]
pub async fn start(
    docker: &Docker,
    containers: &[String],
    attach: bool,
    interactive: bool,
) -> Result<()> {
    ensure!(
        containers.len() == 1 || (!attach && !interactive),
        "Can only attach to one container at a time"
    );

    let options = StartContainerOptions::<&str> {
        ..Default::default()
    };

    let mut attach_join: Vec<JoinHandle<Result<()>>> = Vec::new();
    if attach || interactive {
        for container in containers {
            attach_join = attach_container(docker, container, interactive).await?;
        }
    }

    let starts = containers.iter().map(|container| {
        let docker = docker.clone();
        let options = options.clone();
        let container = container.clone();

        tokio::spawn(async move {
            let res = docker.start_container(&container, Some(options)).await;
            (container, res)
        })
    });

    let err = join_all(starts)
        .await
        .iter()
        .fold(false, |acc, join| match join {
            Ok((container, Ok(()))) => {
                println!("{container}");
                acc
            }
            Ok((container, Err(err))) => {
                error!(?container, ?err, "Failed to start container");

                true
            }
            Err(e) => {
                error!(?e, "Failed to start container");

                true
            }
        });

    ensure!(!err, "Failed to start containers");

    if attach || interactive {
        for join in join_all(attach_join).await {
            join??;
        }
    }

    Ok(())
}

pub async fn stop(docker: &Docker, containers: &[String]) -> Result<()> {
    let stops = containers.iter().map(|container| {
        let docker = docker.clone();
        let container = container.clone();

        tokio::spawn(async move {
            let res = docker.stop_container(&container, None).await;
            (container, res)
        })
    });

    let err = join_all(stops)
        .await
        .iter()
        .fold(false, |err, join| match join {
            Ok((container, Ok(()))) => {
                println!("{container}");

                err
            }
            Ok((container, Err(err))) => {
                error!(?container, ?err, "Failed to stop container");

                true
            }
            Err(e) => {
                error!(?e, "Failed to stop container");
                true
            }
        });

    ensure!(!err, "Failed to stop containers");

    Ok(())
}

pub async fn logs(
    docker: &Docker,
    container: &str,
    follow: bool,
    tail: Option<usize>,
) -> Result<()> {
    let options = LogsOptions::<String> {
        follow,
        stdout: true,
        stderr: true,
        tail: tail.map(|t| t.to_string()).unwrap_or_default(),
        ..Default::default()
    };

    let mut stream = docker.logs(container, Some(options));

    while let Some(info) = stream.next().await {
        let info = info?;

        print!("{}", info);
    }

    Ok(())
}

pub async fn rm(
    docker: &Docker,
    containers: &[String],
    force: bool,
    volumes: bool,
    link: bool,
) -> Result<()> {
    let options = RemoveContainerOptions {
        force,
        v: volumes,
        link,
    };

    let removes = containers.iter().map(|container| {
        let docker = docker.clone();
        let container = container.clone();

        tokio::spawn(async move {
            let res = docker.remove_container(&container, Some(options)).await;
            (container, res)
        })
    });

    let err = join_all(removes)
        .await
        .iter()
        .fold(false, |err, join| match join {
            Ok((container, Ok(()))) => {
                println!("{container}");

                err
            }
            Ok((container, Err(err))) => {
                error!(?container, ?err, "Failed to remove container");

                true
            }
            Err(e) => {
                error!(?e, "Failed to remove container");

                true
            }
        });

    ensure!(!err, "Failed to remove containers");

    Ok(())
}

pub async fn rmi(docker: &Docker, images: &[String], force: bool) -> Result<()> {
    let options = RemoveImageOptions {
        force,
        ..Default::default()
    };

    let removes = images.iter().map(|image| {
        let docker = docker.clone();
        let image = image.clone();

        tokio::spawn(async move { docker.remove_image(&image, Some(options), None).await })
    });

    let err = join_all(removes)
        .await
        .iter()
        .fold(false, |err, join| match join {
            Ok(Ok(responses)) => {
                responses.iter().for_each(|resp| {
                    if let Some(untaged) = &resp.untagged {
                        println!("Untagged: {}", untaged);
                    }

                    if let Some(deleted) = &resp.deleted {
                        println!("Deleted: {}", deleted);
                    }
                });

                err
            }
            Ok(Err(err)) => {
                error!(?err, "Failed to remove image");

                true
            }
            Err(e) => {
                error!(?e, "Failed to remove image");

                true
            }
        });

    ensure!(!err, "Failed to remove images");

    Ok(())
}

#[instrument(skip(docker))]
pub async fn events(docker: &Docker, filter: &[String]) -> Result<()> {
    let filters: HashMap<&str, Vec<&str>> =
        filter
            .iter()
            .try_fold(HashMap::new(), |mut acc, filter| -> Result<_> {
                let (filter, value) = parse_filter(filter)?;

                acc.entry(filter).or_insert_with(Vec::new).push(value);

                Ok(acc)
            })?;

    let options = EventsOptions {
        filters,
        ..Default::default()
    };

    let mut stream = docker.events(Some(options));

    while let Some(event) = stream.next().await {
        let event = event?;

        let time = Local::now();

        let typ = event
            .typ
            .map(|t| t.to_string())
            .unwrap_or_else(|| "".to_string());

        let action = event.action.unwrap_or_default();

        let actor_id = event
            .actor
            .as_ref()
            .and_then(|actor| actor.id.as_deref())
            .unwrap_or("");

        let actor_attributes = event
            .actor
            .as_ref()
            .and_then(|actor| actor.attributes.as_ref())
            .map(|attr| {
                attr.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "".to_string());

        println!(
            "{} {} {} {} ({})",
            time.to_rfc3339(),
            typ,
            action,
            actor_id,
            actor_attributes
        );
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[macro_export]
    macro_rules! docker_test {
        ($mock:expr) => {{
            #[cfg(feature = "mock")]
            let docker: Docker = $mock;

            #[cfg(not(feature = "mock"))]
            let docker: Docker = $crate::connect_to_docker().unwrap();

            docker
        }};
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

    #[test]
    fn test_get_port_binding_ipv6() {
        let input = ["[::]:443:8080"];

        let binginds = get_port_bindings(&input).unwrap();

        let expected = [(
            "8080".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("[::]".to_string()),
                host_port: Some("443".to_string()),
            }]),
        )];

        assert_eq!(binginds, HashMap::from(expected));
    }

    #[test]
    fn test_parse_filter() {
        let input = "label=foo=bar";

        let filter = parse_filter(input).unwrap();

        let expected = ("label", "foo=bar");

        assert_eq!(filter, expected);
    }

    #[tokio::test]
    async fn test_run() {
        let docker = docker_test!({
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

            mock
        });

        let options = None;
        let config = Config {
            image: Some("hello-world"),
            ..Default::default()
        };
        let rm = true;

        let result = run(&docker, options, config, rm).await;

        assert!(result.is_ok(), "run failed with {:?}", result);
    }

    #[tokio::test]
    async fn test_pull() {
        let docker = docker_test!({
            use mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_create_image()
                .return_once(|_, _, _| Box::pin(futures::stream::empty()));

            mock
        });

        let image = "hello-world";
        let tag = "latest";

        let result = pull(&docker, image, tag).await;

        assert!(result.is_ok(), "pull failed with {:?}", result);
    }

    #[tokio::test]
    async fn test_start() {
        let docker = docker_test!({
            use mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_clone().return_once(|| {
                let mut mock = MockDocker::new();

                mock.expect_start_container().return_once(|_, _| Ok(()));

                mock
            });

            mock
        });

        let containers = vec!["test".to_string()];
        let attach = false;
        let interactive = false;

        let result = start(&docker, &containers, attach, interactive).await;

        assert!(result.is_ok(), "start failed with {:?}", result);
    }

    #[tokio::test]
    async fn test_stop() {
        let docker = docker_test!({
            use mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_clone().return_once(|| {
                let mut mock = MockDocker::new();

                mock.expect_stop_container().return_once(|_, _| Ok(()));

                mock
            });

            mock
        });

        let containers = vec!["test".to_string()];

        let result = stop(&docker, &containers).await;

        assert!(result.is_ok(), "stop failed with {:?}", result);
    }

    #[tokio::test]
    async fn test_logs() {
        let docker = docker_test!({
            use mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_logs()
                .return_once(|_, _| Box::pin(futures::stream::empty()));

            mock
        });

        let container = "test";
        let follow = false;
        let tail = None;

        let result = logs(&docker, container, follow, tail).await;

        assert!(result.is_ok(), "logs failed with {:?}", result);
    }

    #[tokio::test]
    async fn test_remove() {
        let docker = docker_test!({
            use mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_clone().returning(|| {
                let mut mock = MockDocker::new();
                mock.expect_remove_container().return_once(|_, _| Ok(()));

                mock
            });

            mock
        });

        let containers = vec!["test".to_string()];

        let result = rm(&docker, &containers, true, true, true).await;

        assert!(result.is_ok(), "remove failed with {:?}", result);
    }

    #[tokio::test]
    async fn test_rmi() {
        let docker = docker_test!({
            use mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_clone().return_once(|| {
                let mut mock = MockDocker::new();

                mock.expect_remove_image().return_once(|_, _, _| Ok(vec![]));

                mock
            });

            mock
        });

        let result = rmi(&docker, &["test".to_string()], true).await;

        assert!(result.is_ok(), "ps failed with {:?}", result);
    }

    #[tokio::test]
    async fn test_events() {
        let docker = docker_test!({
            use mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_events()
                .return_once(|_| Box::pin(futures::stream::empty()));

            mock
        });

        let result = events(&docker, &[]).await;

        assert!(result.is_ok(), "events failed with {:?}", result);
    }
}
