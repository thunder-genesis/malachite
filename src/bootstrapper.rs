use std::{io::Write as _, net::SocketAddr};

use ssh2::Session;
use tokio::net::TcpStream;
use tracing::{error, info};

use crate::docker::DockerCompose;

pub struct Bootstrapper {
    ssh_private_key: String,
    bootstrap_script: Vec<u8>,
}

const REMOTE_USERNAME: &str = "ubuntu";

impl Bootstrapper {
    pub fn new(
        ssh_private_key: String, // PEM file content
        bootstrap_script: Vec<u8>,
    ) -> Self {
        Self {
            ssh_private_key,
            bootstrap_script,
        }
    }

    pub async fn bootstrap(
        &self,
        instance: SocketAddr,
        subchain: DockerCompose,
        bridge_keypair: String,
    ) -> Result<(), ()> {
        let tcp = TcpStream::connect(instance).await.unwrap();
        let mut s = Session::new().unwrap();
        s.set_tcp_stream(tcp);
        s.handshake().unwrap();
        s.userauth_pubkey_memory(REMOTE_USERNAME, None, &self.ssh_private_key, None)
            .unwrap();

        if !s.authenticated() {
            error!("Cannot authenticate");
            return Err(());
        }

        self.ssh_scp(&mut s, "bootstrap.sh", &self.bootstrap_script)?;
        self.ssh_scp(&mut s, "docker-compose.yml", subchain.to_string().as_bytes())?;
        self.ssh_scp(&mut s, "keypair.json", &bridge_keypair.as_bytes())?;
        Ok(())
    }

    fn ssh_scp(&self, session: &mut Session, file_name: &str, data: &[u8]) -> Result<(), ()> {
        let mut remote_file = session
            .scp_send(file_name.as_ref(), 0o744, data.len() as u64, None)
            .unwrap();

        remote_file.write_all(&data).unwrap();

        remote_file.send_eof().unwrap();
        remote_file.wait_eof().unwrap();
        remote_file.wait_close().unwrap();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::docker::DockerCompose;

    use super::*;

    #[tokio::test]
    async fn test_connection() {
        let _ = FmtSubscriber::builder().with_max_level(Level::INFO).try_init();

        let boot = Bootstrapper::new(
            include_str!("../test/ssh-key").to_string(),
            include_bytes!("../scripts/bootstrap.sh").to_vec(),
        );

        let subchain_id = 0x5739;
        let velas_rpc_url = "https://rpc.velas.com";
        let bridge_bind_address = "0.0.0.0:8545";
        let docker_compose = DockerCompose::new(
            "testchain",
            "velasocean.com",
            subchain_id,
            velas_rpc_url,
            bridge_bind_address,
        );
        let socket: SocketAddr = "10.35.48.74:22".parse().unwrap();
        let bridge_keypair =
            include_str!("../test/DBAFnAjY7EucVizMaguyXK2N3HyaWNyVcNqBYeRPd1JP.json").to_string();

        let _result = boot
            .bootstrap(socket, docker_compose, bridge_keypair)
            .await
            .unwrap();
    }
}
