use crate::settings::CisSettings;
use crate::settings::Keys;
use cis_profile::crypto::SecretStore;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

pub fn get_store_from_settings(settings: &CisSettings) -> Result<SecretStore, String> {
    let mut store = SecretStore::default();
    store = match settings.sign_keys.source.as_str() {
        "file" => add_sign_keys_from_files(&settings.sign_keys, store)?,
        "ssm" => add_sign_keys_from_ssm(&settings.sign_keys, store)?,
        _ => return Err(String::from("invalid sign key source: use 'file' or 'ssm'")),
    };
    store = match (
        settings.verify_keys.source.as_str(),
        &settings.verify_keys.well_known_iam_endpoint,
    ) {
        ("file", _) => add_verify_keys_from_files(&settings.verify_keys, store)?,
        ("ssm", _) => add_verify_keys_from_ssm(&settings.verify_keys, store)?,
        ("well_known", Some(url)) => store.with_verify_keys_from_well_known(&url)?,
        _ => {
            return Err(String::from(
                "invalid verify key source: use 'well_known', 'file' or 'ssm'",
            ));
        }
    };
    Ok(store)
}

pub fn add_sign_keys_from_ssm(keys: &Keys, store: SecretStore) -> Result<SecretStore, String> {
    let key_tuples = get_key_tuples(keys);
    store.with_sign_keys_from_ssm_iter(key_tuples)
}

pub fn add_verify_keys_from_ssm(keys: &Keys, store: SecretStore) -> Result<SecretStore, String> {
    let key_tuples = get_key_tuples(keys);
    store.with_verify_keys_from_ssm_iter(key_tuples)
}

pub fn add_sign_keys_from_files(keys: &Keys, store: SecretStore) -> Result<SecretStore, String> {
    let key_tuples = get_key_tuples(keys)
        .into_iter()
        .map(|(k, v)| read_file(&v).map(|content| (k, content)))
        .collect::<Result<Vec<(String, String)>, String>>()?;
    store.with_sign_keys_from_inline_iter(key_tuples)
}

pub fn add_verify_keys_from_files(keys: &Keys, store: SecretStore) -> Result<SecretStore, String> {
    let key_tuples = get_key_tuples(keys)
        .into_iter()
        .map(|(k, v)| read_file(&v).map(|content| (k, content)))
        .collect::<Result<Vec<(String, String)>, String>>()?;
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

fn read_file(file_name: &str) -> Result<String, String> {
    let file =
        File::open(file_name).map_err(|e| format!("unable to open file '{}': {}", file_name, e))?;
    let mut buf_reader = BufReader::new(file);
    let mut content = String::new();
    buf_reader
        .read_to_string(&mut content)
        .map_err(|e| format!("unable to read file '{}': {}", file_name, e))?;
    Ok(content)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn secret_store_from_empty() -> Result<(), String> {
        let cis_settings = CisSettings::default();
        assert!(get_store_from_settings(&cis_settings).is_err());
        Ok(())
    }

    #[test]
    fn test_read_file() -> Result<(), String> {
        let expected = include_str!("../tests/data/fake_key.json");
        let content = read_file("tests/data/fake_key.json")?;
        assert_eq!(expected, content);
        Ok(())
    }
}
