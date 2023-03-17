use bollard::container::{
    AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions,
    StartContainerOptions,
};
use color_eyre::Result;
use futures::StreamExt;
use tracing::{instrument, warn};

#[cfg(feature = "mock")]
mod mock;

#[cfg(not(feature = "mock"))]
use bollard::Docker;
#[cfg(feature = "mock")]
use mock::{Docker as DockerTrait, MockDocker as Docker};

pub fn connect_to_docker() -> Result<Docker> {
    let docker = Docker::connect_with_local_defaults()?;

    Ok(docker)
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
}
