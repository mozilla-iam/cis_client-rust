use shared_expiry_get::ExpiryGetError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CisClientError {
    #[error("secrets error: {0}")]
    SecretsError(#[from] SecretsError),
    #[error("token error: {0}")]
    TokenError(#[from] TokenError),
    #[error("profile error: {0}")]
    ProfileError(#[from] ProfileError),
    #[error("error fetching remote: {0}")]
    RemoteError(#[from] ExpiryGetError),
    #[error("request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("url parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("unable to create tokio runtime")]
    RuntimeError,
    #[error("invalid next page token: {0}")]
    InvalidNextPage(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("invalid sign key source: use 'none', 'file' or 'ssm'")]
    UseNoneFileSsm,
    #[error("invalid sign key source: use 'none', 'file' or 'ssm'")]
    UseNoneFileSsmWellKnonw,
    #[error("key error: {0}")]
    KeyError(#[from] cis_profile::error::KeyError),
    #[error("unable to read key from file")]
    FileReadError,
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("no expiry set")]
    NoExpiry,
    #[error("no token :/")]
    NoToken,
    #[error("error fetching token: {0}")]
    FetchError(#[from] reqwest::Error),
    #[error("error parsing token: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("profile does not exist")]
    ProfileDoesNotExist,
    #[error("invalid profile iter state")]
    InvalidIterState,
}
