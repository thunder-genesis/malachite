use futures_util::TryStreamExt;
use ipfs_embed::{Cid, Config, DefaultParams, Ipfs};
use tracing::debug;

#[derive(Debug, thiserror::Error)]
pub enum IpfsError {}

#[derive(Debug)]
pub struct IpfsClient;

impl IpfsClient {
    pub async fn get_file(&self, cid: impl AsRef<str>) -> Result<Vec<u8>, IpfsError> {
        let mut ipfs = Ipfs::<DefaultParams>::new(Config::default()).await.unwrap();
        // ipfs.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());
        let file_cid: Cid = cid.as_ref().parse().unwrap();
        let file_contents = ipfs.get(&file_cid).unwrap();
        debug!("IPFS file content: {:?}", file_contents);
        debug!("IPFS file as string: {:?}", file_contents.to_string());
        Ok(file_contents.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test() {
        let ipfs = IpfsClient;
        let cid = "cid";
        let file = ipfs.get_file(cid).await.unwrap();
        println!("File content: {:?}", String::from_utf8_lossy(&file));
    }
}
