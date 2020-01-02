use crate::auth::Auth0;
use crate::auth::BearerBearer;
use crate::batch::AsyncProfileIter;
use crate::batch::Batch;
use crate::batch::NextPage;
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
use futures::stream::Stream;
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
    pub fn from_settings(settings: &CisSettings) -> Result<Self, Error> {
        let bearer_store = RemoteStore::new(Auth0::new(settings.client_config.clone()));
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

    pub async fn bearer_token(&self) -> Result<String, Error> {
        let b = self.bearer_store.get().await?;
        Ok((*b.bearer_token_str).to_owned())
    }
}

pub trait AsyncCisClientTrait {
    type PI: Stream<Item = Vec<Profile>>;
    fn get_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Box<dyn Future<Output = Result<Profile, Error>>>;
    fn get_inactive_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Box<dyn Future<Output = Result<Profile, Error>>>;
    fn get_users_iter(&self, filter: Option<&str>) -> Box<dyn Stream<Item = Self::PI>>;
    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<Batch, Error>>>>;
    fn update_user(
        &self,
        id: &str,
        profile: Profile,
    ) -> Box<dyn Future<Output = Result<Value, Error>>>;
    fn update_users(&self, profiles: &[Profile]) -> Box<dyn Future<Output = Result<Value, Error>>>;
    fn delete_user(
        &self,
        id: &str,
        profile: Profile,
    ) -> Box<dyn Future<Output = Result<Value, Error>>>;
    fn get_secret_store(&self) -> &SecretStore;
}

async fn send<T: DeserializeOwned>(
    bearer_store: RemoteStore<BearerBearer, Auth0>,
    url: Url,
) -> Result<T, Error> {
    let token = bearer_store.get().await?;
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
    ) -> Box<dyn Future<Output = Result<Profile, Error>>> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let base = match Url::parse(&self.person_api_user_endpoint) {
            Ok(base) => base,
            Err(e) => return Box::new(future::err(e.into())),
        };
        let url = match base
            .join(by.as_str())
            .and_then(|u| u.join(&safe_id))
            .map(|mut u| {
                if let Some(df) = filter {
                    u.query_pairs_mut().append_pair("filterDisplay", df);
                }
                u.query_pairs_mut()
                    .append_pair("active", &active.to_string());
                u
            }) {
            Ok(url) => url,
            Err(e) => return Box::new(future::err(e.into())),
        };
        Box::new(
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
    type PI = AsyncProfileIter<CisClient>;
    fn get_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Box<dyn Future<Output = Result<Profile, Error>>> {
        self.get_user(id, by, filter, true)
    }
    fn get_inactive_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Box<dyn Future<Output = Result<Profile, Error>>> {
        self.get_user(id, by, filter, false)
    }
    fn get_users_iter(&self, _filter: Option<&str>) -> Box<dyn Stream<Item = Self::PI>> {
        unimplemented!()
    }
    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<Batch, Error>>>> {
        let mut url = match Url::parse(&self.person_api_users_endpoint) {
            Ok(base) => base,
            Err(e) => return future::err(e.into()).boxed(),
        };
        if let Some(df) = filter {
            url.query_pairs_mut().append_pair("filterDisplay", df);
        }
        if let Some(next_page_token) = next_page {
            let next_page_json = match serde_json::to_string(next_page_token) {
                Ok(next_page_json) => next_page_json,
                Err(e) => return future::err(e.into()).boxed(),
            };
            let safe_next_page =
                utf8_percent_encode(&next_page_json, USERINFO_ENCODE_SET).to_string();
            url.set_query(Some(&format!("nextPage={}", safe_next_page)));
        }
        send(self.bearer_store.clone(), url)
            .and_then(|mut json: Value| {
                let items: Vec<Profile> = match serde_json::from_value(json["Items"].take()) {
                    Ok(item) => item,
                    Err(e) => return future::err(e.into()),
                };
                let next_page: Option<NextPage> =
                    serde_json::from_value(json["nextPage"].take()).ok();
                future::ok(Batch { items, next_page })
            })
            .boxed()
    }
    fn update_user(
        &self,
        id: &str,
        profile: Profile,
    ) -> Box<dyn Future<Output = Result<Value, Error>>> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = match Url::parse(&self.change_api_user_endpoint) {
            Ok(base) => base,
            Err(e) => return Box::new(future::err(e.into())),
        };
        url.set_query(Some(&format!("user_id={}", safe_id)));
        Box::new(post(self.bearer_store.clone(), url, profile))
    }
    fn update_users(
        &self,
        _profiles: &[Profile],
    ) -> Box<dyn Future<Output = Result<Value, Error>>> {
        unimplemented!()
    }
    fn delete_user(
        &self,
        id: &str,
        profile: Profile,
    ) -> Box<dyn Future<Output = Result<Value, Error>>> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = match Url::parse(&self.change_api_user_endpoint) {
            Ok(base) => base,
            Err(e) => return Box::new(future::err(e.into())),
        };
        url.set_query(Some(&format!("user_id={}", safe_id)));
        Box::new(delete(self.bearer_store.clone(), url, profile))
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
