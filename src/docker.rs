#[derive(Debug, askama::Template)]
#[template(path = "docker-compose.yml.jinja", escape = "none")]
pub struct DockerCompose {
    pub app_base_url: String,
    pub app_node_url: String,
    pub subchain_id: u64,
    pub keypair: String,
    pub velas_rpc_url: String,
    pub bridge_bind_address: String,
}

impl DockerCompose {
    pub fn new(
        subdomain: impl AsRef<str>,
        domain: impl AsRef<str>,
        subchain_id: u64,
        velas_rpc_url: impl AsRef<str>,
        bridge_bind_address: impl AsRef<str>,
    ) -> Self {
        let subdomain = subdomain.as_ref();
        let domain = domain.as_ref();
        Self {
            app_base_url: format!("https://{subdomain}.{domain}"),
            app_node_url: format!("https://{subdomain}.{domain}/rpc"),
            subchain_id,
            keypair: "/opt/bridge/keypair.json".to_string(),
            velas_rpc_url: velas_rpc_url.as_ref().to_string(),
            bridge_bind_address: bridge_bind_address.as_ref().to_string(),
        }
    }
}

// environment:
//   APP_BASE_URL: "https://my-subchain.velasocean.com"
//   APP_NODE_URL: "https://my-subchain.velasocean.com/rpc"
