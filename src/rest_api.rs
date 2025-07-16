use std::net::Ipv4Addr;

use crate::{
    context::Context,
    docker::DockerCompose,
    eth_contract::SubchainDB::SubchainStatus,
    subchain_transaction::{ChainID, SubchainConfig, evm_state_subchain_account},
    velas_network::VelasRpcError,
};
use actix_web::{
    HttpResponse, HttpResponseBuilder, Responder, ResponseError,
    body::BoxBody,
    get,
    http::StatusCode,
    post,
    web::{Data, Json},
};
use askama::Template;
use serde_json::json;
use solana_sdk::{pubkey::Pubkey, signature::Keypair as SolKeypair, signer::Signer as _};
use tracing::{error, info};

#[derive(Debug, thiserror::Error)]
pub enum CreateSubchainError {
    #[error("Failed to get Subchain status from Contract DB: {0}")]
    GetSubchainStatus(#[source] alloy::contract::Error),

    #[error("Subdomain `{0}` is already in use")]
    SubdomainInUse(String),

    #[error("Failed to register subdomain `{subdomain}`: {source}")]
    RegisterSubdomain {
        subdomain: String,
        #[source]
        source: crate::cloudflare::CloudflareError,
    },

    #[error("Subchain `{chain_id}` already exists")]
    SubchainAlreadyExists { chain_id: ChainID },

    #[error("Failed to fund Subchain owner account `{owner}`: {source}")]
    FundSubchainOwner {
        owner: Pubkey,
        #[source]
        source: VelasRpcError,
    },

    #[error("Failed to create Subchain EVM State account `{account}`: {source}")]
    CreateSubchainEvmStateAccount {
        account: Pubkey,
        #[source]
        source: VelasRpcError,
    },

    #[error("Failed to set Subchain status to Contract DB: {0}")]
    SetSubchainStatus(#[source] alloy::contract::Error),

    #[error("Failed to fund Subchain EVM State account `{account}`: {source}")]
    FundSubchainEvmState {
        account: Pubkey,
        #[source]
        source: VelasRpcError,
    },

    #[error("Failed to set Subchain expiration time to Contract DB: {0}")]
    SetExpirationTime(#[source] alloy::contract::Error),
}

impl ResponseError for CreateSubchainError {
    fn status_code(&self) -> StatusCode {
        StatusCode::OK
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let response = json!({
            "status": "error",
            "body": "implement error_response" // TODO: implement correctly
        });
        HttpResponseBuilder::new(self.status_code()).json(response)
    }
}

type UnixTimestamp = u64;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CreateSubchainValidated {
    pub domain: String,
    pub active_until: UnixTimestamp,
    pub chain_id: ChainID,
    pub config: SubchainConfig,
}

#[post("/create_subchain")]
async fn create_subchain(
    ctx: Data<Context>,
    parameters: Json<CreateSubchainValidated>,
) -> Result<(), CreateSubchainError> {
    // TODO: authentication?
    // TODO: validate input parameters

    let request_id = uuid::Uuid::new_v4();
    let span = tracing::info_span!("create_subchain", request_id = request_id.to_string());
    let _enter = span.enter();

    info!("Creating Subchain Owner keypair...");
    let owner = SolKeypair::new();
    info!("Subchain Owner keypair created, pubkey: {}", owner.pubkey());

    let _result = run_create_subchain(ctx, &owner, parameters.into_inner()).await;

    // TODO: logging and alerting error
    // TODO: recovery actions for errors

    Ok(())
}

const DEFAULT_IP: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

async fn run_create_subchain(
    ctx: Data<Context>,
    owner: &SolKeypair,
    parameters: CreateSubchainValidated,
) -> Result<(), CreateSubchainError> {
    let CreateSubchainValidated {
        domain,
        active_until,
        chain_id,
        config,
    } = parameters;

    info!("Request to create subchain with Chain ID: {chain_id}, subchain config: {config:?}");

    info!("Checking if subchain already exists...");
    let subchain_status = ctx
        .eth_contract
        .getSubchainStatus(chain_id)
        .call()
        .await
        .map_err(CreateSubchainError::GetSubchainStatus)?;

    if subchain_status != SubchainStatus::None {
        return Err(CreateSubchainError::SubchainAlreadyExists { chain_id });
    }

    // TODO: tiny chance of race condition
    info!("Checking is domain `{}` is available...", domain);
    let is_available = ctx.cloudflare.is_subdomain_exists(&domain).await.unwrap();
    if !is_available {
        return Err(CreateSubchainError::SubdomainInUse(domain.clone()));
    }
    info!("Registring DNS record for domain `{}`...", domain);
    let _dns_record = ctx
        .cloudflare
        .register_subdomain(&domain, DEFAULT_IP)
        .await
        .map_err(|source| CreateSubchainError::RegisterSubdomain {
            subdomain: domain.clone(),
            source,
        })?;

    info!("Funding Subchain Owner {}...", owner.pubkey());
    let sig = ctx
        .vlx
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
    let sig = ctx
        .vlx
        .create_subchain(owner, chain_id, config)
        .await
        .map_err(|source| CreateSubchainError::CreateSubchainEvmStateAccount {
            account: evm_state_pda,
            source,
        })?;
    info!("Subchain EVM State account created, signature: {sig}");

    info!("Marking Subchain {chain_id} as active in Contract DB...");
    ctx.eth_contract
        .setSubchainStatus(chain_id, SubchainStatus::SubchainDeployed)
        .call()
        .await
        .map_err(CreateSubchainError::SetSubchainStatus)?;
    info!("Subchain {chain_id} marked as active in Contract DB");

    info!("Funding Subchain EVM State account {}...", evm_state_pda);
    let sig = ctx
        .vlx
        .fund_subchain_state(evm_state_pda)
        .await
        .map_err(|source| CreateSubchainError::FundSubchainEvmState {
            account: evm_state_pda,
            source,
        })?;
    info!(
        "Subchain EVM State account {evm_state_pda} funded, signature: {}",
        sig
    );

    info!("Setting expiration timestamp {active_until} for Subchain {chain_id}...");
    ctx.eth_contract
        .setExpiryTimestamp(chain_id, active_until)
        .call()
        .await
        .map_err(CreateSubchainError::SetExpirationTime)?;
    info!("Expiration timestamp {active_until} is set for Subchain {chain_id}");

    Ok(())
}

#[get("/debug")]
async fn debug() -> impl Responder {
    let result = DockerCompose::new("my-chain", "velasocean.com").render().unwrap();
    HttpResponse::Ok().body(result)
}

#[cfg(test)]
mod tests {
    use actix_web::{dev::Service, test};
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::{cli::Cli, create_app};

    use super::*;

    #[actix_web::test]
    async fn test() {
        FmtSubscriber::builder().with_max_level(Level::INFO).init();

        let cli = Cli::mock();

        let context = Data::new(Context::new(&cli).await.unwrap());

        let app = test::init_service(create_app(context)).await;

        let req = test::TestRequest::post()
            .uri("/v1/create_subchain")
            .set_json(CreateSubchainValidated {
                domain: "hello".to_string(),
                active_until: 777888,
                chain_id: 0x561,
                config: SubchainConfig::default(),
            })
            .to_request();

        let resp = app.call(req).await.unwrap();

        info!("Response: {:?}", resp);

        let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
        info!("Body: {}", body);
    }
}
