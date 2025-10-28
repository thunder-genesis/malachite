use std::collections::BTreeMap;

use alloy::primitives::{Address, U256 as AlloyU256};
use primitive_types::{H160, H256, U256 as PrimitiveU256};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};
use url::Url;

use crate::subchain_transaction::AllocAccount;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataRaw {
    // solidity flattened source code actually
    pub compiled_contract_source: String,
    pub logo_dark_base64: String,
    pub logo_light_base64: String,
    pub project_description: String,
    pub favicon_base64: String,
    pub explorer_background_base64: String,
}

// TODO: use logos and project description
#[derive(Debug)]
#[allow(unused)]
pub struct Metadata {
    // solidity flattened source code actually
    pub alloc: BTreeMap<H160, AllocAccount>,
    pub logo_dark_base64: String,
    pub logo_light_base64: String,
    pub project_description: String,
    pub favicon_base64: String,
    pub explorer_background_base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
enum GenesisResponse {
    Ok {
        #[serde(rename = "genesisAlloc")]
        genesis_alloc: BTreeMap<H160, JsonAllocAccount>,
    },
    Error {
        message: String,
        logs: String,
    },
}

#[derive(Debug, Deserialize)]
struct JsonAllocAccount {
    code: String,
    storage: BTreeMap<H256, H256>,
    balance: PrimitiveU256,
    // we can use hatdcoded `1`
    // nonce: PrimitiveU256,
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("Failed to parse Metadata URL: {0}")]
    FailedToParseMetadataUrl(#[from] url::ParseError),

    #[error("Failed to fetch Metadata: {0}")]
    Download(#[source] reqwest::Error),

    #[error("Failed to exctact body from Metadata response: {0}")]
    Body(#[source] reqwest::Error),

    #[error("Failed to parse Metadata JSON: {0}")]
    ParseMetadataJson(#[source] serde_json::Error),

    #[error("Failed to parse compilation JSON: {0}")]
    ParseCompilationJson(#[source] serde_json::Error),

    #[error("Solidity contract compilation failed: {message}\n\n{logs}")]
    CompilationFailed { message: String, logs: String },

    #[error("Invalid compilation output format: deployed bytecode is not in hex")]
    DeployedBytecodeIsNotHex(#[source] alloy::hex::FromHexError),
}

#[derive(Debug)]
pub struct MetadataExtractor {
    genesis_mint: Url,
}

impl MetadataExtractor {
    pub fn new(genesis_mint: Url) -> Self {
        Self { genesis_mint }
    }

    pub async fn extract_and_compile_metadata(
        &self,
        metadata_url: String,
        coin_name: String,
        coin_symbol: String,
        initial_supply: AlloyU256,
        owner: Address,
    ) -> Result<Metadata, MetadataError> {
        let metadata_url = Url::parse(&metadata_url)?;
        let metadata_json = self.download_metadata(metadata_url).await?;
        info!("Downloaded medatata:\n{metadata_json}");

        let metadata_parsed = self.deserialize_metadata(metadata_json).await?;
        let metadata_compiled = self
            .compile_metadata(metadata_parsed, coin_name, coin_symbol, initial_supply, owner)
            .await?;
        Ok(metadata_compiled)
    }

    async fn download_metadata(&self, metadata_url: Url) -> Result<String, MetadataError> {
        info!("Downloading metadata from `{metadata_url}`");
        reqwest::get(metadata_url)
            .await
            .map_err(MetadataError::Download)?
            .text()
            .await
            .map_err(MetadataError::Body)
    }

    async fn deserialize_metadata(&self, raw_json: String) -> Result<MetadataRaw, MetadataError> {
        info!("Parsing metadata JSON");
        serde_json::from_str(&raw_json).map_err(MetadataError::ParseMetadataJson)
    }

    fn parse_genesis_response(&self, response: Value) -> Result<BTreeMap<H160, AllocAccount>, MetadataError> {
        let response = GenesisResponse::deserialize(response).map_err(MetadataError::ParseCompilationJson)?;
        let genesis_alloc = match response {
            GenesisResponse::Ok { genesis_alloc, .. } => genesis_alloc,
            GenesisResponse::Error { message, logs } => {
                error!("Compilation failed: {message}\n\n{logs}");
                return Err(MetadataError::CompilationFailed { message, logs });
            }
        };

        let mut result: BTreeMap<H160, AllocAccount> = BTreeMap::new();

        for (address, value) in genesis_alloc {
            let code = alloy::hex::decode(value.code).map_err(MetadataError::DeployedBytecodeIsNotHex)?;
            let alloc_account = AllocAccount {
                code,
                storage: value.storage,
                balance: value.balance,
                nonce: 1,
            };
            result.insert(address, alloc_account);
        }

        Ok(result)
    }

    async fn compile_metadata(
        &self,
        raw_metadata: MetadataRaw,
        name: String,
        symbol: String,
        initial_supply: AlloyU256,
        owner: Address,
    ) -> Result<Metadata, MetadataError> {
        let MetadataRaw {
            compiled_contract_source,
            logo_dark_base64,
            logo_light_base64,
            project_description,
            favicon_base64,
            explorer_background_base64,
        } = raw_metadata;
        let client = reqwest::Client::new();
        let request = client
            .post(self.genesis_mint.clone())
            .json(&json!({
                "contractSource": compiled_contract_source,
                "coinName": name,
                "coinSymbol": symbol,
                "initialSupply": initial_supply,
                "ownerAddress": owner
            }))
            .build()
            .unwrap();

        info!("Performing compilation request: {:?}", &request);

        let response: Value = client.execute(request).await.unwrap().json().await.unwrap();

        info!("Parsing compilation response: {response}");
        let genesis_alloc = self.parse_genesis_response(response).unwrap();

        let metadata = Metadata {
            alloc: genesis_alloc,
            logo_dark_base64,
            logo_light_base64,
            project_description,
            favicon_base64,
            explorer_background_base64,
        };

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {
        let response_ok = json!({
            "status": "ok",
            "genesisAlloc": {
                "0x0000000000000000000000000000000000000000": {
                    "nonce": "0x1",
                    "balance": "0x0",
                    "code": "0x604060",
                    "storage": {
                        "0x0000000000000000000000000000000000000000000000000000000000000002": "0x000000000000000000000000000000000000000000000000000000000001e240","0x0000000000000000000000000000000000000000000000000000000000000002": "0x000000000000000000000000000000000000000000000000000000000001e240",
                    }
                }
            },
            "logs": "foo"
        });

        let response_ok_deser = serde_json::from_value::<GenesisResponse>(response_ok.clone());

        assert!(matches!(response_ok_deser.unwrap(), GenesisResponse::Ok { .. }));

        let response_err = json!({
            "status": "error",
            "message": "an error occured",
            "logs": "foo"
        });

        let response_err_deser = serde_json::from_value::<GenesisResponse>(response_err.clone());

        assert!(matches!(
            response_err_deser.unwrap(),
            GenesisResponse::Error { .. }
        ));
    }
}
