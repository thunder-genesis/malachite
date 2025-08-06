use alloy::providers::ProviderBuilder;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{signature::Keypair as SolKeypair, signer::EncodableKey as _};

use crate::{
    alert::TgAlert,
    bootstrapper::Bootstrapper,
    cli::Cli,
    cloudflare::Cloudflare,
    eth_contract::{SubchainRegistry, SubchainRegistryImpl},
    openstack::Openstack,
    velas_network::VelasNetwork,
};

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Failed to create Telegram alert client: {0}")]
    TelegramAlertError(#[from] tgbot::api::ClientError),

    #[error("Failed to create Ethereum Contract provider: {0}")]
    EthContractError(#[from] alloy::transports::RpcError<alloy::transports::TransportErrorKind>),

    #[error("Failed to load Velas hotwallet: {0}")]
    VelasHotwallet(String),

    #[error("Failed to create Cloudflare context: {0}")]
    CloudflareError(#[from] cloudflare::framework::Error),

    #[error("Failed to read file `{file_name}`: {source}")]
    FailedToReadFile {
        file_name: String,
        #[source]
        source: std::io::Error,
    },
}

pub struct Context {
    pub tg_alert: TgAlert,
    pub eth_contract: SubchainRegistryImpl,
    pub vlx: VelasNetwork,
    pub cloudflare: Cloudflare,
    pub openstack: Openstack,
    pub bootstrapper: Bootstrapper,
}

impl Context {
    pub async fn new(cli: &Cli) -> Result<Self, ContextError> {
        let tg_alert = if let Some(tg_bot_token) = &cli.tg_bot_token {
            let chat_id = cli.tg_chat_id.as_deref().unwrap_or_default();
            TgAlert::new_client(tg_bot_token, chat_id)?
        } else {
            TgAlert::new_empty()
        };

        let eth_contract = {
            let eth_provider = ProviderBuilder::new()
                .wallet(cli.smc_signer.clone())
                .connect(&cli.smc_network_rpc)
                .await?;
            SubchainRegistry::new(cli.smc_address, eth_provider)
        };

        let velas_network = {
            let hotwallet = SolKeypair::read_from_file(cli.vlx_native_keypair.clone())
                .map_err(|e| ContextError::VelasHotwallet(e.to_string()))?;
            let client = RpcClient::new(cli.vlx_network_rpc.clone());
            VelasNetwork::new(
                client,
                hotwallet,
                cli.fund_subchain_owner,
                cli.fund_subchain_state,
            )
        };

        let openstack = Openstack::new(cli);

        let bootstrapper = {
            let ssh_private_key = std::fs::read_to_string(&cli.ssh_secret_key).map_err(|source| {
                ContextError::FailedToReadFile {
                    file_name: cli.ssh_secret_key.to_string_lossy().into_owned(),
                    source,
                }
            })?;
            let ssh_bootstrap_script = std::fs::read(&cli.ssh_bootstrap_script).map_err(|source| {
                ContextError::FailedToReadFile {
                    file_name: cli.ssh_bootstrap_script.to_string_lossy().into_owned(),
                    source,
                }
            })?;
            Bootstrapper::new(ssh_private_key, ssh_bootstrap_script)
        };

        let cloudflare = Cloudflare::new(&cli.cloudflare_api_token, &cli.domain)?;

        Ok(Self {
            tg_alert,
            eth_contract,
            vlx: velas_network,
            cloudflare,
            openstack,
            bootstrapper,
        })
    }
}
