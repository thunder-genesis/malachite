mod alert;
mod bootstrapper;
mod cli;
mod cloudflare;
mod context;
mod docker;
mod eth_contract;
mod openstack;
mod rest_api;
mod subchain_transaction;
mod velas_network;

use crate::{cli::Cli, context::Context};
use actix_web::{
    App, HttpServer,
    body::BoxBody,
    dev::{ServiceFactory, ServiceRequest, ServiceResponse},
    web::{self, Data},
};
use clap::Parser as _;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

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

    let context = Data::new(Context::new(&cli).await?);
    info!("Listening service on {}...", cli.bind_address);

    HttpServer::new(move || create_app(context.clone()))
        .bind(cli.bind_address)?
        .run()
        .await
        .map_err(Into::into)
}

pub fn create_app(
    context: Data<Context>,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Error = actix_web::Error,
        InitError = (),
        Response = ServiceResponse<BoxBody>,
    >,
> {
    App::new()
        .app_data(context)
        .service(
            web::scope("/v1")
                .service(rest_api::create_subchain)
                .service(rest_api::debug),
        )
        .default_service(web::get().to(|| async { "This is default service stub" }))
}
