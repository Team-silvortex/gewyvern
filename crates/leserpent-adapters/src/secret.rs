use std::collections::BTreeMap;
use std::env;
use std::fmt;

use zeroize::Zeroize;

use crate::validate_id;

pub const MAX_SECRET_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn new(value: impl Into<String>) -> Result<Self, SecretStoreError> {
        let value = value.into();
        validate_id("secret key", &value).map_err(|_| SecretStoreError::InvalidKey)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Result<Self, SecretStoreError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_SECRET_BYTES || value.contains(['\r', '\n']) {
            return Err(SecretStoreError::InvalidValue);
        }
        Ok(Self(value))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecretStoreError {
    InvalidKey,
    InvalidValue,
    InvalidEnvironmentName,
    Unavailable,
}

pub trait SecretStore: Send + Sync {
    fn load(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError>;
}

#[derive(Default)]
pub struct EmptySecretStore;

impl SecretStore for EmptySecretStore {
    fn load(&self, _key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError> {
        Ok(None)
    }
}

#[derive(Default)]
pub struct ConfiguredSecretStore {
    values: BTreeMap<SecretKey, SecretValue>,
}

impl ConfiguredSecretStore {
    pub fn new(
        values: impl IntoIterator<Item = (SecretKey, SecretValue)>,
    ) -> Result<Self, SecretStoreError> {
        let mut store = Self::default();
        for (key, value) in values {
            if store.values.insert(key, value).is_some() {
                return Err(SecretStoreError::InvalidKey);
            }
        }
        Ok(store)
    }
}

impl SecretStore for ConfiguredSecretStore {
    fn load(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError> {
        self.values
            .get(key)
            .map(|value| SecretValue::new(value.expose_secret()))
            .transpose()
    }
}

#[derive(Default)]
pub struct EnvironmentSecretStore {
    variables: BTreeMap<SecretKey, String>,
}

impl EnvironmentSecretStore {
    pub fn new(
        variables: impl IntoIterator<Item = (SecretKey, String)>,
    ) -> Result<Self, SecretStoreError> {
        let mut store = Self::default();
        for (key, variable) in variables {
            if !valid_environment_name(&variable) || store.variables.insert(key, variable).is_some()
            {
                return Err(SecretStoreError::InvalidEnvironmentName);
            }
        }
        Ok(store)
    }
}

impl SecretStore for EnvironmentSecretStore {
    fn load(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError> {
        let Some(variable) = self.variables.get(key) else {
            return Ok(None);
        };
        let Some(value) = env::var_os(variable) else {
            return Ok(None);
        };
        let value = value
            .into_string()
            .map_err(|_| SecretStoreError::Unavailable)?;
        SecretValue::new(value).map(Some)
    }
}

fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_values_are_redacted_and_stores_are_alias_scoped() {
        let key = SecretKey::new("runtime-a-admin").unwrap();
        let value = SecretValue::new("not-for-logs").unwrap();
        assert_eq!(format!("{value:?}"), "SecretValue([REDACTED])");
        let store = ConfiguredSecretStore::new([(key.clone(), value)]).unwrap();
        assert_eq!(
            store.load(&key).unwrap().unwrap().expose_secret(),
            "not-for-logs"
        );
        assert!(
            store
                .load(&SecretKey::new("runtime-b-admin").unwrap())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn invalid_secret_metadata_fails_closed() {
        assert!(SecretKey::new("bad/key").is_err());
        assert!(SecretValue::new("").is_err());
        assert!(SecretValue::new("line\nbreak").is_err());
        assert!(
            EnvironmentSecretStore::new([(
                SecretKey::new("runtime-a-admin").unwrap(),
                "bad-name".to_string(),
            )])
            .is_err()
        );
    }
}
