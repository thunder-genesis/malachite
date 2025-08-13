use std::{net::Ipv4Addr, time::Duration};

use alloy::{primitives::Uint, providers::ProviderBuilder};
use clap::Parser;
use futures_util::StreamExt as _;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair as SolKeypair, signer::Signer as _};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::{
    cli::Cli,
    cloudflare::Cloudflare,
    context::Context,
    docker::DockerCompose,
    openstack::Openstack,
    rest_api::CreateSubchainError,
    subchain_registry::SubchainRegistry::{self, Status},
    subchain_transaction::{SubchainConfig, evm_state_subchain_account},
    velas_network::VelasNetwork,
};

type SubchainEntry = SubchainRegistry::getSubchainReturn;
type SubchainRegistryImpl = SubchainRegistry::SubchainRegistryInstance<
    alloy::providers::fillers::FillProvider<
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::JoinFill<
                alloy::providers::Identity,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::GasFiller,
                    alloy::providers::fillers::JoinFill<
                        alloy::providers::fillers::BlobGasFiller,
                        alloy::providers::fillers::JoinFill<
                            alloy::providers::fillers::NonceFiller,
                            alloy::providers::fillers::ChainIdFiller,
                        >,
                    >,
                >,
            >,
            alloy::providers::fillers::WalletFiller<alloy::network::EthereumWallet>,
        >,
        alloy::providers::RootProvider,
    >,
>;

async fn main_loop() -> Result<(), ()> {
    let dotenv = dotenvy::dotenv();

    let cli = Cli::parse();

    FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if dotenv.is_ok() {
        info!("Loaded environment variables from `.env`");
    }

    info!("Executing service with parameters: {:#?}", cli);

    let context = Context::new(&cli).await.unwrap();

    let mut registrations = context
        .subchain_registry
        .SubchainRegistered_filter()
        .watch()
        .await
        .unwrap()
        .into_stream();

    while let Some(subchain_registered) = registrations.next().await {
        match subchain_registered {
            Ok((subchain_registered, _log)) => {
                let idx = subchain_registered.index;
                info!("New subchain registered with index {idx}");
                handle_new_subchain(&context, idx).await.unwrap();
            }
            Err(e) => {
                error!("Failed to get `SubchainRegistered` event: {e}");
                continue;
            }
        }
    }

    Ok(())
}

