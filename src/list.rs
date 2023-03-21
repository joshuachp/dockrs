use std::{borrow::Cow, collections::HashMap, fmt::Display};

use bollard::{container::ListContainersOptions, service::ContainerSummary};
use chrono::{NaiveDateTime, Utc};
use color_eyre::Result;
use prettytable::{format::FormatBuilder, Row, Table};

#[cfg(feature = "mock")]
use crate::mock::{DockerTrait, MockDocker as Docker};
use crate::parse_filter;
#[cfg(not(feature = "mock"))]
use bollard::Docker;

#[derive(Debug, Default)]
struct Size {
    size: i64,
}

const BYTES: i64 = 1000;
const BYTES_END: i64 = BYTES + 1;
const KILOBYTES: i64 = 1000_i64.pow(2);
const KILOBYTES_END: i64 = KILOBYTES + 1;
const MEGABYTES: i64 = 1000_i64.pow(3);
const MEGABYTES_END: i64 = MEGABYTES + 1;
const GIGABYTES: i64 = 1000_i64.pow(4);
const GIGABYTES_END: i64 = GIGABYTES + 1;
const TERABYTES: i64 = 1000_i64.pow(5);

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.size {
            ..=BYTES => write!(f, "{}B", self.size),
            BYTES_END..=KILOBYTES => write!(f, "{:.2}kB", self.size as f64 / BYTES as f64),
            KILOBYTES_END..=MEGABYTES => write!(f, "{:.2}MB", self.size as f64 / KILOBYTES as f64),
            MEGABYTES_END..=GIGABYTES => write!(f, "{:.2}GB", self.size as f64 / MEGABYTES as f64),
            GIGABYTES_END..=TERABYTES => write!(f, "{:.2}TB", self.size as f64 / GIGABYTES as f64),
            _ => write!(f, "{:.2}PB", self.size / TERABYTES),
        }
    }
}

impl From<i64> for Size {
    fn from(size: i64) -> Self {
        Self { size }
    }
}

struct Stats<'a> {
    stats: Vec<Cow<'a, str>>,
}

impl<'a> From<&'a ContainerSummary> for Stats<'a> {
    fn from(value: &'a ContainerSummary) -> Self {
        let id = value
            .id
            .as_ref()
            .map(|id| id.chars().take(12).collect::<String>())
            .unwrap_or_default();

        let image = value.image.as_deref().unwrap_or_default();
        let command = value.command.as_deref().unwrap_or_default();

        let created = value
            .created
            .and_then(|time| NaiveDateTime::from_timestamp_opt(time, 0))
            .and_then(|time| match time.and_local_timezone(Utc) {
                chrono::LocalResult::Single(res) => Some(res),
                _ => None,
            })
            .map(|time| {
                let minutes = Utc::now().signed_duration_since(time).num_minutes();

                format!("{} minutes ago", minutes)
            })
            .unwrap_or_default();

        let status = value.status.as_deref().unwrap_or_default();

        let ports = value
            .ports
            .as_ref()
            .map(|ports| {
                ports
                    .iter()
                    .map(|port| {
                        let mut res = String::new();

                        if let Some(ip) = &port.ip {
                            res.push_str(ip);
                            res.push(':');
                        }

                        res.push_str(&port.private_port.to_string());
                        res.push(':');
                        res.push_str(&port.private_port.to_string());

                        res
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let names = value
            .names
            .as_ref()
            .map(|names| names.join(", "))
            .unwrap_or_default();

        let size_rw = value.size_rw.map(Size::from).unwrap_or_default();

        let size_root = value.size_root_fs.map(Size::from).unwrap_or_default();

        let size = format!("{} (virtual {})", size_rw, size_root);

        Self {
            stats: vec![
                Cow::from(id),
                Cow::from(image),
                Cow::from(command),
                Cow::from(created),
                Cow::from(status),
                Cow::from(ports),
                Cow::from(names),
                Cow::from(size),
            ],
        }
    }
}

pub async fn list(docker: &Docker, all: bool, size: bool, filters: &[String]) -> Result<()> {
    let filters: HashMap<&str, Vec<&str>> =
        filters
            .iter()
            .try_fold(HashMap::new(), |mut acc, filter| -> Result<_> {
                let (filter, value) = parse_filter(filter)?;

                acc.entry(filter).or_insert_with(Vec::new).push(value);

                Ok(acc)
            })?;

    let options = ListContainersOptions::<&str> {
        all,
        size,
        filters,
        ..Default::default()
    };

    let containers = docker.list_containers(Some(options)).await?;

    let mut headers = vec![
        "CONTAINER ID",
        "IMAGE",
        "COMMAND",
        "CREATED",
        "STATUS",
        "PORTS",
        "NAMES",
    ];

    if size {
        headers.push("SIZE");
    }

    let format = FormatBuilder::new()
        .column_separator(' ')
        .padding(0, 2)
        .build();

    let mut table = Table::new();
    table.set_format(format);
    table.add_row(Row::from(headers));

    for container in containers {
        let stats = Stats::from(&container);

        table.add_row(Row::from(stats.stats));
    }

    table.printstd();

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::docker_test;

    use super::*;

    #[test]
    fn test_size_display() {
        let size = Size::from(1125);

        let expected = "1.12kB";

        assert_eq!(size.to_string(), expected);
    }

    #[tokio::test]
    async fn test_list() -> Result<()> {
        let docker = docker_test!({
            use crate::mock::MockDocker;

            let mut mock = MockDocker::new();

            mock.expect_list_containers().returning(|_| Ok(vec![]));

            mock
        });

        list(&docker, false, true, &[]).await?;

        Ok(())
    }
}
