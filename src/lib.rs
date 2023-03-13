use bollard::{
    container::{AttachContainerOptions, AttachContainerResults, Config},
    image::CreateImageOptions,
    Docker,
};
use color_eyre::Result;
use futures::StreamExt;
use tracing::warn;

fn connect_to_docker() -> Result<Docker> {
    let docker = Docker::connect_with_local_defaults()?;

    Ok(docker)
}

pub async fn run(image: &str) -> Result<()> {
    let docker = connect_to_docker()?;

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