async fn handle_new_subchain(ctx: &Context, subchain_idx: Uint<256, 4>) -> Result<(), CreateSubchainError> {
    info!("Fetching details of new subchain...");

    // TODO: Change contract interface to explicitly return struct
    let SubchainEntry {
        name,
        domain,
        symbol,
        metadataUrl,
        chainId,
        owner,
        status,
        registrationTime,
        activeTill,
    } = ctx
        .subchain_registry
        .getSubchain(subchain_idx)
        .call()
        .await
        .unwrap();

    info!(
        "New subchain details: \
        name={name}, domain={domain}, symbol={symbol}, metadataUrl={metadataUrl}, chainId={chainId}, \
        owner={owner}, status={status:?}, registrationTime={registrationTime}, activeTill={activeTill}"
    );

    // TODO: This check should be fixed on a contract level to avoid possible overflows
    if chainId > Uint::from(u64::MAX) {
        panic!("Chain ID is too big, fix contract to avoid possible overflows");
    }
    let chain_id: [u8; 8] = chainId.bitand(Uint::from(u64::MAX)).to_be_bytes();
    let chain_id = u64::from_be_bytes(chain_id);

    info!("quirk: converted chain ID to u64: {chain_id}");

    // // TODO: tiny chance of race condition, security issue
    // info!("Checking is domain `{}` is available...", domain);
    // let is_available = ctx.cloudflare.is_subdomain_exists(&domain).await.unwrap();
    // if !is_available {
    //     return Err(CreateSubchainError::SubdomainInUse(domain.clone()));
    // }

    let owner = SolKeypair::new();
    info!("Funding Subchain Owner {}...", owner.pubkey());
    let sig = ctx
        .velas_network
        .fund_subchain_owner(&owner.pubkey())
        .await
        .map_err(|source| CreateSubchainError::FundSubchainOwner {
            owner: owner.pubkey(),
            source,
        })?;
    info!("Subchain Owner {} funded, signature: {sig}", owner.pubkey());

    // NOTE: At this point `owner` account is funded and extra care is needed to avoid losing funds.

    let evm_state_pda = evm_state_subchain_account(chain_id);

    info!("Creating Subchain EVM State account {evm_state_pda}...");
    let config = SubchainConfig {
        alloc: Default::default(), // TODO: fill alloc with real values
        whitelisted: Default::default(),
        hardfork: crate::subchain_transaction::Hardfork::Istanbul,
        network_name: name.clone(),
        token_name: symbol,
        min_gas_price: Default::default(), // TODO: set proper value
    };
    let sig = ctx
        .velas_network
        .create_subchain(&owner, chain_id, config)
        .await
        .map_err(|source| CreateSubchainError::CreateSubchainEvmStateAccount {
            account: evm_state_pda,
            source,
        })?;
    info!("Subchain EVM State account created, signature: {sig}");

    info!("Funding Subchain EVM State account {}...", evm_state_pda);
    let sig = ctx
        .velas_network
        .fund_subchain_state(evm_state_pda)
        .await
        .map_err(|source| CreateSubchainError::FundSubchainEvmState {
            account: evm_state_pda,
            source,
        })?;
    info!("Subchain EVM State account {evm_state_pda} funded, signature: {sig}");

    info!("Deploying OpenStack instance for subchain `{name}`...");
    let instance_ip = ctx
        .openstack
        .deploy_openstack_instance(&name)
        .await
        .map_err(|source| CreateSubchainError::OpenStackError {
            source,
            name: name.clone(),
        })?;
    info!("OpenStack instance for subchain `{name}` has been deployed, IP: {instance_ip}");

    info!("Registring DNS record for domain `{}`...", domain);
    let _dns_record = ctx
        .cloudflare
        .register_subdomain(&domain, instance_ip)
        .await
        .map_err(|source| CreateSubchainError::RegisterSubdomain {
            subdomain: domain.clone(),
            source,
        })?;

    'ping: {
        const MAX_RETRIES: u32 = 60;
        for n in 1..=MAX_RETRIES {
            info!("Pinging instance {instance_ip}, attempt {n}/{MAX_RETRIES}...");
            let ping = surge_ping::ping(instance_ip.into(), &[]).await;

            match ping {
                Ok((_packet, duration)) => {
                    info!("{instance_ip} pinged in {}ms", duration.as_millis());
                    break 'ping;
                }
                Err(_err) => {
                    info!("{instance_ip} has not responded");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        return Err(CreateSubchainError::InstanceNotResponding {
            instance_ip,
            name: name.clone(),
        });
    }

    // TODO: await SSH availability in more reliable way
    const SSH_AWAITING_SECONDS: u64 = 60;
    info!("Waiting for SSH to become available in {SSH_AWAITING_SECONDS} seconds...");
    tokio::time::sleep(Duration::from_secs(SSH_AWAITING_SECONDS)).await;

    info!("Bootstrapping Subchain instance `{name}`...");
    let docker_compose = DockerCompose::new(
        domain.clone(),
        ctx.domain.clone(),
        chain_id,
        &ctx.vlx_network_for_bridge,
    );
    let ssh_socket = (instance_ip, 22).into();
    ctx.bootstrapper
        .bootstrap(ssh_socket, docker_compose, &owner)
        .unwrap();
    info!("Subchain bridge instance bootstrapped successfully");

    info!("Marking Subchain {chain_id} as active in Subchain Registry...");
    ctx.subchain_registry
        .setStatus(subchain_idx, Status::Active)
        .call()
        .await
        .map_err(CreateSubchainError::SetSubchainStatus)?;
    info!("Subchain {chain_id} marked as active in Subchain Registry");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn main_loop_test() {
        main_loop().await.unwrap();
    }
}
