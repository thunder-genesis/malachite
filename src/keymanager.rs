use solana_sdk::{
    pubkey::Pubkey, signature::EncodableKey, signature::Keypair as SolKeypair, signer::Signer as _,
};
use std::{env::temp_dir, fs::File, io::Write, path::PathBuf};
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum KeypairManagerError {
    #[error("Can't write velas keypair to file: {0}")]
    WriteToFile(#[from] Box<dyn std::error::Error + 'static>),

    #[error("Can't open keypair file for shredding: {0}")]
    OpenForShred(#[source] std::io::Error),

    #[error("Can't read keypair file metadata: {0}")]
    ReadMetadata(#[source] std::io::Error),

    #[error("Can't overwrite keypair file: {0}")]
    Overwrite(#[source] std::io::Error),

    #[error("Can't delete shredded keypair file: {0}")]
    DeleteKeypair(#[source] std::io::Error),
}

fn keypair_path(pubkey: &Pubkey) -> PathBuf {
    temp_dir().join(format!("subchain-owner-{}.json", pubkey))
}

pub struct KeypairManager;

impl KeypairManager {
    pub fn create_key(&self) -> Result<SolKeypair, KeypairManagerError> {
        let keypair = SolKeypair::new();
        let pubkey = keypair.pubkey();
        let path = keypair_path(&pubkey);
        let _result = keypair.write_to_file(&path)?;
        Ok(keypair)
    }

    pub fn forget_key(&self, pubkey: &Pubkey) -> Result<(), KeypairManagerError> {
        let path = keypair_path(pubkey);
        let mut file = File::create(&path).map_err(KeypairManagerError::OpenForShred)?;
        let len = file.metadata().map_err(KeypairManagerError::ReadMetadata)?.len();
        let buf = vec![0; len as usize];
        file.write_all(&buf).map_err(KeypairManagerError::Overwrite)?;
        std::fs::remove_file(path).map_err(KeypairManagerError::DeleteKeypair)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_forget_key() {
        let manager = KeypairManager;
        let keypair = manager.create_key().expect("Failed to create keypair");
        let pubkey = keypair.pubkey();
        let path = keypair_path(&pubkey);
        assert!(path.exists(), "Keypair file should exist after creation");
        manager.forget_key(&pubkey).expect("Failed to forget keypair");
        assert!(!path.exists(), "Keypair file should not exist after forgetting");
    }
}
