#[derive(Debug, Fail)]
pub enum SecretsError {
    #[fail(display = "invalid sign key source: use 'none', 'file' or 'ssm'")]
    UseNoneFileSsm,
    #[fail(display = "invalid sign key source: use 'none', 'file' or 'ssm'")]
    UseNoneFileSsmWellKnonw,
}

#[derive(Debug, Fail)]
pub enum TokenError {
    #[fail(display = "no expiry set")]
    NoExpiry,
    #[fail(display = "no token :/")]
    NoToken,
}

#[derive(Debug, Fail)]
pub enum ProfileIterError {
    #[fail(display = "invalid profile iter state")]
    InvalidState,
}

#[derive(Debug, Fail)]
pub enum ProfileError {
    #[fail(display = "profile does not exist")]
    ProfileDoesNotExist,
}
