use std::net::{IpAddr, Ipv4Addr};

use openstack_sdk::api::QueryAsync as _;
use openstack_sdk::api::compute::v2::server::create_290 as create_api;
use openstack_sdk::{
    AsyncOpenStack, OpenStackError,
    config::{Auth, CloudConfig},
};
use tracing::{debug, error, info};

use crate::cli::Cli;

#[derive(Debug, thiserror::Error)]
pub enum CloudError {
    #[error("Failed to create OpenStack client: {0}")]
    CreateClient(#[source] OpenStackError),

    #[error("Failed to format OpenStack `networks` request: {0}")]
    NetworksBuilder(#[from] create_api::NetworksBuilderError),

    #[error("Failed to format OpenStack `server` request: {0}")]
    ServerBuilder(#[from] create_api::ServerBuilderError),

    #[error("Failed to format `instance` OpenStack request: {0}")]
    InstanceBuilder(#[from] create_api::RequestBuilderError),

    #[error("Failed to create OpenStack instance: {0}")]
    InstanceCreate(#[source] openstack_sdk::api::ApiError<openstack_sdk::RestError>),

    #[error("Instance details request returned unexpected output, no .id field: {0}")]
    MalformedInstanceDetailsResponse(String),

    #[error("Failed to format `deploying details` OpenStack request: {0}")]
    DetailsProgressBuilder(#[from] openstack_sdk::api::compute::v2::server::get::RequestBuilderError),

    #[error("Failed to fetch OpenStack instance details: {0}")]
    InstanceDetails(#[source] openstack_sdk::api::ApiError<openstack_sdk::RestError>),

    #[error("Instance deploy was initiated, but readiness check timed out")]
    Timeout,

    #[error("Failed to parse OpenStack instance details response: {0}")]
    MalformedInstanceDetails(#[from] serde_json::Error),

    #[error("Instance seemed to be deployed successfully, but has no IP address assigned")]
    ActiveInstanceHasNoAddress,

    #[error("Instance seemed to be deployed successfully, but has no IPv4 address assigned")]
    ActiveInstanceHasNoIPV4Address,
}

#[derive(Debug)]
pub struct Openstack {
    os_auth_url: String,
    os_username: String,
    os_user_domain_name: String,
    os_password: String,
    os_project_name: String,
    os_project_domain_name: String,
    os_network_id: String,
    os_image_id: String,
    os_flavor_id: String,
    os_ssh_pubkey_name: String,
}

impl Openstack {
    pub fn new(cli: &Cli) -> Self {
        let os_auth_url = cli.os_auth_url.clone();
        let os_username = cli.os_username.clone();
        let os_user_domain_name = cli.os_user_domain_name.clone();
        let os_password = cli.os_password.clone();
        let os_project_name = cli.os_project_name.clone();
        let os_project_domain_name = cli.os_project_domain_name.clone();
        let os_network_id = cli.os_network_id.clone();
        let os_image_id = cli.os_image_id.clone();
        let os_flavor_id = cli.os_flavor_id.clone();
        let os_ssh_pubkey_name = cli.os_ssh_pubkey_name.clone();

        Self {
            os_auth_url,
            os_username,
            os_user_domain_name,
            os_password,
            os_project_name,
            os_project_domain_name,
            os_network_id,
            os_image_id,
            os_flavor_id,
            os_ssh_pubkey_name,
        }
    }

    // TODO: split into creating instance and waiting for it
    /// Deploys an OpenStack instance with the specified name and returns its IP address.
    /// Typically, a delay is required for the instance to become available for ICMP and SSH.
    pub async fn deploy_openstack_instance(&self, instance_name: &str) -> Result<Ipv4Addr, CloudError> {
        info!("Deploying OpenStack instance `{instance_name}`...");

        let client = self.create_client().await?;

        info!("Building Instance creation request...");
        let networks = create_api::NetworksBuilder::default()
            .uuid(&self.os_network_id)
            .build()?;

        // TODO: discover ID's by human readable names
        let server = create_api::ServerBuilder::default()
            .image_ref(&self.os_image_id)
            .flavor_ref(&self.os_flavor_id)
            .name(instance_name)
            .networks(create_api::ServerNetworks::F1(vec![networks]))
            .key_name(&self.os_ssh_pubkey_name)
            .build()?;

        let instance = create_api::RequestBuilder::default().server(server).build()?;

        info!("Executing Instance Creation request...");
        let created: serde_json::Value = instance
            .query_async(&client)
            .await
            .map_err(CloudError::InstanceCreate)?;

        info!("Extracing newly created Instance ID...");
        let instance_id = created
            .as_object()
            .and_then(|o| o.get("id"))
            .and_then(|o| o.as_str())
            .ok_or(CloudError::MalformedInstanceDetailsResponse(created.to_string()))?;

        info!("Instance created with id `{instance_id}`");

        info!("Building Instance Details request...");
        let details_req = openstack_sdk::api::compute::v2::server::get::RequestBuilder::default()
            .id(instance_id)
            .build()?;

        let mut state = InstanceStatus::default();

        // TODO: use proper retry policy
        for n in 1..100 {
            info!("Requesting Instance details ({n})...");
            let instance_details: serde_json::Value = details_req
                .query_async(&client)
                .await
                .map_err(CloudError::InstanceDetails)?;

            debug!("Instance details: {}", instance_details);

            info!("Parsing Instance details...");
            let recent_state: InstanceStatus = serde_json::from_value(instance_details)?;

            if recent_state != state {
                state = recent_state.clone();

                info!(
                    "Instance state: {}. Task state: {:?}",
                    state.vm_state, state.task_state
                );

                if state.status == "ACTIVE" {
                    let addresses = state
                        .clone()
                        .addresses
                        .public
                        .ok_or(CloudError::ActiveInstanceHasNoAddress)?;

                    for a in addresses {
                        match a.addr {
                            IpAddr::V4(ip) => return Ok(ip),
                            _ => continue,
                        }
                    }

                    return Err(CloudError::ActiveInstanceHasNoIPV4Address);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }

        Err(CloudError::Timeout)
    }

    async fn create_client(&self) -> Result<AsyncOpenStack, CloudError> {
        info!("Creating OpenStack client...");
        let config = CloudConfig {
            auth: Some(Auth {
                auth_url: Some(self.os_auth_url.clone()),
                username: Some(self.os_username.clone()),
                user_domain_name: Some(self.os_user_domain_name.clone()),
                password: Some(self.os_password.to_string().into()),
                project_name: Some(self.os_project_name.clone()),
                project_domain_name: Some(self.os_project_domain_name.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };

        AsyncOpenStack::new(&config)
            .await
            .map_err(CloudError::CreateClient)
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, serde::Deserialize)]
struct InstanceStatus {
    #[serde(rename = "OS-EXT-STS:vm_state")]
    pub vm_state: String,
    #[serde(rename = "OS-EXT-STS:task_state")]
    pub task_state: Option<String>,
    pub status: String,
    pub addresses: Addresses,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, serde::Deserialize)]
struct Addresses {
    pub public: Option<Vec<Address>>,
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize)]
struct Address {
    pub addr: IpAddr,
}

// let flavors_req = openstack_sdk::api::compute::v2::flavor::list::RequestBuilder::default()
//     .build()
//     .unwrap();
// let flavors: serde_json::Value = flavors_req.query_async(&openstack.client).await.unwrap();

// let images_req = openstack_sdk::api::image::v2::image::list::RequestBuilder::default()
//     .build()
//     .unwrap();
// let images: Result<serde_json::Value, _> = images_req.query_async(&openstack.client).await;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "this is not a test"]
    async fn test_openstack_module() {
        let cli = Cli::mock();
        let _ = tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        let openstack = Openstack::new(&cli);

        let result = openstack.deploy_openstack_instance("test1").await;

        assert!(result.is_ok());

        info!("IP address: {}", result.unwrap());
    }
}
