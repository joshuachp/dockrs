use color_eyre::{eyre::Context, Result};
use futures::StreamExt;
use shiplift::{tty::TtyChunk, ContainerOptions, Docker};
use tracing::{error, trace};

pub async fn run(image: &str) -> Result<()> {
    let docker = Docker::new();

    trace!("Creating container from image: {}", image);

    let container_info = docker
        .containers()
        .create(&ContainerOptions::builder(image).auto_remove(true).build())
        .await
        .wrap_err("Failed to create container")?;

    trace!("Attaching to container: {}", container_info.id);

    let container = docker.containers().get(&container_info.id);

    let tty_multiplexer = container.attach().await?;

    container
        .start()
        .await
        .wrap_err("Failed to start container")?;

    let (mut reader, _writer) = tty_multiplexer.split();

    while let Some(tty_result) = reader.next().await {
        match tty_result {
            Ok(chunk) => print_chunk(chunk),
            Err(e) => error!("{}", e),
        }
    }

    trace!("Container exited: {}", container_info.id);

    Ok(())
}

fn print_chunk(chunk: TtyChunk) {
    match chunk {
        TtyChunk::StdOut(bytes) => println!("Stdout: {}", std::str::from_utf8(&bytes).unwrap()),
        TtyChunk::StdErr(bytes) => eprintln!("Stdout: {}", std::str::from_utf8(&bytes).unwrap()),
        TtyChunk::StdIn(_) => unreachable!(),
    }
}
