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

use alloy::primitives::{Address, U256 as AlloyU256};
use clap::Parser;
use futures_util::StreamExt as _;
use solana_sdk::{pubkey::Pubkey, signer::Signer as _};
use tracing::{error, info, instrument};
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
pub enum HandleSubchainError {
    #[error("Failed to parse chain ID: {0}")]
    BadChainID(#[from] alloy::primitives::ruint::FromUintError<u64>),

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

    let context = Context::new(&cli)
        .await
        .expect("Failed to create context, check configuration");

    let mut registrations = context
        .subchain_registry
        .SubchainRegistered_filter()
        .watch()
        .await
        .expect("Can't connect to EVM network and listen for SubchainRegistered events")
        .into_stream();

    info!("Listening for SubchainRegistered events...");

    // use alloy::rpc::types::Log;
    // let subchain_registered = crate::subchain_registry::SubchainRegistry::SubchainRegistered {
    //     index: alloy::primitives::Uint::from(1),
    //     owner: Address::ZERO,
    // };
    // let log: Log<alloy::primitives::LogData> = Log::default();
    // let mut registrations =
    //     Box::pin(futures_util::stream::once(async { Ok((subchain_registered, log)) }).chain(registrations));

    while let Some(subchain_registered) = registrations.next().await {
        let (subchain_registered, _log) = match subchain_registered {
            Ok(subchain_registered) => subchain_registered,
            Err(e) => {
                error!("Failed to get `SubchainRegistered` event: {e}");
                // TODO: tg alert
                continue;
            }
        };

        let subchain_idx = subchain_registered.index;
        let _owner = subchain_registered.owner;
        info!("New Subchain registered with index {subchain_idx}");
        info!("Fetching details of new Subchain...");

        info!("Fetching details of subchain[{subchain_idx}]...");
        let subchain_entry = match context.subchain_registry.getSubchain(subchain_idx).call().await {
            Ok(subchain_entry) => subchain_entry,
            Err(e) => {
                error!("Failed to execute `getSubchain` RPC call: {e}");
                // TODO: tg alert
                continue;
            }
        };

        let SubchainEntry {
            name,
            domain,
            symbol,
            metadataUrl,
            chainId,
            owner,
            status: _,
            registrationTime: _,
            activeTill: _,
        } = subchain_entry;

        info!("subchain name: {name}");
        info!("subchain domain: {domain}");
        info!("subchain symbol: {symbol}");
        info!("subchain metadataUrl: {metadataUrl}");
        info!("subchain chainId: {chainId}");
        info!("subchain owner: {owner}");
        // info!("subchain status: {status}");
        // info!("subchain registrationTime: {registrationTime}");
        // info!("subchain activeTill: {activeTill}");

        let chain_id: u64 = match chainId.try_into() {
            Ok(chain_id) => chain_id,
            Err(e) => {
                error!("Failed to convert Chain ID U256 {chainId} to u64: {e}");
                // TODO: tg alert
                continue;
            }
        };

        if let Err(e) =
            deploy_new_subchain(&context, name, domain, symbol, metadataUrl, chain_id, owner).await
        {
            error!("Failed to deploy new Subchain: {e}");
            let _ = context
                .tg_alert
                .notify_subchain_creation_error(&e)
                .await
                .inspect_err(|e| error!("Telegram notification failed: {e}"));
            continue;
        }

        info!("Subchain deployed successfully!");

        info!("Marking Subchain {chain_id} as active in Subchain Registry...");

        if let Err(e) = context
            .subchain_registry
            .setStatus(subchain_idx, Status::Active)
            .call()
            .await
        {
            error!("Failed to execute `setStatus` RPC call for Subchain {chain_id}: {e}");
            // TODO: tg alert
            continue;
        }

        // TODO: tg alert

        info!("Subchain {chain_id} marked as active in Subchain Registry");
    }

    // TODO: tg alert

    Ok(())
}

#[instrument(skip_all, fields(subchain_name, domain, chain_id))]
async fn deploy_new_subchain(
    ctx: &Context,
    subchain_name: String,
    domain: String,
    symbol: String,
    metadata_url: String,
    chain_id: u64,
    owner: Address,
) -> Result<(), HandleSubchainError> {
    info!(
        "Handling new subchain request: name={subchain_name}, domain={domain}, symbol={symbol}, \
        metadata_url={metadata_url}, chainId={chain_id}, owner={owner}"
    );

    let metadata = ctx
        .metadata
        .extract_and_compile_metadata(
            metadata_url,
            subchain_name.clone(),
            symbol.clone(),
            subchain_eth(1),
            owner,
        )
        .await?;

    let owner = ctx.keypair_manager.create_key()?;
    info!("Funding Subchain Owner {}...", owner.pubkey());
    let sig = ctx
        .velas_network
        .fund_subchain_owner(&owner.pubkey())
        .await
        .map_err(|source| HandleSubchainError::FundSubchainOwner {
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
        network_name: subchain_name.clone(),
        token_name: symbol,
        min_gas_price: ctx.min_gas_price,
    };
    let sig = ctx
        .velas_network
        .create_subchain(&owner, chain_id, config)
        .await
        .map_err(|source| HandleSubchainError::CreateSubchainEvmStateAccount {
            account: evm_state_pda,
            source,
        })?;
    info!("Subchain EVM State account created, signature: {sig}");

    info!("Funding Subchain EVM State account {}...", evm_state_pda);
    let sig = ctx
        .velas_network
        .fund_subchain_state(evm_state_pda)
        .await
        .map_err(|source| HandleSubchainError::FundSubchainEvmState {
            account: evm_state_pda,
            source,
        })?;
    info!("Subchain EVM State account {evm_state_pda} funded, signature: {sig}");

    info!("Deploying OpenStack instance for subchain `{subchain_name}`...");
    let instance_ip = ctx
        .openstack
        .deploy_openstack_instance(&subchain_name)
        .await
        .map_err(|source| HandleSubchainError::OpenStackError {
            source,
            name: subchain_name.clone(),
        })?;
    info!("OpenStack instance for subchain `{subchain_name}` has been deployed, IP: {instance_ip}");

    info!("Registring DNS record for domain `{}`...", domain);
    let _dns_record = ctx
        .cloudflare
        .register_subdomain(&domain, instance_ip)
        .await
        .map_err(|source| HandleSubchainError::RegisterSubdomain {
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

        return Err(HandleSubchainError::InstanceNotResponding {
            instance_ip,
            name: subchain_name.clone(),
        });
    }

    // TODO: await SSH availability in more reliable way
    const SSH_AWAITING_SECONDS: u64 = 60;
    info!("Waiting for SSH to become available in {SSH_AWAITING_SECONDS} seconds...");
    tokio::time::sleep(Duration::from_secs(SSH_AWAITING_SECONDS)).await;

    info!("Bootstrapping Subchain instance `{subchain_name}`...");
    let docker_compose = DockerCompose::new(
        domain.clone(),
        ctx.domain.clone(),
        chain_id,
        &ctx.vlx_network_for_bridge,
    );
    let ssh_socket = (instance_ip, 22).into();
    ctx.bootstrapper.bootstrap(ssh_socket, docker_compose, &owner)?;
    info!("Subchain bridge instance bootstrapped successfully");

    ctx.keypair_manager.forget_key(&owner.pubkey())?;
    Ok(())
}

fn subchain_eth(value: u64) -> AlloyU256 {
    AlloyU256::from(value) * AlloyU256::from(10).pow(AlloyU256::from(18))
}
