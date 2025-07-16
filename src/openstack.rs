use openstack_sdk::{
    AsyncOpenStack, OpenStackError,
    config::{Auth, CloudConfig},
};

use crate::cli::Cli;

#[derive(Debug)]
pub struct Openstack {
    client: AsyncOpenStack,
}

impl Openstack {
    pub async fn new(cli: &Cli) -> Result<Self, OpenStackError> {
        let config = CloudConfig {
            auth: Some(Auth {
                auth_url: Some(cli.os_auth_url.clone()),
                username: Some(cli.os_username.clone()),
                user_domain_name: Some(cli.os_user_domain_name.clone()),
                password: Some(cli.os_password.to_string().into()),
                project_name: Some(cli.os_project_name.clone()),
                project_domain_name: Some(cli.os_project_domain_name.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let client = AsyncOpenStack::new(&config).await?;
        Ok(Self { client })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_openstack_module() {
        let cli = Cli::mock();

        let openstack = Openstack::new(&cli).await;

        assert!(
            openstack.is_ok(),
            "Failed to create Openstack client: {:?}",
            openstack.err()
        );
    }
}
