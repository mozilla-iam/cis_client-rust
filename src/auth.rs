use crate::error::TokenError;
use crate::settings::ClientConfig;
use biscuit::jws;
use chrono::DateTime;
use chrono::Utc;
use failure::Error;
use futures::future;
use futures::Future;
use reqwest::r#async::Client;
use serde_json::json;
use serde_json::Value;
use shared_expiry_get::Expiry;
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
    fn update(&self) -> Box<dyn Future<Item = BearerBearer, Error = Error> + Send> {
        Box::new(get_raw_access_token(&*self.config).and_then(|token| {
            let exp = match get_expiration(&token) {
                Ok(exp) => exp,
                Err(e) => return future::err(e),
            };
            future::ok(BearerBearer {
                bearer_token_str: token,
                exp: Arc::new(exp),
            })
        }))
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

pub fn get_raw_access_token(
    client_config: &ClientConfig,
) -> Box<dyn Future<Item = Arc<String>, Error = Error> + Send> {
    let payload = json!(
        {
            "client_id": client_config.client_id,
            "client_secret": client_config.client_secret,
            "audience": client_config.audience,
            "grant_type": "client_credentials",
            "scopes": client_config.scopes,
        }
    );
    let client = Client::new();
    let res = client
        .post(&client_config.token_endpoint)
        .json(&payload)
        .send()
        .map_err(Into::into);
    Box::new(
        res.and_then(|mut r| r.json().map_err(Into::into))
            .and_then(|j: serde_json::Value| {
                j["access_token"]
                    .as_str()
                    .map(ToOwned::to_owned)
                    .map(Arc::new)
                    .ok_or_else(|| TokenError::NoToken.into())
            }),
    )
}
