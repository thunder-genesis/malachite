/// Telegram alers
#[allow(unused)]
mod alert;
/// Virtual machine initialization with Subchain gateway and explorer
mod bootstrapper;
/// Malachite CLI
mod cli;
/// Domain names managements for Subchains
mod cloudflare;
/// Unbrella for all external API's
mod context;
/// Dockerfile templates for new virtual machines
mod docker;
/// Create, backup and shred Subchain Owner keypairs
mod keymanager;
/// Metadata downloading and contract compiling
mod metadata;
/// Virtual machine running and stopping
mod openstack;
/// EVM Subchain Manager contract interact
mod subchain_registry;
/// Transaction for creating Subchains in Velas Native
mod subchain_transaction;
/// Velas Native various transactions and RPC interaction
mod velas_network;

use std::{net::Ipv4Addr, time::Duration};

use alloy::primitives::{U256 as AlloyU256, Uint};
use clap::Parser;
use futures_util::StreamExt as _;
use solana_sdk::{pubkey::Pubkey, signer::Signer as _};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::{
    cli::Cli,
    context::Context,
    docker::DockerCompose,
    subchain_registry::SubchainRegistry::{self, Status},
    subchain_transaction::{SubchainConfig, evm_state_subchain_account},
};

type SubchainEntry = SubchainRegistry::getSubchainReturn;

#[derive(Debug, thiserror::Error)]
pub enum CreateSubchainError {
    #[error("Failed to get Subchain status from Subchain Registry: {0}")]
    GetSubchainStatus(#[from] alloy::contract::Error),

    #[error(transparent)]
    MetadataError(#[from] metadata::MetadataError),

    #[error("Failed to register subdomain `{subdomain}`: {source}")]
    RegisterSubdomain {
        subdomain: String,
        #[source]
        source: crate::cloudflare::CloudflareError,
    },

    #[error("Subchain `{chain_id}` already exists")]
    SubchainAlreadyExists { chain_id: subchain_transaction::ChainID },

    #[error("Failed to fund Subchain owner account `{owner}`: {source}")]
    FundSubchainOwner {
        owner: Pubkey,
        #[source]
        source: velas_network::VelasRpcError,
    },

    #[error("Failed to create Subchain EVM State account `{account}`: {source}")]
    CreateSubchainEvmStateAccount {
        account: Pubkey,
        #[source]
        source: velas_network::VelasRpcError,
    },

    #[error("Failed to set Subchain status to Subchain Registry: {0}")]
    SetSubchainStatus(#[source] alloy::contract::Error),

    #[error("Failed to fund Subchain EVM State account `{account}`: {source}")]
    FundSubchainEvmState {
        account: Pubkey,
        #[source]
        source: velas_network::VelasRpcError,
    },

    #[error("Failed to set Subchain expiration time to Subchain Registry: {0}")]
    SetExpirationTime(#[source] alloy::contract::Error),

    #[error("Failed to launch OpenStack instance for subchain `{name}`: {source}")]
    OpenStackError {
        name: String,
        #[source]
        source: crate::openstack::CloudError,
    },

    #[error("OpenStack instance `{name}` IP `{instance_ip}` is not responding")]
    InstanceNotResponding { name: String, instance_ip: Ipv4Addr },

    #[error(transparent)]
    BootstrapperError(#[from] crate::bootstrapper::BootstrapError),

    #[error(transparent)]
    KeypairManagementError(#[from] crate::keymanager::KeypairManagerError),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    // TODO: handle other EVM events
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
                let _owner = subchain_registered.owner;
                info!("New subchain registered with index {idx}");
                match handle_new_subchain(&context, idx).await {
                    Ok(()) => info!("Successfully handled new subchain"),
                    Err(e) => error!("Failed to handle new subchain: {e}"),
                }
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

    let subchain_entry = ctx.subchain_registry.getSubchain(subchain_idx).call().await?;

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
    } = subchain_entry;

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
    info!("Quirk: converted chain ID to u64: {chain_id}");

    let metadata = ctx
        .metadata
        .extract_and_compile_metadata(metadataUrl, name.clone(), symbol.clone(), subchain_eth(1), owner)
        .await?;

    let owner = ctx.keypair_manager.create_key()?;
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

    let evm_state_pda = evm_state_subchain_account(chain_id);

    info!("Creating Subchain EVM State account {evm_state_pda}...");

    let config = SubchainConfig {
        alloc: metadata.alloc,
        whitelisted: Default::default(), // TODO: strict IP?
        hardfork: crate::subchain_transaction::Hardfork::Istanbul,
        network_name: name.clone(),
        token_name: symbol,
        min_gas_price: gwei(300), // TODO: set proper value
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
    ctx.bootstrapper.bootstrap(ssh_socket, docker_compose, &owner)?;
    info!("Subchain bridge instance bootstrapped successfully");

    info!("Marking Subchain {chain_id} as active in Subchain Registry...");
    ctx.subchain_registry
        .setStatus(subchain_idx, Status::Active)
        .call()
        .await
        .map_err(CreateSubchainError::SetSubchainStatus)?;
    info!("Subchain {chain_id} marked as active in Subchain Registry");

    ctx.keypair_manager.forget_key(&owner.pubkey())?;
    Ok(())
}

fn gwei(value: u64) -> primitive_types::U256 {
    primitive_types::U256::from(value) * primitive_types::U256::from(10).pow(9.into())
}

fn subchain_eth(value: u64) -> AlloyU256 {
    AlloyU256::from(value) * AlloyU256::from(10).pow(AlloyU256::from(18))
}
