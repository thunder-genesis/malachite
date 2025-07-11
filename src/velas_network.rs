use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair as SolKeypair, Signature},
    signer::Signer,
    system_transaction::transfer,
    transaction::Transaction,
};
use tracing::info;

use crate::subchain_transaction::{SubchainConfig, create_evm_subchain_account};

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum VelasRpcError {
    #[error("Failed to get latest native blockhash: {0}")]
    GetBlockhash(String),

    #[error("Failed to fund Subchain owner account `{account}`: {error}")]
    FundSubchainOwner { account: Pubkey, error: String },

    #[error("Failed to create Subchain `{chain_id}`: {error}")]
    CreateSubchain { chain_id: u64, error: String },

    #[error("Failed to fund Subchain EVM State account `{account}`: {error}")]
    FundSubchainState { account: Pubkey, error: String },
}

pub struct VelasNetwork {
    client: RpcClient,
    hotwallet: SolKeypair,
    fund_subchain_owner: u64,
    fund_subchain_state: u64,
}

impl VelasNetwork {
    pub fn new(
        client: RpcClient,
        hotwallet: SolKeypair,
        fund_subchain_owner: u64,
        fund_subchain_state: u64,
    ) -> Self {
        Self {
            client,
            hotwallet,
            fund_subchain_owner,
            fund_subchain_state,
        }
    }

    pub async fn fund_subchain_owner(&self, account: &Pubkey) -> Result<Signature, VelasRpcError> {
        let recent_blockhash = self.get_latest_blockhash().await?;
        let signature = {
            let fund_subchain_owner = transfer(
                &self.hotwallet,
                account,
                self.fund_subchain_owner,
                recent_blockhash,
            );

            self.client
                .send_and_confirm_transaction(&fund_subchain_owner)
                .await
                .map_err(|e| VelasRpcError::FundSubchainOwner {
                    account: *account,
                    error: e.to_string(),
                })?
        };
        Ok(signature)
    }

    pub async fn create_subchain(
        &self,
        owner: &SolKeypair,
        chain_id: u64,
        config: SubchainConfig,
    ) -> Result<Signature, VelasRpcError> {
        let recent_blockhash = self.get_latest_blockhash().await?;
        let signature = {
            let ix = create_evm_subchain_account(owner.pubkey(), chain_id, config, None);
            let tx =
                Transaction::new_signed_with_payer(&[ix], Some(&owner.pubkey()), &[&owner], recent_blockhash);
            self.client.send_and_confirm_transaction(&tx).await.map_err(|e| {
                VelasRpcError::CreateSubchain {
                    chain_id,
                    error: e.to_string(),
                }
            })?
        };
        Ok(signature)
    }

    pub async fn fund_subchain_state(&self, subchain_state: Pubkey) -> Result<Signature, VelasRpcError> {
        let recent_blockhash = self.get_latest_blockhash().await?;
        let signature = {
            let fund_subchain_state = transfer(
                &self.hotwallet,
                &subchain_state,
                self.fund_subchain_state,
                recent_blockhash,
            );

            self.client
                .send_and_confirm_transaction(&fund_subchain_state)
                .await
                .map_err(|e| VelasRpcError::FundSubchainState {
                    account: subchain_state,
                    error: e.to_string(),
                })?
        };

        Ok(signature)
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, VelasRpcError> {
        info!("Receiving recent blockhash...");
        let recent_blockhash = self
            .client
            .get_latest_blockhash()
            .await
            .map_err(|e| VelasRpcError::GetBlockhash(e.to_string()))?;
        info!("Recent blockhash: {recent_blockhash}");
        Ok(recent_blockhash)
    }
}
