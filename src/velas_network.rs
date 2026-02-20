use borsh::BorshSerialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash,
    packet::PACKET_DATA_SIZE,
    pubkey::Pubkey,
    signature::{Keypair as SolKeypair, Signature},
    signer::Signer,
    system_instruction::create_account,
    system_transaction::transfer,
    transaction::Transaction,
};
use tracing::info;

use crate::subchain_transaction::{
    ExtendedConfig, SubchainConfig, big_tx_allocate, big_tx_write, create_evm_subchain_account,
};

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

    #[error("Failed to serialize Subchain `{0}` config into Borsh message: {1}")]
    SerializeError(u64, String),

    #[error("Failed to allocate big transaction storage for Subchain `{chain_id}`: {error}")]
    AllocateBigTxStorage { chain_id: u64, error: String },

    #[error("Failed to write big transaction data for Subchain `{chain_id}`: {error}")]
    WriteBigTxStorage { chain_id: u64, error: String },
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
        let ix = create_evm_subchain_account(owner.pubkey(), chain_id, config.clone(), None);
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&owner.pubkey()), &[&owner], recent_blockhash);

        info!("Determining if Subchain `{chain_id}` config size...");
        let result = if tx.message_data().len() > PACKET_DATA_SIZE {
            info!("Subchain `{chain_id}` config needs big transaction storage...");
            let (extended_config, config) = ExtendedConfig::split(config);
            let mut data = vec![];
            extended_config
                .serialize(&mut data)
                .map_err(|e| VelasRpcError::SerializeError(chain_id, e.to_string()))?;

            let storage = SolKeypair::new();
            info!(
                "Generated big transaction storage for Subchain `{chain_id}`: {}",
                storage.pubkey()
            );

            let recent_blockhash = self.get_latest_blockhash().await?;
            info!("Allocating big transaction storage");
            let create_storage_ix = create_account(
                &owner.pubkey(),
                &storage.pubkey(),
                10000000000,
                data.len() as u64,
                &crate::subchain_transaction::EVM_LOADER_ID,
            );

            let big_tx_alloc = big_tx_allocate(storage.pubkey(), data.len());
            let tx = Transaction::new_signed_with_payer(
                &[create_storage_ix, big_tx_alloc],
                Some(&owner.pubkey()),
                &[&owner, &storage],
                recent_blockhash,
            );
            self.client.send_and_confirm_transaction(&tx).await.map_err(|e| {
                VelasRpcError::AllocateBigTxStorage {
                    chain_id,
                    error: e.to_string(),
                }
            })?;

            info!("Writing big transaction storage...");

            const TX_MTU: usize = 908;
            let write_txs = data
                .chunks(TX_MTU)
                .enumerate()
                .map(|(i, chunk)| big_tx_write(storage.pubkey(), (i * TX_MTU) as u64, chunk.to_vec()))
                .collect::<Vec<_>>();

            let chunks_count = write_txs.len();
            info!("Writing big transaction in {} chunks...", chunks_count);
            let recent_blockhash = self.get_latest_blockhash().await?;
            for (tx_id, ix) in write_txs.into_iter().enumerate() {
                info!("[{}/{}] Writing chunk...", tx_id + 1, chunks_count);
                let tx = Transaction::new_signed_with_payer(
                    &[ix],
                    Some(&owner.pubkey()),
                    &[&owner, &storage],
                    recent_blockhash,
                );
                self.client
                    .send_transaction(&tx)
                    .await
                    .map_err(|e| VelasRpcError::WriteBigTxStorage {
                        chain_id,
                        error: e.to_string(),
                    })?;
            }

            info!("Creating Subchain `{chain_id}`...");
            let recent_blockhash = self.get_latest_blockhash().await?;
            let ix = create_evm_subchain_account(owner.pubkey(), chain_id, config, Some(storage.pubkey()));
            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&owner.pubkey()),
                &[&owner, &storage],
                recent_blockhash,
            );

            self.client.send_and_confirm_transaction(&tx).await
        } else {
            self.client.send_and_confirm_transaction(&tx).await
        };

        result.map_err(|e| VelasRpcError::CreateSubchain {
            chain_id,
            error: e.to_string(),
        })
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
