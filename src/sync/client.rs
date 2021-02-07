use crate::client::CisClient;
use crate::encoding::USERINFO_ENCODE_SET;
use crate::error::CisClientError;
use crate::error::ProfileError;
use crate::getby::GetBy;
use crate::sync::batch::Batch;
use crate::sync::batch::NextPage;
use crate::sync::batch::ProfileIter;
use cis_profile::crypto::SecretStore;
use cis_profile::schema::Profile;
use log::info;
use percent_encoding::utf8_percent_encode;
use reqwest::blocking::Client;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;

pub trait CisClientTrait {
    type PI: Iterator<Item = Result<Vec<Profile>, CisClientError>>;
    fn get_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, CisClientError>;
    fn get_inactive_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, CisClientError>;
    fn get_any_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, CisClientError>;
    fn get_users_iter(&self, filter: Option<&str>) -> Result<Self::PI, CisClientError>;
    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Result<Batch, CisClientError>;
    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, CisClientError>;
    fn update_users(&self, profiles: &[Profile]) -> Result<Value, CisClientError>;
    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, CisClientError>;
    fn get_secret_store(&self) -> &SecretStore;
}

impl CisClient {
    fn get_user_sync(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
        active: Option<bool>,
    ) -> Result<Profile, CisClientError> {
        let active = match active {
            None => String::from("any"),
            Some(b) => b.to_string(),
        };
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let url = self
            .person_api_user_endpoint
            .clone()
            .join(by.as_str())
            .and_then(|u| u.join(safe_id.trim_start_matches('.')))
            .map(|mut u| {
                if let Some(df) = filter {
                    u.query_pairs_mut().append_pair("filterDisplay", df);
                }
                u.query_pairs_mut().append_pair("active", &active);
                u
            })?;
        let profile: Profile = self.get(url)?;
        if profile.uuid.value.is_none() {
            return Err(ProfileError::ProfileDoesNotExist.into());
        }
        Ok(profile)
    }
    fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T, CisClientError> {
        let token = self.bearer_token_sync()?;
        let client = Client::new().get(url.as_str()).bearer_auth(token);
        let res = client.send()?.error_for_status()?;
        res.json().map_err(Into::into)
    }
    fn post<T: DeserializeOwned, P: Serialize>(
        &self,
        url: Url,
        payload: P,
    ) -> Result<T, CisClientError> {
        let token = self.bearer_token_sync()?;
        let client = Client::new().post(url).json(&payload).bearer_auth(token);
        let res = client.send()?.error_for_status()?;
        res.json().map_err(Into::into)
    }
    fn delete<T: DeserializeOwned, P: Serialize>(
        &self,
        url: Url,
        payload: P,
    ) -> Result<T, CisClientError> {
        let token = self.bearer_token_sync()?;
        let client = Client::new().delete(url).json(&payload).bearer_auth(token);
        let res = client.send()?.error_for_status()?;
        res.json().map_err(Into::into)
    }
}

impl CisClientTrait for CisClient {
    type PI = ProfileIter<CisClient>;

    fn get_inactive_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, CisClientError> {
        self.get_user_sync(id, by, filter, Some(false))
    }
    fn get_any_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, CisClientError> {
        self.get_user_sync(id, by, filter, None)
    }
    fn get_user_by(
        &self,
        id: &str,
        by: &GetBy,
        filter: Option<&str>,
    ) -> Result<Profile, CisClientError> {
        self.get_user_sync(id, by, filter, Some(true))
    }

    fn get_users_iter(&self, filter: Option<&str>) -> Result<Self::PI, CisClientError> {
        let p = ProfileIter::new(self.clone(), filter.map(String::from));
        Ok(p)
    }

    fn get_batch(
        &self,
        next_page: &Option<NextPage>,
        filter: &Option<String>,
    ) -> Result<Batch, CisClientError> {
        let mut url = self.person_api_users_endpoint.clone();
        if let Some(df) = filter {
            url.query_pairs_mut().append_pair("filterDisplay", df);
        }
        if let Some(next_page_token) = next_page {
            let next_page_json = serde_json::to_string(next_page_token)?;
            let safe_next_page =
                utf8_percent_encode(&next_page_json, USERINFO_ENCODE_SET).to_string();
            url.set_query(Some(&format!("nextPage={}", safe_next_page)));
        }
        info!("{}", url.as_str());
        let mut json: Value = self.get(url)?;
        let raw_items: Value = json["Items"].take();
        let items: Option<Vec<Profile>> = match raw_items {
            Value::Array(items) => Some(
                items
                    .into_iter()
                    .filter_map(|item| serde_json::from_value::<Profile>(item).ok())
                    .collect(),
            ),
            _ => None,
        };
        let next_page: Option<NextPage> = serde_json::from_value(json["nextPage"].take()).ok();
        Ok(Batch { items, next_page })
    }

    fn update_user(&self, id: &str, profile: Profile) -> Result<Value, CisClientError> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = self.change_api_user_endpoint.clone();
        url.set_query(Some(&format!("user_id={}", safe_id)));
        self.post(url, profile)
    }

    fn update_users(&self, profiles: &[Profile]) -> Result<Value, CisClientError> {
        let url = self.change_api_users_endpoint.clone();
        for chunk in profiles.chunks(self.batch_size) {
            self.post(url.clone(), chunk)?;
        }
        Ok(json!({ "status": "all good" }))
    }

    fn delete_user(&self, id: &str, profile: Profile) -> Result<Value, CisClientError> {
        let safe_id = utf8_percent_encode(id, USERINFO_ENCODE_SET).to_string();
        let mut url = self.change_api_user_endpoint.clone();
        url.set_query(Some(&format!("user_id={}", safe_id)));
        self.delete(url, profile)
    }

    fn get_secret_store(&self) -> &SecretStore {
        &self.secret_store
    }
}
