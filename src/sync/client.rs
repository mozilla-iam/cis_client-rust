use crate::client::CisClient;
use crate::encoding::USERINFO_ENCODE_SET;
use crate::error::ProfileError;
use crate::getby::GetBy;
use crate::sync::batch::Batch;
use crate::sync::batch::NextPage;
use crate::sync::batch::ProfileIter;
use cis_profile::crypto::SecretStore;
use cis_profile::schema::Profile;
use failure::Error;
use futures::executor::block_on;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use percent_encoding::utf8_percent_encode;
use reqwest::Client;
use reqwest::Response;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;

pub trait CisClientTrait {
    type PI: Iterator<Item = Result<Vec<Profile>, Error>>;
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> Result<Profile, Error>;
    fn get_inactive_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, Error>;
    fn get_users_iter(&self, filter: Option<&str>) -> Result<Self::PI, Error>;
    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Result<Batch, Error>;
    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, Error>;
    fn update_users(&self, profiles: &[Profile]) -> Result<Value, Error>;
    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, Error>;
    fn get_secret_store(&self) -> &SecretStore;
}

impl CisClient {
    fn get_user_sync(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
        active: bool,
    ) -> Result<Profile, Error> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let base = Url::parse(&self.person_api_user_endpoint)?;
        let url = base
            .join(by.as_str())
            .and_then(|u| u.join(&safe_id))
            .map(|mut u| {
                if let Some(df) = filter {
                    u.query_pairs_mut().append_pair("filterDisplay", df);
                }
                u.query_pairs_mut()
                    .append_pair("active", &active.to_string());
                u
            })?;
        let profile: Profile = self.get(url)?;
        if profile.uuid.value.is_none() {
            return Err(ProfileError::ProfileDoesNotExist.into());
        }
        Ok(profile)
    }
    fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T, Error> {
        block_on(async move {
            let token = self.bearer_token().await?;
            let client = Client::new().get(url.as_str()).bearer_auth(token);
            let res = client.send().err_into().map(flatten_status).await?;
            res.json().err_into().await
        })
    }
    fn post<T: DeserializeOwned, P: Serialize>(&self, url: Url, payload: P) -> Result<T, Error> {
        block_on(async move {
            let token = self.bearer_token().await?;
            let client = Client::new().post(url).json(&payload).bearer_auth(token);
            let res = client.send().err_into().map(flatten_status).await?;
            res.json().err_into().await
        })
    }
    fn delete<T: DeserializeOwned, P: Serialize>(&self, url: Url, payload: P) -> Result<T, Error> {
        block_on(async move {
            let token = self.bearer_token().await?;
            let client = Client::new().delete(url).json(&payload).bearer_auth(token);
            let res = client.send().err_into().map(flatten_status).await?;
            res.json().err_into().await
        })
    }
}

impl CisClientTrait for CisClient {
    type PI = ProfileIter<CisClient>;

    fn get_inactive_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, Error> {
        self.get_user_sync(id, by, filter, false)
    }
    fn get_user_by(&self, id: &str, by: &GetBy, filter: Option<&str>) -> Result<Profile, Error> {
        self.get_user_sync(id, by, filter, true)
    }

    fn get_users_iter(&self, filter: Option<&str>) -> Result<Self::PI, Error> {
        let p = ProfileIter::new(self.clone(), filter.map(String::from));
        Ok(p)
    }

    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Result<Batch, Error> {
        let mut url = Url::parse(&self.person_api_users_endpoint)?;
        if let Some(df) = filter {
            url.query_pairs_mut().append_pair("filterDisplay", df);
        }
        if let Some(next_page_token) = next_page {
            let next_page_json = serde_json::to_string(next_page_token)?;
            let safe_next_page =
                utf8_percent_encode(&next_page_json, USERINFO_ENCODE_SET).to_string();
            url.set_query(Some(&format!("nextPage={}", safe_next_page)));
        }
        println!("{}", url.as_str());
        let mut json: Value = self.get(url)?;
        let items: Option<Vec<Profile>> = Some(serde_json::from_value(json["Items"].take())?);
        let next_page: Option<NextPage> = serde_json::from_value(json["nextPage"].take()).ok();
        Ok(Batch { items, next_page })
    }

    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, Error> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = Url::parse(&self.change_api_user_endpoint)?;
        url.set_query(Some(&format!("user_id={}", safe_id)));
        self.post(url, profile)
    }

    fn update_users(&self, profiles: &[Profile]) -> Result<Value, Error> {
        let url = Url::parse(&self.change_api_users_endpoint)?;
        for chunk in profiles.chunks(self.batch_size) {
            self.post(url.clone(), chunk)?;
        }
        Ok(json!({ "status": "all good" }))
    }

    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, Error> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = Url::parse(&self.change_api_user_endpoint)?;
        url.set_query(Some(&format!("user_id={}", safe_id)));
        self.delete(url, profile)
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
