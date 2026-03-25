#[derive(Debug, askama::Template)]
#[template(path = "docker-compose.yml.jinja", escape = "none")]
pub struct DockerCompose {
    app_base_url: String,
    app_node_url: String,
    subchain_id: u64,
    keypair: String,
    velas_rpc_url: String,
    bridge_bind_address: String,
    exposing_port: String,
}

impl DockerCompose {
    pub fn new(
        subdomain: impl AsRef<str>,
        domain: impl AsRef<str>,
        subchain_id: u64,
        velas_rpc_url: impl AsRef<str>,
    ) -> Self {
        let subdomain = subdomain.as_ref();
        let domain = domain.as_ref();
        Self {
            app_base_url: format!("https://{subdomain}.{domain}/explorer"),
            app_node_url: format!("https://{subdomain}.{domain}/rpc"),
            subchain_id,
            keypair: "/opt/bridge/keypair.json".to_string(),
            velas_rpc_url: velas_rpc_url.as_ref().to_string(),
            bridge_bind_address: "0.0.0.0:8545".to_string(),
            exposing_port: "8545:8545".to_string(),
        }
    }
}

// environment:
//   APP_BASE_URL: "https://my-subchain.velasocean.com"
//   APP_NODE_URL: "https://my-subchain.velasocean.com/rpc"
