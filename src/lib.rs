use bollard::{
    container::{AttachContainerOptions, AttachContainerResults, Config},
    Docker,
};
use color_eyre::Result;
use futures::StreamExt;
use tracing::warn;

pub async fn run(image: &str) -> Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    let config = Config {
        image: Some(image),
        attach_stdout: Some(true),
        tty: Some(true),
        ..Default::default()
    };

    let container = docker.create_container::<&str, &str>(None, config).await?;

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

    docker.remove_container(&container.id, None).await?;

    Ok(())
}
