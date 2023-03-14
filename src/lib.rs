use bollard::{
    container::{AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions},
    Docker,
};
use color_eyre::Result;
use futures::StreamExt;
use tracing::{instrument, warn};

#[instrument]
pub async fn run(
    options: Option<CreateContainerOptions<&str>>,
    config: Config<&str>,
    rm: bool,
) -> Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    let container = docker
        .create_container::<&str, &str>(options, config)
        .await?;

    if !container.warnings.is_empty() {
        warn!("Warnings while creating the container");
        for warning in container.warnings {
            warn!(?warning);
        }
    }

    let options = AttachContainerOptions {
        stream: Some(true),
        stdin: Some(false),
        stdout: Some(true),
        stderr: Some(true),
        ..Default::default()
    };

    let AttachContainerResults { mut output, .. } = docker
        .attach_container::<&str>(&container.id, Some(options))
        .await?;

    let join = tokio::spawn(async move {
        while let Some(chunk) = output.next().await {
            print!("{}", chunk.unwrap());
        }
    });

    docker.start_container::<&str>(&container.id, None).await?;

    join.await?;

    if rm {
        docker.remove_container(&container.id, None).await?;
    }

    Ok(())
}
