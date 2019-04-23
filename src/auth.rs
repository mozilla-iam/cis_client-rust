use crate::error::TokenError;
use crate::settings::ClientConfig;
use biscuit::jws;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
use condvar_store::GetExpiry;
use failure::Error;
use reqwest::Client;
use serde_json::Value;

pub struct BearerBearer {
    pub bearer_token_str: String,
    pub exp: DateTime<Utc>,
    pub config: ClientConfig,
}

impl GetExpiry for BearerBearer {
    fn get(&mut self) -> Result<(), Error> {
        self.bearer_token_str = get_raw_access_token(&self.config)?;
        self.exp = get_expiration(&self.bearer_token_str)?;
        Ok(())
    }
    fn expiry(&self) -> DateTime<Utc> {
        self.exp
    }
}

impl BearerBearer {
    pub fn new(config: ClientConfig) -> Self {
        BearerBearer {
            bearer_token_str: String::default(),
            exp: Utc.timestamp(0, 0),
            config,
        }
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

pub fn get_raw_access_token(client_config: &ClientConfig) -> Result<String, Error> {
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
    let mut res = client
        .post(&client_config.token_endpoint)
        .json(&payload)
        .send()?;
    let j: serde_json::Value = res.json()?;
    j["access_token"]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| TokenError::NoToken.into())
}
