use std::sync::Arc;

use bollard::{
    container::{
        AttachContainerOptions, AttachContainerResults, Config, ListContainersOptions, Stats,
        StatsOptions,
    },
    image::CreateImageOptions,
    Docker,
};
use color_eyre::{eyre::ContextCompat, Result};
use futures::StreamExt;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, trace, warn};

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

#[instrument(skip(docker))]
async fn recv_stats(
    docker: &Docker,
    id: &str,
    options: StatsOptions,
    stat: Arc<Mutex<Option<Stats>>>,
) -> Result<()> {
    let mut stream = docker.stats(id, Some(options));

    debug!("Waiting for first stats item");

    while let Some(item) = stream.next().await {
        trace!(?item);

        stat.lock().await.replace(item?);
    }

    Ok(())
}

pub async fn stats() -> Result<()> {
    let docker = connect_to_docker()?;

    let options = ListContainersOptions::<&str> {
        all: true,
        ..Default::default()
    };

    let containers = docker.list_containers(Some(options)).await?;

    info!("Found {} containers", containers.len());

    let mut stats: Vec<Arc<Mutex<Option<Stats>>>> = Vec::with_capacity(containers.len());

    let options = StatsOptions {
        stream: true,
        ..Default::default()
    };

    for container in containers {
        let id = container.id.wrap_err("Container without id")?;
        let stat: Arc<Mutex<Option<Stats>>> = Arc::new(Mutex::new(None));

        stats.push(stat.clone());

        let dc_clone = docker.clone();

        debug!("Spawning stats task for {}", id);

        tokio::spawn(async move { recv_stats(&dc_clone, &id, options, stat).await });
    }

    debug!("Starting stats loop");

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        interval.tick().await;

        // TODO: This is not the best way to clear the screen
        print!("\x1B[2J\x1B[1;1H");

        for stat in &stats {
            let stat = stat.lock().await;

            if let Some(stat) = &*stat {
                println!("{:?}", stat);
            }
        }
    }
}
