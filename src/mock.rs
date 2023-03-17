use async_trait::async_trait;
use bollard::{
    container::{
        AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions,
        RemoveContainerOptions, StartContainerOptions,
    },
    errors::Error,
    models::ContainerCreateResponse,
};
use mockall::automock;

#[automock]
#[async_trait]
pub trait Docker: Sized {
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
}
