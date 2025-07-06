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
use solana_sdk::{signature::Keypair as SolKeypair, signer::Signer as _};
use tracing::info;

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum CreateSubchainError {
    #[error("Failed to get subchain status from DB: {0}")]
    GetSubchainStatusError(String),

    #[error("Subchain {chain_id} already exists")]
    SubchainAlreadyExists { chain_id: ChainID },

    #[error(transparent)]
    VelasRpcError(#[from] VelasRpcError),
}

impl ResponseError for CreateSubchainError {
    fn status_code(&self) -> StatusCode {
        StatusCode::OK
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let response = json!({
            "status": "error",
            "body": self
        });
        HttpResponseBuilder::new(self.status_code()).json(response)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CreateSubchain {
    pub domain: String,
    pub chain_id: ChainID,
    pub config: SubchainConfig,
}

#[post("/create_subchain")]
async fn create_subchain(
    ctx: Data<Context>,
    parameters: Json<CreateSubchain>,
) -> Result<(), CreateSubchainError> {
    // TODO: authentication?

    let request_id = uuid::Uuid::new_v4().to_string();
    let span = tracing::info_span!("create_subchain", request_id = request_id);
    let _enter = span.enter();

    let CreateSubchain {
        domain,
        chain_id,
        config,
    } = parameters.into_inner();

    info!("Creating subchain with Chain ID: {chain_id}, subchain config: {config:?}");

    let subchain_status = ctx
        .eth_contract
        .getSubchainStatus(chain_id)
        .call()
        .await
        .map_err(|e| CreateSubchainError::GetSubchainStatusError(e.to_string()))?;

    match subchain_status {
        SubchainStatus::None => (),
        _ => {
            return Err(CreateSubchainError::SubchainAlreadyExists { chain_id });
        }
    };

    let owner = SolKeypair::new();

    info!("Subchain owner keypair created, pubkey: {}", owner.pubkey());

    info!("Funding subchain owner...");
    let sig = ctx
        .velas_network
        .fund_subchain_owner(&owner.pubkey())
        .await?;
    info!("Subchain owner funded, signature: {sig}");

    // NOTE: At this point `owner` account is funded and extra care is needed to avoid losing funds.

    info!("Creating subchain account");
    let sig = ctx
        .velas_network
        .create_subchain(owner, chain_id, config)
        .await?;
    info!("Subchain account created, signature: {sig}");

    // TODO: update contract storage

    let subchain_state_pda = evm_state_subchain_account(chain_id);
    info!("Funding subchain state PDA: {}", subchain_state_pda);
    let sig = ctx
        .velas_network
        .fund_subchain_state(subchain_state_pda)
        .await?;
    info!("Subchain state PDA funded, signature: {}", sig);

    Ok(())
}

#[get("/debug")]
async fn debug() -> impl Responder {
    let result = DockerCompose::new("my-chain", "velasocean.com")
        .render()
        .unwrap();
    HttpResponse::Ok().body(result)
}

#[cfg(test)]
mod tests {
    use actix_web::{dev::Service, test};
    use alloy::primitives::address;
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::{cli::Cli, create_app};

    use super::*;

    #[actix_web::test]
    async fn test() {
        FmtSubscriber::builder().with_max_level(Level::DEBUG).init();

        let cli = Cli {
            bind_address: "127.0.0.1:8080".parse().unwrap(),
            smc_network_rpc: "http://127.0.0.1:8545".to_string(),
            smc_address: address!("0x0101010101010101010101010101010101010101"),
            smc_signer: "0x0101010101010101010101010101010101010101010101010101010101010101"
                .parse()
                .unwrap(),
            vlx_network_rpc: "http://127.0.0.1:8899".to_string(),
            vlx_native_keypair: "test/hotwallet.json".into(),
            tg_bot_token: None,
            tg_chat_id: None,
            fund_subchain_owner: 1_000_001___000000000,
            fund_subchain_state: 10_000___000000000,
            cloudflare_api_token: "".into(),
            domain: "velasocean.com".into(),
        };

        let context = Data::new(Context::new(&cli).await.unwrap());

        let app = test::init_service(create_app(context)).await;

        let req = test::TestRequest::post()
            .uri("/v1/create_subchain")
            .set_json(CreateSubchain {
                domain: "hello".to_string(),
                chain_id: 0x5601,
                config: SubchainConfig::default(),
            })
            .to_request();

        let resp = app.call(req).await.unwrap();

        println!("Response: {:?}", resp);

        let body = test::read_body(resp).await;
        println!("Body: {:?}", body);
    }
}
