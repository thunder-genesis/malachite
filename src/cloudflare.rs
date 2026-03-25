use std::net::Ipv4Addr;

use cloudflare::{
    endpoints::{
        dns::dns::{CreateDnsRecord, CreateDnsRecordParams, DnsContent, DnsRecord},
        zones::zone::{ListZones, ListZonesParams},
    },
    framework::{Environment, auth::Credentials, client::async_api::Client},
};

type CFError = cloudflare::framework::Error;
type ZoneID = String;

#[derive(Debug, thiserror::Error)]
pub enum CloudflareError {
    #[error("Failed to list available zones for subdomain `{subdomain}`: {source}")]
    ListZones {
        subdomain: String,
        #[source]
        source: cloudflare::framework::response::ApiFailure,
    },

    #[error("Failed to find Zone ID")]
    NoZone,

    #[error("Failed to create DNS record for subdomain `{subdomain}`: {source}")]
    CreateDnsRecord {
        subdomain: String,
        #[source]
        source: cloudflare::framework::response::ApiFailure,
    },

    #[error("Failed to list DNS records for subdomain `{subdomain}`: {source}")]
    ListDnsRecords {
        subdomain: String,
        #[source]
        source: cloudflare::framework::response::ApiFailure,
    },
}

pub struct Cloudflare {
    domain: String,
    client: Client,
}

impl Cloudflare {
    pub fn new(api_token: impl AsRef<str>, domain: impl AsRef<str>) -> Result<Self, CFError> {
        let domain = domain.as_ref().to_string();
        let client = {
            let credentials = Credentials::UserAuthToken {
                token: api_token.as_ref().to_string(),
            };
            let config = Default::default();
            let environment = Environment::Production;
            Client::new(credentials, config, environment)?
        };

        Ok(Self { domain, client })
    }

    pub async fn register_subdomain(
        &self,
        subdomain: impl AsRef<str>,
        ip: Ipv4Addr,
    ) -> Result<DnsRecord, CloudflareError> {
        let zone = self.get_zone().await?;

        let create_dns_record = CreateDnsRecord {
            zone_identifier: zone.as_str(),
            params: CreateDnsRecordParams {
                ttl: Some(3600),
                priority: None,
                proxied: Some(true),
                name: subdomain.as_ref(),
                content: DnsContent::A { content: ip },
            },
        };

        self.client
            .request(&create_dns_record)
            .await
            .map(|r| r.result)
            .map_err(|source| CloudflareError::CreateDnsRecord {
                subdomain: subdomain.as_ref().to_string(),
                source,
            })
    }

    async fn get_zone(&self) -> Result<ZoneID, CloudflareError> {
        let list_zones = ListZones {
            params: ListZonesParams {
                name: Some(self.domain.clone()),
                ..Default::default()
            },
        };

        let api_result =
            self.client
                .request(&list_zones)
                .await
                .map_err(|source| CloudflareError::ListZones {
                    subdomain: self.domain.clone(),
                    source,
                })?;

        api_result
            .result
            .into_iter()
            .find(|zone| zone.name == self.domain)
            .map(|zone| zone.id)
            .ok_or(CloudflareError::NoZone)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[tokio::test]
//     async fn test() {
//         let _ = dotenvy::dotenv();

//         let cloudflare = Cloudflare::new(
//             std::env::var("CLOUDFLARE_API_TOKEN").unwrap(),
//             std::env::var("DOMAIN").unwrap(),
//         )
//         .unwrap();

//         cloudflare
//             .register_subdomain("test", "127.0.0.1".parse().unwrap())
//             .await
//             .unwrap();

//         cloudflare
//             .register_subdomain("test", "127.0.0.1".parse().unwrap())
//             .await
//             .unwrap();

//         let is_exist = cloudflare.is_subdomain_exists("test").await.unwrap();
//         println!("Domain `test` is exists: {is_exist}");

//         let is_exist = cloudflare.is_subdomain_exists("test2").await.unwrap();
//         println!("Domain `test2` is exists: {is_exist}");
//     }
// }
