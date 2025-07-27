#[derive(Debug, askama::Template)]
#[template(path = "docker-compose.yml.jinja", escape = "none")]
pub struct DockerCompose {
    pub app_base_url: String,
    pub app_node_url: String,
}

impl DockerCompose {
    pub fn new(subdomain: impl AsRef<str>, domain: impl AsRef<str>) -> Self {
        let subdomain = subdomain.as_ref();
        let domain = domain.as_ref();
        Self {
            app_base_url: format!("https://{subdomain}.{domain}"),
            app_node_url: format!("https://{subdomain}.{domain}/rpc"),
        }
    }
}

// environment:
//   APP_BASE_URL: "https://my-subchain.velasocean.com"
//   APP_NODE_URL: "https://my-subchain.velasocean.com/rpc"
