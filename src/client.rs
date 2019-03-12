use crate::auth::BearerBearer;
use crate::batch::Batch;
use crate::batch::NextPage;
use crate::batch::ProfileIter;
use crate::secrets::get_store_from_settings;
use crate::settings::CisSettings;
use cis_profile::crypto::SecretStore;
use cis_profile::schema::Profile;
use condvar_store::CondvarStore;
use percent_encoding::utf8_percent_encode;
use percent_encoding::USERINFO_ENCODE_SET;
use reqwest::Client;
use reqwest::Url;
use serde_json::Value;
use std::sync::Arc;

static DEFAULT_BATCH_SIZE: usize = 25;

#[allow(dead_code)]
pub enum GetBy {
    Uuid,
    UserId,
    PrimaryEmail,
    PrimaryUsername,
}

impl GetBy {
    pub fn as_str(self: &GetBy) -> &'static str {
        match self {
            GetBy::Uuid => "uuid/",
            GetBy::UserId => "user_id/",
            GetBy::PrimaryEmail => "primary_email/",
            GetBy::PrimaryUsername => "primary_username/",
        }
    }
}

pub trait CisClientTrait {
    type PI: Iterator<Item = Result<Vec<Profile>, String>>;
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> Result<Profile, String>;
    fn get_users_iter(&self, filter: Option<&str>) -> Result<Self::PI, String>;
    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Result<Batch, String>;
    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, String>;
    fn update_users(&self, profiles: &[Profile]) -> Result<Value, String>;
    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, String>;
    fn get_secret_store(&self) -> &SecretStore;
}

#[derive(Clone)]
pub struct CisClient {
    pub bearer_store: CondvarStore<BearerBearer>,
    pub person_api_user_endpoint: String,
    pub person_api_users_endpoint: String,
    pub change_api_user_endpoint: String,
    pub change_api_users_endpoint: String,
    pub secret_store: Arc<SecretStore>,
    pub batch_size: usize,
}

impl CisClient {
    pub fn from_settings(settings: &CisSettings) -> Result<Self, String> {
        let bearer_store = CondvarStore::new(BearerBearer::new(settings.client_config.clone()));
        let secret_store = get_store_from_settings(settings)?;
        Ok(CisClient {
            bearer_store,
            person_api_user_endpoint: settings
                .person_api_user_endpoint
                .clone()
                .unwrap_or_default(),
            person_api_users_endpoint: settings
                .person_api_users_endpoint
                .clone()
                .unwrap_or_default(),
            change_api_user_endpoint: settings
                .change_api_user_endpoint
                .clone()
                .unwrap_or_default(),
            change_api_users_endpoint: settings
                .change_api_users_endpoint
                .clone()
                .unwrap_or_default(),
            secret_store: Arc::new(secret_store),
            batch_size: DEFAULT_BATCH_SIZE,
        })
    }

    fn bearer_token(&self) -> Result<String, String> {
        let b = self
            .bearer_store
            .get()
            .map_err(|e| format!("{}: {}", "unable to get token", e))?;
        let b1 = b
            .read()
            .map_err(|e| format!("{}: {}", "unable to read token", e))?;
        Ok((*b1.bearer_token_str).to_owned())
    }
}

impl CisClientTrait for CisClient {
    type PI = ProfileIter<CisClient>;
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> Result<Profile, String> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let base = Url::parse(&self.person_api_user_endpoint).map_err(|e| format!("{}", e))?;
        let url = base
            .join(by.as_str())
            .and_then(|u| u.join(&safe_id))
            .map(|mut u| {
                if let Some(df) = filter {
                    u.set_query(Some(&format!("filterDisplay={}", df.to_string())))
                }
                u
            })
            .map_err(|e| format!("{}", e))?;
        let token = self.bearer_token()?;
        let client = Client::new().get(url.as_str()).bearer_auth(token);
        let mut res: reqwest::Response = client.send().map_err(|e| format!("{}", e))?;
        if res.status().is_success() {
            res.json()
                .map_err(|e| format!("Invalid JSON from user endpoint: {}", e))
        } else {
            Err(format!("person API returned: {}", res.status()))
        }
    }

    fn get_users_iter(&self, filter: Option<&str>) -> Result<Self::PI, String> {
        let p = ProfileIter::new(self.clone(), filter.map(String::from));
        Ok(p)
    }

    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Result<Batch, String> {
        let mut url = Url::parse(&self.person_api_users_endpoint).map_err(|e| format!("{}", e))?;
        if let Some(df) = filter {
            url.set_query(Some(&format!("filterDisplay={}", df.to_string())))
        }
        if let Some(next_page_token) = next_page {
            let next_page_json = serde_json::to_string(next_page_token)
                .map_err(|e| format!("unable to serialize nextPage token: {}", e))?;
            let safe_next_page =
                utf8_percent_encode(&next_page_json, USERINFO_ENCODE_SET).to_string();
            url.set_query(Some(&format!("nextPage={}", safe_next_page)));
        }
        println!("{}", url.as_str());
        let token = self.bearer_token()?;
        let client = Client::new().get(url.as_str()).bearer_auth(token);
        let mut res: reqwest::Response = client
            .send()
            .map_err(|e| format!("error during request to {:?}: {}", e.url(), e))?;
        if res.status().is_success() {
            let mut json: Value = res
                .json()
                .map_err(|e| format!("Invalid JSON from user endpoint: {}", e))?;
            let items: Option<Vec<Profile>> = Some(
                serde_json::from_value(json["Items"].take())
                    .map_err(|e| format!("unable to parse profiles: {}", e))?,
            );
            let next_page: Option<NextPage> = serde_json::from_value(json["nextPage"].take()).ok();
            return Ok(Batch { items, next_page });
        }
        Err(format!("person API returned: {}", res.status()))
    }

    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, String> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let token = self.bearer_token()?;
        let mut url = Url::parse(&self.change_api_user_endpoint).map_err(|e| format!("{}", e))?;
        url.set_query(Some(&format!("user_id={}", safe_id)));
        let client = Client::new().post(url).json(&profile).bearer_auth(token);
        let mut res: reqwest::Response = client.send().map_err(|e| format!("change.api: {}", e))?;
        res.json()
            .map_err(|e| format!("change.api → json: {} ({:?})", e, res))
    }

    fn update_users(&self, profiles: &[Profile]) -> Result<Value, String> {
        for chunk in profiles.chunks(self.batch_size) {
            let token = self.bearer_token()?;
            let client = Client::new()
                .post(&self.change_api_users_endpoint)
                .json(&chunk)
                .bearer_auth(token);
            let mut res: reqwest::Response = client.send().map_err(|e| format!("{}", e))?;
            res.json().map_err(|e| format!("{}", e))?;
        }
        Ok(json!({ "status": "all good" }))
    }

    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, String> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let token = self.bearer_token()?;
        let mut url = Url::parse(&self.change_api_user_endpoint).map_err(|e| format!("{}", e))?;
        url.set_query(Some(&format!("user_id={}", safe_id)));
        let client = Client::new().delete(url).json(&profile).bearer_auth(token);
        let mut res: reqwest::Response = client.send().map_err(|e| format!("change.api: {}", e))?;
        res.json()
            .map_err(|e| format!("change.api → json: {} ({:?})", e, res))
    }

    fn get_secret_store(&self) -> &SecretStore {
        &self.secret_store
    }
}
