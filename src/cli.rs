use alloy::{primitives::Address, signers::local::PrivateKeySigner};
use clap::Parser;
use std::{net::SocketAddrV4, path::PathBuf};

#[derive(Parser, veil::Redact)]
pub struct Cli {
    /// Bind server address
    #[arg(long, env, value_name = "SOCK_ADDR", default_value = "0.0.0.0:1987")]
    pub bind_address: SocketAddrV4,

    /// Subchain Manager Contract Ethereum RPC URL endpoint
    #[arg(long, env, value_name = "URL", value_hint = clap::ValueHint::Url, default_value = "http://127.0.0.1:8545")]
    pub smc_network_rpc: String,

    /// Subchain Manager Contract Address
    #[arg(long, env, value_name = "PUBLIC_KEY")]
    pub smc_address: Address,

    /// Private Key for interracting with SMC
    #[arg(
        long,
        env,
        default_value = "0x0101010101010101010101010101010101010101010101010101010101010101",
        value_name = "PRIVATE_KEY"
    )]
    #[redact(fixed = 8)]
    pub smc_signer: PrivateKeySigner,

    #[arg(long, env, value_name = "URL", value_hint = clap::ValueHint::Url, default_value = "http://127.0.0.1:8899")]
    pub vlx_network_rpc: String,

    /// Hot wallet for deploying subchains and funding fee accounts
    #[arg(
        long,
        env,
        value_name = "JSON_FILEPATH",
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
}
