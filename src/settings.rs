use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeySource {
    None,
    File,
    Ssm,
    WellKnown,
}

impl Default for KeySource {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
    pub audience: String,
    pub token_endpoint: Url,
    pub scopes: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            client_id: Default::default(),
            client_secret: Default::default(),
            audience: Default::default(),
            token_endpoint: Url::parse("https://auth.mozilla.auth0.com/oauth/token").unwrap(),
            scopes: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Keys {
    pub source: KeySource,
    pub well_known_iam_endpoint: Option<Url>,
    pub mozilliansorg_key: Option<String>,
    pub hris_key: Option<String>,
    pub ldap_key: Option<String>,
    pub cis_key: Option<String>,
    pub access_provider_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CisSettings {
    pub person_api_user_endpoint: Url,
    pub person_api_users_endpoint: Url,
    pub change_api_user_endpoint: Url,
    pub change_api_users_endpoint: Url,
    pub client_config: ClientConfig,
    pub sign_keys: Keys,
    pub verify_keys: Keys,
}

impl Default for CisSettings {
    fn default() -> Self {
        CisSettings {
            person_api_user_endpoint: Url::parse("https://person.api.sso.mozilla.com/v2/user")
                .unwrap(),
            person_api_users_endpoint: Url::parse("https://person.api.sso.mozilla.com/v2/users")
                .unwrap(),
            change_api_user_endpoint: Url::parse("https://change.api.sso.mozilla.com/v2/user")
                .unwrap(),
            change_api_users_endpoint: Url::parse("https://change.api.sso.mozilla.com/v2/users")
                .unwrap(),
            client_config: Default::default(),
            sign_keys: Default::default(),
            verify_keys: Default::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn keys_default() {
        Keys::default();
    }

    #[test]
    fn cis_settings_default() {
        CisSettings::default();
    }

    #[test]
    fn client_config_default() {
        ClientConfig::default();
    }
}
