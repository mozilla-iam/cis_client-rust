use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
    pub audience: String,
    pub token_endpoint: String,
    pub scopes: String,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Keys {
    pub source: String,
    pub well_known_iam_endpoint: Option<String>,
    pub mozilliansorg_key: Option<String>,
    pub hris_key: Option<String>,
    pub ldap_key: Option<String>,
    pub cis_key: Option<String>,
    pub access_provider_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct CisSettings {
    pub person_api_user_endpoint: Option<String>,
    pub person_api_users_endpoint: Option<String>,
    pub change_api_user_endpoint: Option<String>,
    pub change_api_users_endpoint: Option<String>,
    pub client_config: ClientConfig,
    pub sign_keys: Keys,
    pub verify_keys: Keys,
}
