use crate::error::TokenError;
use crate::settings::ClientConfig;
use biscuit::jws;
use chrono::DateTime;
use chrono::Utc;
use failure::Error;
use futures::future;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use reqwest::Client;
use serde_json::Value;
use shared_expiry_get::Expiry;
use shared_expiry_get::ExpiryFut;
use shared_expiry_get::ExpiryGetError;
use shared_expiry_get::Provider;
use std::sync::Arc;

#[derive(Clone)]
pub struct BearerBearer {
    pub bearer_token_str: Arc<String>,
    pub exp: Arc<DateTime<Utc>>,
}

impl Expiry for BearerBearer {
    fn valid(&self) -> bool {
        *self.exp > Utc::now()
    }
}

pub struct Auth0 {
    pub config: Arc<ClientConfig>,
}

impl Auth0 {
    pub fn new(config: ClientConfig) -> Self {
        Auth0 {
            config: Arc::new(config),
        }
    }
}

impl Provider<BearerBearer> for Auth0 {
    fn update(&self) -> ExpiryFut<BearerBearer> {
        log::debug!("update");
        get_raw_access_token(Arc::clone(&self.config))
            .map_err(|e| ExpiryGetError::UpdateFailed(e.to_string()))
            .and_then(|token| {
                let exp = match get_expiration(&token) {
                    Ok(exp) => exp,
                    Err(e) => return future::err(ExpiryGetError::UpdateFailed(e.to_string())),
                };
                log::debug!("bearer");
                future::ok(BearerBearer {
                    bearer_token_str: token,
                    exp: Arc::new(exp),
                })
            })
            .boxed()
    }
}

fn get_expiration(token: &str) -> Result<DateTime<Utc>, Error> {
    let c: jws::Compact<biscuit::ClaimsSet<Value>, biscuit::Empty> =
        jws::Compact::new_encoded(&token);
    let payload = c.unverified_payload()?;
    let exp = payload
        .registered
        .expiry
        .ok_or_else(|| TokenError::NoExpiry)?;
    Ok(*exp)
}

pub async fn get_raw_access_token(client_config: Arc<ClientConfig>) -> Result<Arc<String>, Error> {
    log::debug!("get raw access token");
    let query = &[
        ("client_id", client_config.client_id.as_str()),
        ("client_secret", client_config.client_secret.as_str()),
        ("audience", client_config.audience.as_str()),
        ("grant_type", "client_credentials"),
        ("scope", client_config.scopes.as_str()),
    ];
    let client = Client::new();
    let res = client
        .post(&client_config.token_endpoint)
        .form(query)
        .send()
        .await?;
    log::debug!("got raw res");
    let j = res.json::<Value>().await?;
    log::debug!("got raw access token");
    j["access_token"]
        .as_str()
        .map(ToOwned::to_owned)
        .map(Arc::new)
        .ok_or_else(|| TokenError::NoToken.into())
}
