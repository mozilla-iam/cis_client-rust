use crate::error::SecretsError;
use crate::settings::CisSettings;
use crate::settings::Keys;
use cis_profile::crypto::SecretStore;
use failure::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

pub async fn get_store_from_settings(settings: &CisSettings) -> Result<SecretStore, Error> {
    let mut store = SecretStore::default();
    store = match settings.sign_keys.source.as_str() {
        "none" => store,
        "file" => add_sign_keys_from_files(&settings.sign_keys, store)?,
        "ssm" => add_sign_keys_from_ssm(&settings.sign_keys, store).await?,
        _ => return Err(SecretsError::UseNoneFileSsm.into()),
    };
    store = match (
        settings.verify_keys.source.as_str(),
        &settings.verify_keys.well_known_iam_endpoint,
    ) {
        ("none", _) => store,
        ("file", _) => add_verify_keys_from_files(&settings.verify_keys, store)?,
        ("ssm", _) => add_verify_keys_from_ssm(&settings.verify_keys, store).await?,
        ("well_known", Some(url)) => store.with_verify_keys_from_well_known(url).await?,
        _ => {
            return Err(SecretsError::UseNoneFileSsmWellKnonw.into());
        }
    };
    Ok(store)
}

pub async fn add_sign_keys_from_ssm(keys: &Keys, store: SecretStore) -> Result<SecretStore, Error> {
    let key_tuples = get_key_tuples(keys);
    store.with_sign_keys_from_ssm_iter(key_tuples).await
}

pub async fn add_verify_keys_from_ssm(
    keys: &Keys,
    store: SecretStore,
) -> Result<SecretStore, Error> {
    let key_tuples = get_key_tuples(keys);
    store.with_verify_keys_from_ssm_iter(key_tuples).await
}

pub fn add_sign_keys_from_files(keys: &Keys, store: SecretStore) -> Result<SecretStore, Error> {
    let key_tuples = get_key_tuples(keys)
        .into_iter()
        .map(|(k, v)| read_file(&v).map(|content| (k, content)))
        .collect::<Result<Vec<(String, String)>, Error>>()?;
    store.with_sign_keys_from_inline_iter(key_tuples)
}

pub fn add_verify_keys_from_files(keys: &Keys, store: SecretStore) -> Result<SecretStore, Error> {
    let key_tuples = get_key_tuples(keys)
        .into_iter()
        .map(|(k, v)| read_file(&v).map(|content| (k, content)))
        .collect::<Result<Vec<(String, String)>, Error>>()?;
    store.with_verify_keys_from_inline_iter(key_tuples)
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

fn read_file(file_name: &str) -> Result<String, Error> {
    let file = File::open(file_name)?;
    let mut buf_reader = BufReader::new(file);
    let mut content = String::new();
    buf_reader.read_to_string(&mut content)?;
    Ok(content)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn secret_store_from_empty() -> Result<(), Error> {
        let cis_settings = CisSettings::default();
        assert!(get_store_from_settings(&cis_settings).await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn secret_store_from_empty_with_none_setting() -> Result<(), Error> {
        let mut cis_settings = CisSettings::default();
        cis_settings.sign_keys.source = String::from("none");
        cis_settings.verify_keys.source = String::from("none");

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
