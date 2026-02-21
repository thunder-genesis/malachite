use std::{
    io::{Read as _, Write as _},
    net::SocketAddr,
};

use solana_sdk::signature::Keypair as SolKeypair;
use ssh2::Session;
use std::net::TcpStream;
use tracing::{error, info};

use crate::docker::DockerCompose;

const SPACER: &str = "--------------------------------------------------";

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error(transparent)]
    SSHError(#[from] SCPError),

    #[error("SSH session is not authenticated")]
    NotAuthenticated,

    #[error("Failed to authenticate SSH session: {0}")]
    FailedToAuthenticate(#[source] ssh2::Error),

    #[error("Failed to create SSH session: {0}")]
    FailedToCreateSession(#[source] ssh2::Error),

    #[error("Remote instance is unavailable: {0}")]
    InstanceUnavailable(#[source] std::io::Error),

    #[error("Failed to handshake SSH session: {0}")]
    FailedHandshake(#[source] ssh2::Error),

    #[error("Failed to create SSH channel: {0}")]
    FailedToCreateChannel(#[source] ssh2::Error),

    #[error("Failed to execute remote command: {0}")]
    FailedToExecRemoteCommand(#[source] ssh2::Error),

    #[error("Failed to read stdout from remote command: {0}")]
    FailedToReadStdout(#[source] std::io::Error),

    #[error("Failed to read stderr from remote command: {0}")]
    FailedToReadStderr(#[source] std::io::Error),

    #[error("Failed to close SSH channel: {0}")]
    FailedToCloseChannel(#[source] ssh2::Error),

    #[error("Failed to get exit status of remote command: {0}")]
    FailedToGetExitStatus(#[source] ssh2::Error),

    #[error("Remote script exited with non-zero status: {0}")]
    RemoteScriptNonZeroExit(i32),
}

#[derive(Debug, thiserror::Error)]
pub enum SCPError {
    #[error("Failed to scp `{file_name}` file to remote: {source}")]
    FailToSecureCopyToRemote {
        #[source]
        source: ssh2::Error,
        file_name: String,
    },

    #[error("Failed to scp `{file_name}` file to remote: {source}")]
    FailToWriteBuffer {
        #[source]
        source: std::io::Error,
        file_name: String,
    },
}

impl SCPError {
    fn fail_to_scp(err: ssh2::Error, file_name: &str) -> Self {
        Self::FailToSecureCopyToRemote {
            source: err,
            file_name: file_name.to_string(),
        }
    }
}

pub struct Bootstrapper {
    ssh_username: String,
    ssh_private_key: String,
    bootstrap_script: Vec<u8>,
}

impl Bootstrapper {
    pub fn new(
        ssh_username: String,
        ssh_private_key: String, // PEM file content
        bootstrap_script: Vec<u8>,
    ) -> Self {
        Self {
            ssh_username,
            ssh_private_key,
            bootstrap_script,
        }
    }

    pub fn bootstrap(
        &self,
        instance: SocketAddr,
        subchain: DockerCompose,
        bridge_keypair: &SolKeypair,
    ) -> Result<(), BootstrapError> {
        info!("Opening SSH session to remote instance: {}...", instance);
        let tcp = TcpStream::connect(instance).map_err(BootstrapError::InstanceUnavailable)?;
        let mut s = Session::new().map_err(BootstrapError::FailedToCreateSession)?;
        s.set_tcp_stream(tcp);
        s.handshake().map_err(BootstrapError::FailedHandshake)?;

        info!("Authenticating SSH session...");

        s.userauth_pubkey_memory(&self.ssh_username, None, &self.ssh_private_key, None)
            .map_err(BootstrapError::FailedToAuthenticate)?;

        if !s.authenticated() {
            error!("SSH session is not authenticated");
            return Err(BootstrapError::NotAuthenticated);
        }

        info!("Copying `bootstrap.sh` to remote instance...");
        self.ssh_scp(&mut s, "bootstrap.sh", &self.bootstrap_script)?;

        info!("Copying `docker-compose.yml` to remote instance...");
        self.ssh_scp(&mut s, "docker-compose.yml", subchain.to_string().as_bytes())?;

        info!("Copying `keypair.json` to remote instance...");
        let bridge_keypair = serde_json::to_string(bridge_keypair.to_bytes().as_ref()).unwrap();
        self.ssh_scp(&mut s, "keypair.json", bridge_keypair.as_bytes())?;

        let mut channel = s
            .channel_session()
            .map_err(BootstrapError::FailedToCreateChannel)?;

        info!("Executing remote script...");
        channel
            .exec("bash ./bootstrap.sh")
            .map_err(BootstrapError::FailedToExecRemoteCommand)?;

        let mut stdout = String::new();
        channel
            .read_to_string(&mut stdout)
            .map_err(BootstrapError::FailedToReadStdout)?;
        info!(SPACER);
        info!("REMOTE STDOUT: \n{}", stdout);
        info!(SPACER);

        let mut stderr = String::new();
        channel
            .stderr()
            .read_to_string(&mut stderr)
            .map_err(BootstrapError::FailedToReadStderr)?;
        info!(SPACER);
        info!("REMOTE STRERR: \n{}", stderr);
        info!(SPACER);

        channel
            .wait_close()
            .map_err(BootstrapError::FailedToCloseChannel)?;
        let status = channel
            .exit_status()
            .map_err(BootstrapError::FailedToGetExitStatus)?;

        if status == 0 {
            info!("Remote script exited with status: {}", status);
        } else {
            error!("Remote script exited with non-zero status: {}", status);
            return Err(BootstrapError::RemoteScriptNonZeroExit(status));
        }

        Ok(())
    }

    fn ssh_scp(&self, session: &mut Session, file_name: &str, data: &[u8]) -> Result<(), SCPError> {
        let mut remote_file = session
            .scp_send(file_name.as_ref(), 0o744, data.len() as u64, None)
            .map_err(|source| SCPError::fail_to_scp(source, file_name))?;

        remote_file
            .write_all(data)
            .map_err(|source| SCPError::FailToWriteBuffer {
                source,
                file_name: file_name.to_string(),
            })?;

        remote_file
            .send_eof()
            .map_err(|source| SCPError::fail_to_scp(source, file_name))?;
        remote_file
            .wait_eof()
            .map_err(|source| SCPError::fail_to_scp(source, file_name))?;
        remote_file
            .wait_close()
            .map_err(|source| SCPError::fail_to_scp(source, file_name))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use solana_sdk::signer::EncodableKey;
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::docker::DockerCompose;

    use super::*;

    #[tokio::test]
    #[ignore = "this is not a test"]
    async fn test_connection() {
        let _ = FmtSubscriber::builder().with_max_level(Level::INFO).try_init();

        let boot = Bootstrapper::new(
            "root".to_string(),
            include_str!("../test/ssh-key").to_string(),
            include_bytes!("../bootstrap/bootstrap.sh").to_vec(),
        );

        let subchain_id = 0x5739;
        let velas_rpc_url = "https://rpc.velas.com";
        let docker_compose = DockerCompose::new("testchain", "velasocean.com", subchain_id, velas_rpc_url);
        let socket: SocketAddr = "10.35.48.74:22".parse().unwrap();
        let bridge_keypair =
            SolKeypair::read_from_file("../test/DBAFnAjY7EucVizMaguyXK2N3HyaWNyVcNqBYeRPd1JP.json").unwrap();

        let _result = boot.bootstrap(socket, docker_compose, &bridge_keypair).unwrap();
    }
}
