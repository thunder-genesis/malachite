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
    #[error("Get Blockhash RPC request failed: {0}")]
    GetBlockhashError(String),

    #[error("Failed to fund subchain owner account {account}: {error}")]
    FundSubchainOwnerError { account: Pubkey, error: String },

    #[error("Failed to create subchain {chain_id}: {error}")]
    CreateSubchainError { chain_id: u64, error: String },

    #[error("Failed to fund subchain state account {account}: {error}")]
    FundSubchainStateError { account: Pubkey, error: String },
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
                .map_err(|e| VelasRpcError::FundSubchainOwnerError {
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
                VelasRpcError::CreateSubchainError {
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
                .map_err(|e| VelasRpcError::FundSubchainStateError {
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
            .map_err(|e| VelasRpcError::GetBlockhashError(e.to_string()))?;
        info!("Recent blockhash: {recent_blockhash}");
        Ok(recent_blockhash)
    }
}
