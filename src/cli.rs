use alloy::{primitives::Address, signers::local::PrivateKeySigner};
use clap::Parser;
use std::{net::SocketAddrV4, path::PathBuf};

#[derive(Parser, veil::Redact)]
pub struct Cli {
    /// Bind server address
    #[arg(long, env, value_name = "SOCK_ADDR", default_value = "0.0.0.0:1987")]
    pub bind_address: SocketAddrV4,

    /// Subchain Registry Contract Ethereum RPC URL HTTP(S) endpoint
    #[arg(long, env, value_name = "URL", value_hint = clap::ValueHint::Url, default_value = "http://127.0.0.1:8545")]
    pub registry_network_rpc: String,

    /// Subchain Registry Contract Address
    #[arg(long, env, value_name = "PUBLIC_KEY")]
    pub registry_address: Address,

    /// Private Key for interracting with Subchain Registry
    #[arg(
        long,
        env,
        default_value = "0x0101010101010101010101010101010101010101010101010101010101010101",
        value_name = "PRIVATE_KEY"
    )]
    #[redact(fixed = 8)]
    pub registry_signer: PrivateKeySigner,

    /// Velas Node RPC URL used by this service
    #[arg(long, env, value_name = "URL", value_hint = clap::ValueHint::Url, default_value = "http://127.0.0.1:8899")]
    pub vlx_network_rpc: String,

    /// Velas Node RPC URL used by subchain bridge
    #[arg(long, env, value_name = "URL", value_hint = clap::ValueHint::Url, default_value = "http://127.0.0.1:8899")]
    pub vlx_network_for_bridge: String,

    /// Hot wallet for deploying subchains and funding fee accounts
    #[arg(
        long,
        env,
        value_name = "JSONFILE",
        value_hint = clap::ValueHint::FilePath,
        default_value = "test/DBAFnAjY7EucVizMaguyXK2N3HyaWNyVcNqBYeRPd1JP.json"
    )]
    pub vlx_native_keypair: PathBuf,

    /// Telegram Bot Token for notifications
    #[arg(long, env, value_name = "TOKEN")]
    #[redact(fixed = 8)]
    pub tg_bot_token: Option<String>,

    /// Telegram Chat ID for notifications
    #[arg(long, env, value_name = "NUM")]
    pub tg_chat_id: Option<String>,

    /// Fund newly created subchain owner account with this amount of lamports
    #[arg(
        long,
        env,
        value_name = "LAMPORTS",
        default_value = "1000001000000000" // 1_000_001 vlx
    )]
    pub fund_subchain_owner: u64,

    /// Fund newly created subchain state with this amount of lamports
    #[arg(
        long,
        env,
        value_name = "LAMPORTS",
        default_value = "10000000000000" // 10_000 vlx
    )]
    pub fund_subchain_state: u64,

    /// Cloudflare API token for DNS management
    #[arg(long, env, value_name = "API_TOKEN")]
    #[redact(fixed = 8)]
    pub cloudflare_api_token: String,

    /// Cloudflare registered domain umbrella for subchains
    #[arg(long, env, value_name = "DOMAIN", default_value = "velasocean.com")]
    pub domain: String,

    /// Openstack Auth URL
    #[arg(long, env, value_name = "URL")]
    pub os_auth_url: String,

    /// Openstack Project Name
    #[arg(long, env, value_name = "STRING")]
    pub os_project_name: String,

    /// Openstack Username
    #[arg(long, env, value_name = "STRING")]
    pub os_username: String,

    /// Openstack Password
    #[arg(long, env, value_name = "STRING")]
    #[redact(fixed = 8)]
    pub os_password: String,

    /// Openstack User Domain Name
    #[arg(long, env, value_name = "STRING", default_value = "Default")]
    pub os_user_domain_name: String,

    /// Openstack Project Domain Name
    #[arg(long, env, value_name = "STRING", default_value = "Default")]
    pub os_project_domain_name: String,

    /// OpenStack Network ID to which the instance will be connected
    #[arg(long, env, value_name = "STRING")]
    pub os_network_id: String,

    /// OpenStack Instance Operating System Image ID
    #[arg(long, env, value_name = "STRING")]
    pub os_image_id: String,

    /// OpenStack Instance Hardware Specification ID (CPU, RAM, DISK, etc.)
    #[arg(long, env, value_name = "STRING")]
    pub os_flavor_id: String,

    /// OpenStack SSH Public Key ID
    #[arg(long, env, value_name = "STRING")]
    pub os_ssh_pubkey_name: String,

    /// Path to SSH secret key PEM file for bootstrapping the OpenStack instance
    #[arg(long, env, value_name = "PEMFILE", value_hint = clap::ValueHint::FilePath)]
    pub ssh_secret_key: PathBuf,

    /// Path to SSH bootstrap script
    #[arg(long, env, value_name = "BASHFILE", value_hint = clap::ValueHint::FilePath)]
    pub ssh_bootstrap_script: PathBuf,

    /// Path to `forge` executable
    #[arg(long, env, value_name = "BIN", value_hint = clap::ValueHint::FilePath)]
    pub forge_executable: PathBuf,
}

#[cfg(test)]
impl Cli {
    pub fn mock() -> Self {
        Self::parse_from(include_str!("../test/cli.txt").split('\n'))
    }
}
