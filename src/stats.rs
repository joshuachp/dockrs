use std::{collections::HashMap, io::stdout, sync::Arc};

use bollard::{
    container::{ListContainersOptions, Stats, StatsOptions},
    service::ContainerSummary,
};
use color_eyre::{eyre::ContextCompat, Result};
use crossterm::{
    cursor::MoveToRow,
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use tokio::{signal::ctrl_c, sync::Mutex};
use tracing::{debug, error, info, instrument, trace};

#[cfg(feature = "mock")]
use crate::mock::{DockerTrait, MockDocker as Docker};
#[cfg(not(feature = "mock"))]
use bollard::Docker;

struct Containers {
    stats: HashMap<String, Arc<Mutex<Option<Stats>>>>,
}

impl Containers {
    fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    async fn update(
        &mut self,
        docker: &Docker,
        options: StatsOptions,
        containers: &[ContainerSummary],
    ) -> Result<()> {
        for container in containers {
            let id = container.id.as_deref().wrap_err("Conainer without id")?;

            if self.stats.contains_key(id) {
                continue;
            }

            let stat: Arc<Mutex<Option<Stats>>> = Arc::new(Mutex::new(None));

            self.stats.insert(id.to_string(), stat.clone());

            let dc_clone = docker.clone();
            let id_c = id.to_string();

            tokio::spawn(async move {
                if let Err(err) = recv_stats(&dc_clone, &id_c, options, stat).await {
                    error!(?err, "Error while receiving stats for container {}", id_c);
                }
            });
        }

        Ok(())
    }
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

    stat.lock().await.take();

    Ok(())
}

#[instrument]
pub async fn stats(docker: &Docker, keep_screen: bool) -> Result<()> {
    debug!("Intializing stats");

    let stats = initialize_stats(docker).await?;

    debug!("Starting stats loop");

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

    if !keep_screen {
        execute!(stdout(), EnterAlternateScreen)?;

        tokio::spawn(async move {
            ctrl_c().await.unwrap();
            execute!(stdout(), LeaveAlternateScreen).unwrap();
            std::process::exit(0);
        });
    }

    loop {
        if !keep_screen {
            execute!(stdout(), Clear(ClearType::All), MoveToRow(0))?;
        }

        println!("Container ID\tName\tCPU\tMemory\tNetwork\tBlock I/O");

        for stat in stats.lock().await.stats.values() {
            let stat_guard = stat.lock().await;

            let stat = if let Some(stat) = stat_guard.as_ref() {
                stat
            } else {
                continue;
            };

            let id = stat.id.as_str();
            let name = stat.name.as_str();

            let cpu = stat.cpu_stats.cpu_usage.total_usage;

            let memory = stat
                .memory_stats
                .usage
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".to_string());

            let net = stat
                .network
                .map(|s| s.rx_bytes.to_string())
                .unwrap_or_else(|| "-".to_string());

            let read = stat
                .storage_stats
                .read_size_bytes
                .map_or_else(|| "-".to_string(), |s| s.to_string());
            let write = stat
                .storage_stats
                .write_size_bytes
                .map_or_else(|| "-".to_string(), |s| s.to_string());

            println!(
                "{}\t{}\t{}\t{}\t{}\t{}/{}",
                id, name, cpu, memory, net, read, write
            );
        }

        interval.tick().await;
    }
}

#[instrument]
async fn initialize_stats(docker: &Docker) -> Result<Arc<Mutex<Containers>>> {
    let stats = Arc::new(Mutex::new(Containers::new()));

    let list_options = ListContainersOptions::<&str> {
        all: true,
        ..Default::default()
    };

    let stats_options = StatsOptions {
        stream: true,
        ..Default::default()
    };

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    let stats_clone = stats.clone();

    let docker = docker.clone();
    tokio::spawn(async move {
        let span = tracing::trace_span!("update_containers");
        let _enter = span.enter();

        loop {
            let containers = match docker.list_containers(Some(list_options.clone())).await {
                Ok(containers) => containers,
                Err(e) => {
                    error!(?e, "Failed to list containers");

                    interval.tick().await;
                    continue;
                }
            };

            info!("Found {} containers", containers.len());

            let res = stats_clone
                .lock()
                .await
                .update(&docker, stats_options, &containers)
                .await;

            if let Err(e) = res {
                error!(?e, "Failed to update containers");
            }

            interval.tick().await;
        }
    });

    Ok(stats)
}

#[cfg(test)]
mod test {
    use crate::docker_test;

    use super::*;

    #[tokio::test]
    async fn test_stats() {
        let docker = docker_test!({
            let mut mock = Docker::new();

            mock.expect_clone().returning(|| {
                let mut docker = Docker::new();

                docker.expect_list_containers().returning(|_| {
                    Ok(vec![ContainerSummary {
                        id: Some("id".to_string()),
                        ..Default::default()
                    }])
                });

                docker.expect_clone().returning(|| {
                    let mut docker = Docker::new();
                    docker
                        .expect_stats()
                        .returning(|_, _| futures::stream::empty().boxed());

                    docker
                });

                docker
            });

            mock
        });

        let join = tokio::spawn(async move {
            let res = stats(&docker, true).await;

            assert!(res.is_ok());
        });

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        join.abort();

        let err = join.await.unwrap_err();

        assert!(err.is_cancelled());
    }
}
