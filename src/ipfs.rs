use futures_util::TryStreamExt;
use tracing::debug;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubchainMetadata {
    /// The flattened Solidity source code of your contract, all imports resolved into one file.
    compiled_contract_source: String,

    /// Base64-encoded dark mode logo image (data URI prefix required).
    logo_dark_base64: String,

    /// Base64-encoded light mode logo image (data URI prefix required).
    logo_light_base64: String,

    /// A brief description of the project.
    project_description: String,

    /// Base64-encoded favicon (data URI prefix required).
    favicon_base64: String,

    /// Base64-encoded explorer plate background image (data URI prefix required).
    explorer_background_base64: String,
}

#[derive(Debug, thiserror::Error)]
pub enum IpfsError {
    #[error("Failed to fetch file from IPFS")]
    Error,
}

#[derive(Debug)]
pub struct IpfsClient;

impl IpfsClient {
    pub async fn get_file(&self, cid: impl AsRef<str>) -> Result<(), IpfsError> {
        use ipfs_embed::{Cid, Config, DefaultParams, Ipfs, PeerId};
        let mut ipfs = Ipfs::<DefaultParams>::new(Config::default()).await.unwrap();
        ipfs.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());
        // let peer_id = PeerId::
        let file_cid: Cid = cid.as_ref().parse().unwrap();
        // let provider =
        //     PeerId::from_multihash("12D3KooWJ4N8bY3v1n7pYtU5rXo9rYv6H3Z2x5nXo5c5H8v6v9mS").unwrap();
        let file_contents = ipfs.fetch(&file_cid, vec![]).await.unwrap();
        let file_contents = ipfs.fetch(&file_cid, vec![]).await.unwrap();
        debug!("IPFS file content: {:?}", file_contents.data());
        // debug!("IPFS file as string: {:?}", file_contents.to_string());
        // let json = file_contents.to_string();
        // let metadata: SubchainMetadata = serde_json::from_str(&json).unwrap();
        Ok(())
    }

    pub async fn get_file2(&self, cid: impl AsRef<str>) -> Result<SubchainMetadata, IpfsError> {
        use futures_util::TryStreamExt;
        use ipfs_api::{IpfsApi, IpfsClient, request::ObjectGet, response::ObjectGetResponse};

        let mut ipfs = IpfsClient::default();
        ipfs.bootstrap_add_default().await.unwrap();

        let file = ipfs
            .get(&cid.as_ref())
            .map_ok(|chunk| {
                debug!("got chunk: {:?}", chunk);
                chunk.to_vec()
            })
            .try_concat()
            .await
            // .map_err(|e| {
            //     debug!("error: {}", e);
            //     IpfsError::Error
            // })
            .unwrap();

        // let result = ipfs.get(&cid.as_ref());
        // let block: ObjectGetResponse = ipfs.ob(&cid.as_ref()).await.unwrap();
        // let file = block.data;

        println!("got file: {:?}", file);
        // let object: ObjectGetResponse = serde_json::from_slice(&file).unwrap();
        // let file2 = object.data.into_bytes();
        // println!("got file2: {:?}", file2);
        let file = String::from_utf8(file).unwrap();
        println!("got file as string: {:?}", file);
        // ipfs.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());
        // let file_cid: Cid = cid.as_ref().parse().unwrap();
        // let file_contents = ipfs.get(&file_cid).unwrap();
        // debug!("IPFS file content: {:?}", file_contents);
        // debug!("IPFS file as string: {:?}", file_contents.to_string());
        // let json = file_contents.to_string();
        let metadata: SubchainMetadata = serde_json::from_str(&file).unwrap();
        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test() {
        let ipfs = IpfsClient;
        let cid = "QmWEPecyrEN91VaaruMwqUmuioJyL3df4ivzXsSH8cLhAR";
        let file = ipfs.get_file2(cid).await.unwrap();
        println!("File content: {:?}", file);
    }

    #[tokio::test]
    async fn test2() {
        use futures_util::TryStreamExt;
        use ipfs_api::{IpfsApi, IpfsClient, response::ObjectGetResponse};

        let mut ipfs = IpfsClient::default();
        ipfs.bootstrap_add_default().await.unwrap();
        let response = ipfs
            .object_get("/ipfs/QmWEPecyrEN91VaaruMwqUmuioJyL3df4ivzXsSH8cLhAR")
            .await
            .unwrap();
        println!("Response: {:?}", response);
    }
}
