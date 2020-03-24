use crate::auth::Auth0;
use crate::auth::BearerBearer;
use crate::encoding::USERINFO_ENCODE_SET;
use crate::error::ProfileError;
use crate::getby::GetBy;
use crate::secrets::get_store_from_settings;
use crate::settings::CisSettings;
use cis_profile::crypto::SecretStore;
use cis_profile::schema::Profile;
use failure::Error;
use futures::future;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::Future;
use percent_encoding::utf8_percent_encode;
use reqwest::Client;
use reqwest::Response;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use shared_expiry_get::RemoteStore;
use std::pin::Pin;
use std::sync::Arc;

static DEFAULT_BATCH_SIZE: usize = 25;

#[derive(Clone)]
pub struct CisClient {
    pub bearer_store: RemoteStore<BearerBearer, Auth0>,
    pub person_api_user_endpoint: String,
    pub person_api_users_endpoint: String,
    pub change_api_user_endpoint: String,
    pub change_api_users_endpoint: String,
    pub secret_store: Arc<SecretStore>,
    pub batch_size: usize,
}

impl CisClient {
    pub async fn from_settings(settings: &CisSettings) -> Result<Self, Error> {
        let bearer_store = RemoteStore::new(Auth0::new(settings.client_config.clone()));
        let secret_store = get_store_from_settings(settings).await?;
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
    #[cfg(feature = "sync")]
    pub fn from_settings_sync(settings: &CisSettings) -> Result<Self, Error> {
        use tokio::runtime::Runtime;
        let mut rt = Runtime::new()?;
        rt.block_on(Self::from_settings(settings))
    }

    pub async fn bearer_token(&self) -> Result<String, Error> {
        let b = self.bearer_store.get().await?;
        Ok((*b.bearer_token_str).to_owned())
    }

    #[cfg(feature = "sync")]
    pub fn bearer_token_sync(&self) -> Result<String, Error> {
        use tokio::runtime::Runtime;
        let mut rt = Runtime::new()?;
        rt.block_on(self.bearer_token())
    }
}

pub type CisFut<T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send>>;

pub trait AsyncCisClientTrait {
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> CisFut<Profile>;
    fn get_inactive_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> CisFut<Profile>;
    fn update_user(&self, id: &str, profile: Profile) -> CisFut<Value>;
    fn update_users(&self, profiles: &[Profile]) -> CisFut<Value>;
    fn delete_user(&self, id: &str, profile: Profile) -> CisFut<Value>;
    fn get_secret_store(&self) -> &SecretStore;
}

async fn send<T: DeserializeOwned>(
    bearer_store: RemoteStore<BearerBearer, Auth0>,
    url: Url,
) -> Result<T, Error> {
    log::debug!("getting token");
    let token = bearer_store.get().await?;
    log::debug!("got token");
    let res = Client::new()
        .get(url.as_str())
        .bearer_auth(token.bearer_token_str)
        .send()
        .err_into()
        .map(flatten_status)
        .await?;
    res.json().err_into().await
}

async fn post<T: DeserializeOwned>(
    bearer_store: RemoteStore<BearerBearer, Auth0>,
    url: Url,
    payload: impl Serialize,
) -> Result<T, Error> {
    let token = bearer_store.get().await?;
    let res = Client::new()
        .post(url.as_str())
        .json(&payload)
        .bearer_auth(token.bearer_token_str)
        .send()
        .err_into()
        .map(flatten_status)
        .await?;
    res.json().err_into().await
}

async fn delete<T: DeserializeOwned>(
    bearer_store: RemoteStore<BearerBearer, Auth0>,
    url: Url,
    payload: impl Serialize,
) -> Result<T, Error> {
    let token = bearer_store.get().await?;
    let res = Client::new()
        .delete(url.as_str())
        .json(&payload)
        .bearer_auth(token.bearer_token_str)
        .send()
        .err_into()
        .map(flatten_status)
        .await?;
    res.json().err_into().await
}

impl CisClient {
    fn get_user(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
        active: bool,
    ) -> CisFut<Profile> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let base = match Url::parse(&self.person_api_user_endpoint) {
            Ok(base) => base,
            Err(e) => return Box::pin(future::err(e.into())),
        };
        let url = match base
            .join(by.as_str())
            .and_then(|u| u.join(safe_id.trim_start_matches('.')))
            .map(|mut u| {
                if let Some(df) = filter {
                    u.query_pairs_mut().append_pair("filterDisplay", df);
                }
                u.query_pairs_mut()
                    .append_pair("active", &active.to_string());
                u
            }) {
            Ok(url) => url,
            Err(e) => return Box::pin(future::err(e.into())),
        };
        Box::pin(
            send(self.bearer_store.clone(), url).and_then(|profile: Profile| {
                if profile.uuid.value.is_none() {
                    return future::err(ProfileError::ProfileDoesNotExist.into());
                }
                future::ok(profile)
            }),
        )
    }
}

impl AsyncCisClientTrait for CisClient {
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> CisFut<Profile> {
        self.get_user(id, by, filter, true)
    }
    fn get_inactive_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> CisFut<Profile> {
        self.get_user(id, by, filter, false)
    }
    fn update_user(&self, id: &str, profile: Profile) -> CisFut<Value> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = match Url::parse(&self.change_api_user_endpoint) {
            Ok(base) => base,
            Err(e) => return Box::pin(future::err(e.into())),
        };
        url.set_query(Some(&format!("user_id={}", safe_id)));
        Box::pin(post(self.bearer_store.clone(), url, profile))
    }
    fn update_users(&self, _profiles: &[Profile]) -> CisFut<Value> {
        unimplemented!()
    }
    fn delete_user(&self, id: &str, profile: Profile) -> CisFut<Value> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = match Url::parse(&self.change_api_user_endpoint) {
            Ok(base) => base,
            Err(e) => return Box::pin(future::err(e.into())),
        };
        url.set_query(Some(&format!("user_id={}", safe_id)));
        Box::pin(delete(self.bearer_store.clone(), url, profile))
    }
    fn get_secret_store(&self) -> &SecretStore {
        &self.secret_store
    }
}

fn flatten_status(result: Result<Response, Error>) -> Result<Response, Error> {
    match result {
        Ok(res) => res.error_for_status().map_err(Into::into),
        Err(e) => Err(e),
    }
}
