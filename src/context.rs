use alloy::providers::ProviderBuilder;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{signature::Keypair as SolKeypair, signer::EncodableKey as _};

use crate::{
    alert::TgAlert,
    cli::Cli,
    cloudflare::Cloudflare,
    eth_contract::{SubchainDB, SubchainDBImpl},
    velas_network::VelasNetwork,
};

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("")]
    TelegramAlertError,
    #[error("")]
    EthContractError,
    #[error("")]
    VelasNetworkError,
}

pub struct Context {
    pub tg_alert: TgAlert,
    pub eth_contract: SubchainDBImpl,
    pub velas_network: VelasNetwork,
    pub cloudflare: Cloudflare,
}

impl Context {
    pub async fn new(cli: &Cli) -> Result<Self, ContextError> {
        let tg_alert = if let Some(tg_bot_token) = &cli.tg_bot_token {
            let chat_id = cli.tg_chat_id.as_deref().unwrap_or_default();
            TgAlert::new_client(tg_bot_token, chat_id).map_err(|_e| ContextError::TelegramAlertError)?
        } else {
            TgAlert::new_empty()
        };

        let eth_contract = {
            let eth_provider = ProviderBuilder::new()
                .wallet(cli.smc_signer.clone())
                .connect(&cli.smc_network_rpc)
                .await
                .map_err(|_e| ContextError::EthContractError)?;
            SubchainDB::new(cli.smc_address, eth_provider)
        };

        let velas_network = {
            let hotwallet = SolKeypair::read_from_file(cli.vlx_native_keypair.clone())
                .map_err(|_e| ContextError::VelasNetworkError)?;
            let client = RpcClient::new(cli.vlx_network_rpc.clone());
            VelasNetwork::new(
                client,
                hotwallet,
                cli.fund_subchain_owner,
                cli.fund_subchain_state,
            )
        };

        let cloudflare = Cloudflare {
            api_token: cli.cloudflare_api_token.clone(),
            domain: cli.domain.clone(),
        };

        Ok(Self {
            tg_alert,
            eth_contract,
            velas_network,
            cloudflare,
        })
    }
}
