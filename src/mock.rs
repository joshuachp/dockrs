use std::pin::Pin;

use async_trait::async_trait;
use bollard::{
    auth::DockerCredentials,
    container::{
        AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions,
        ListContainersOptions, LogOutput, LogsOptions, RemoveContainerOptions,
        StartContainerOptions, Stats, StatsOptions, StopContainerOptions,
    },
    errors::Error,
    image::{CreateImageOptions, RemoveImageOptions},
    models::{ContainerCreateResponse, CreateImageInfo},
    service::{ContainerSummary, ImageDeleteResponseItem},
};
use futures::Stream;
use hyper::Body;
use mockall::mock;

type DockerStream<T> = Pin<Box<dyn Stream<Item = Result<T, Error>> + Send>>;

#[async_trait]
pub trait DockerTrait: Sized {
    fn connect_with_local_defaults() -> Result<Self, Error>;
    async fn remove_container(
        &self,
        container_name: &str,
        options: Option<RemoveContainerOptions>,
    ) -> Result<(), Error>;

    async fn start_container<'a>(
        &self,
        container_name: &str,
        options: Option<StartContainerOptions<&'a str>>,
    ) -> Result<(), Error>;
    async fn create_container<'a, 'b>(
        &self,
        options: Option<CreateContainerOptions<&'a str>>,
        config: Config<&'b str>,
    ) -> Result<ContainerCreateResponse, Error>;
    async fn attach_container<'a>(
        &self,
        container_name: &str,
        options: Option<AttachContainerOptions<&'a str>>,
    ) -> Result<AttachContainerResults, Error>;
    fn create_image(
        &self,
        options: Option<CreateImageOptions<String>>,
        root_fs: Option<Body>,
        credentials: Option<DockerCredentials>,
    ) -> DockerStream<CreateImageInfo>;
    async fn list_containers<'a>(
        &self,
        options: Option<ListContainersOptions<&'a str>>,
    ) -> Result<Vec<ContainerSummary>, Error>;
    fn stats(&self, container_name: &str, options: Option<StatsOptions>) -> DockerStream<Stats>;
    async fn stop_container(
        &self,
        container_name: &str,
        options: Option<StopContainerOptions>,
    ) -> Result<(), Error>;
    fn logs(
        &self,
        container_name: &str,
        options: Option<LogsOptions<String>>,
    ) -> DockerStream<LogOutput>;
    async fn remove_image(
        &self,
        image_name: &str,
        options: Option<RemoveImageOptions>,
        credentials: Option<DockerCredentials>,
    ) -> Result<Vec<ImageDeleteResponseItem>, Error>;
}

mock! {
    #[derive(Debug)]
    pub Docker {}
    impl Clone for Docker {
        fn clone(&self) -> Self;
    }
    #[async_trait]
    impl DockerTrait  for Docker {
        fn connect_with_local_defaults() -> Result<Self, Error>;
        async fn remove_container(
            &self,
            container_name: &str,
            options: Option<RemoveContainerOptions>,
        ) -> Result<(), Error>;

        async fn start_container<'a>(
            &self,
            container_name: &str,
            options: Option<StartContainerOptions<&'a str>>,
        ) -> Result<(), Error>;
        async fn create_container<'a, 'b>(
            &self,
            options: Option<CreateContainerOptions<&'a str>>,
            config: Config<&'b str>,
        ) -> Result<ContainerCreateResponse, Error>;
        async fn attach_container<'a>(
            &self,
            container_name: &str,
            options: Option<AttachContainerOptions<&'a str>>,
        ) -> Result<AttachContainerResults, Error>;
        fn create_image(
            &self,
            options: Option<CreateImageOptions<String>>,
            root_fs: Option<Body>,
            credentials: Option<DockerCredentials>,
        ) -> DockerStream<CreateImageInfo>;
        async fn list_containers<'a>(
            &self,
            options: Option<ListContainersOptions<&'a str>>,
        ) -> Result<Vec<ContainerSummary>, Error>;
        fn stats(&self, container_name: &str, options: Option<StatsOptions>) -> DockerStream<Stats>;
        async fn stop_container(
            &self,
            container_name: &str,
            options: Option<StopContainerOptions>,
        ) -> Result<(), Error>;
        fn logs(
            &self,
            container_name: &str,
            options: Option<LogsOptions<String>>,
        ) -> DockerStream<LogOutput>;
        async fn remove_image(
            &self,
            image_name: &str,
            options: Option<RemoveImageOptions>,
            credentials: Option<DockerCredentials>,
        ) -> Result<Vec<ImageDeleteResponseItem>, Error>;
    }
}
