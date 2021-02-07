use crate::error::SecretsError;
use crate::settings::CisSettings;
use crate::settings::KeySource;
use crate::settings::Keys;
use cis_profile::crypto::SecretStore;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

pub async fn get_store_from_settings(settings: &CisSettings) -> Result<SecretStore, SecretsError> {
    let mut store = SecretStore::default();
    store = match settings.sign_keys.source {
        KeySource::None => store,
        KeySource::File => add_sign_keys_from_files(&settings.sign_keys, store)?,
        KeySource::Ssm => add_sign_keys_from_ssm(&settings.sign_keys, store).await?,
        _ => return Err(SecretsError::UseNoneFileSsm),
    };
    store = match (
        &settings.verify_keys.source,
        &settings.verify_keys.well_known_iam_endpoint,
    ) {
        (KeySource::None, _) => store,
        (KeySource::File, _) => add_verify_keys_from_files(&settings.verify_keys, store)?,
        (KeySource::Ssm, _) => add_verify_keys_from_ssm(&settings.verify_keys, store).await?,
        (KeySource::WellKnown, Some(url)) => {
            store.with_verify_keys_from_well_known(url.as_str()).await?
        }
        _ => {
            return Err(SecretsError::UseNoneFileSsmWellKnonw);
        }
    };
    Ok(store)
}

pub async fn add_sign_keys_from_ssm(
    keys: &Keys,
    store: SecretStore,
) -> Result<SecretStore, SecretsError> {
    let key_tuples = get_key_tuples(keys);
    store
        .with_sign_keys_from_ssm_iter(key_tuples)
        .await
        .map_err(Into::into)
}

pub async fn add_verify_keys_from_ssm(
    keys: &Keys,
    store: SecretStore,
) -> Result<SecretStore, SecretsError> {
    let key_tuples = get_key_tuples(keys);
    store
        .with_verify_keys_from_ssm_iter(key_tuples)
        .await
        .map_err(Into::into)
}

pub fn add_sign_keys_from_files(
    keys: &Keys,
    store: SecretStore,
) -> Result<SecretStore, SecretsError> {
    let key_tuples = get_key_tuples(keys)
        .into_iter()
        .map(|(k, v)| read_file(&v).map(|content| (k, content)))
        .collect::<Result<Vec<(String, String)>, SecretsError>>()?;
    store
        .with_sign_keys_from_inline_iter(key_tuples)
        .map_err(Into::into)
}

pub fn add_verify_keys_from_files(
    keys: &Keys,
    store: SecretStore,
) -> Result<SecretStore, SecretsError> {
    let key_tuples = get_key_tuples(keys)
        .into_iter()
        .map(|(k, v)| read_file(&v).map(|content| (k, content)))
        .collect::<Result<Vec<(String, String)>, SecretsError>>()?;
    store
        .with_verify_keys_from_inline_iter(key_tuples)
        .map_err(Into::into)
}

fn get_key_tuples(keys: &Keys) -> Vec<(String, String)> {
    vec![
        ("mozilliansorg", &keys.mozilliansorg_key),
        ("hris", &keys.hris_key),
        ("ldap", &keys.ldap_key),
        ("cis", &keys.cis_key),
        ("access_provider", &keys.access_provider_key),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.clone().map(|v| (k.to_owned(), v)))
    .collect()
}

fn read_file(file_name: &str) -> Result<String, SecretsError> {
    let file = File::open(file_name).map_err(|_| SecretsError::FileReadError)?;
    let mut buf_reader = BufReader::new(file);
    let mut content = String::new();
    buf_reader
        .read_to_string(&mut content)
        .map_err(|_| SecretsError::FileReadError)?;
    Ok(content)
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Error;

    #[tokio::test]
    async fn secret_store_from_empty_with_none_setting() -> Result<(), Error> {
        let cis_settings = CisSettings::default();
        assert!(get_store_from_settings(&cis_settings).await.is_ok());
        Ok(())
    }

    #[test]
    fn test_read_file() -> Result<(), Error> {
        let expected = include_str!("../tests/data/fake_key.json");
        let content = read_file("tests/data/fake_key.json")?;
        assert_eq!(expected, content);
        Ok(())
    }
}
