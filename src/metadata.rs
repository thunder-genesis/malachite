use tracing::{error, info};

use serde::{Deserialize, Serialize};
use url::Url;

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

#[derive(Debug)]
pub struct Metadata {
    // solidity flattened source code actually
    pub compiled_contract_source: Vec<u8>,
    pub logo_dark_base64: String,
    pub logo_light_base64: String,
    pub project_description: String,
    pub favicon_base64: String,
    pub explorer_background_base64: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("Failed to fetch Metadata: {0}")]
    Download(#[source] reqwest::Error),

    #[error("Failed to exctact body from Metadata response: {0}")]
    Body(#[source] reqwest::Error),

    #[error("Failed to parse Metadata JSON: {0}")]
    ParseJson(#[source] serde_json::Error),

    #[error("Solidity contract compilation failed with code {code:?}: {stderr}")]
    CompilationFailed { code: Option<i32>, stderr: String },

    #[error("Failed to create temporary directory: {0}")]
    TempDir(#[source] std::io::Error),

    #[error("Failed to write contract source code to temporary file: {0}")]
    WriteContract(#[source] std::io::Error),

    #[error("Failed to execute `forge` command: {0}")]
    ForgeExecution(#[source] std::io::Error),

    #[error("Failed to parse JSON compilation details: {0}")]
    CompilationJsonParse(#[source] serde_json::Error),

    #[error("Invalid compilation output format: unable to find deployed bytecode")]
    NoDeployedBytecode,

    #[error("Invalid compilation output format: deployed bytecode is not in hex")]
    DeployedBytecodeIsNotHex(#[source] alloy::hex::FromHexError),
}

#[derive(Debug)]
pub struct MetadataExtractor; // TODO: provide path for `forge` binary

impl MetadataExtractor {
    pub async fn extract_and_compile_metadata(&self, metadata_url: Url) -> Result<Metadata, MetadataError> {
        let metadata_json = self.download_metadata(metadata_url).await?;
        info!("Downloaded medatata:\n{metadata_json}");

        let metadata_parsed = self.deserialize_metadata(metadata_json).await?;
        let metadata_compiled = self.compile_metadata(metadata_parsed).await?;
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
        serde_json::from_str(&raw_json).map_err(MetadataError::ParseJson)
    }

    async fn compile_metadata(&self, raw_metadata: MetadataRaw) -> Result<Metadata, MetadataError> {
        let MetadataRaw {
            compiled_contract_source,
            logo_dark_base64,
            logo_light_base64,
            project_description,
            favicon_base64,
            explorer_background_base64,
        } = raw_metadata;

        info!("Writing solidity source code to temporary file");
        let tempdir = tempfile::tempdir().map_err(MetadataError::TempDir)?;
        let contract_path = tempdir.path().join("contract.sol");
        std::fs::write(&contract_path, &compiled_contract_source).map_err(MetadataError::WriteContract)?;
        let contract_path = contract_path.to_string_lossy().to_string();
        let dist = tempfile::tempdir().map_err(MetadataError::TempDir)?;
        let dist_path = dist.path().to_string_lossy().to_string();

        info!("Compiling solidity source code using `forge`");
        let output = std::process::Command::new("/usr/bin/forge")
            .args(&[
                "build",
                &contract_path,
                "--json",
                "--force",
                "--use",
                "0.8.19",
                "--optimize",
                "true",
                "--out",
                &dist_path,
            ])
            .output()
            .map_err(MetadataError::ForgeExecution)?;

        if !output.status.success() {
            error!("`forge build` exited with non-zero return code: {:?}", output);
            return Err(MetadataError::CompilationFailed {
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let compilation_details = String::from_utf8_lossy(&output.stdout).to_string();
        let compilation_details: serde_json::Value =
            serde_json::from_str(&compilation_details).map_err(MetadataError::CompilationJsonParse)?;

        // { "contracts": { "<contract_path>": { "<contract_name>": [ { "contract": { "evm": { "deployedBytecode": { "object": "608060...", .. }, ..}, .. }, ..}, ..] }, .. }, ..}
        let deployed_bytecode = compilation_details["contracts"][contract_path]
            .as_object()
            .and_then(|contracts| contracts.iter().next())
            .and_then(|(_, contract)| contract[0]["contract"]["evm"]["deployedBytecode"]["object"].as_str())
            .ok_or(MetadataError::NoDeployedBytecode)?;

        let compiled_contract_source =
            alloy::hex::decode(deployed_bytecode).map_err(MetadataError::DeployedBytecodeIsNotHex)?;

        let metadata = Metadata {
            compiled_contract_source,
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
    use std::process::ExitStatus;

    use super::*;

    #[tokio::test]
    async fn test() {
        let compiled_contract_source = r#"
pragma solidity >=0.8.2 <0.9.0;
contract Storage {
    uint256 number;
    function store(uint256 num) public {
        number = num;
    }
    function retrieve() public view returns (uint256){
        return number;
    }
}
        "#
        .to_string();

        let metadata = MetadataRaw {
            compiled_contract_source,
            ..Default::default()
        };

        let extractor = MetadataExtractor;
        let metadata = extractor.compile_metadata(metadata).await.unwrap();
        assert!(metadata.compiled_contract_source.starts_with(&[0x60, 0x80, 0x60]));
    }
}
