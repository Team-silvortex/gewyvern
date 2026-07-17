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

pub struct PlatformSecretStore {
    service: String,
}

impl PlatformSecretStore {
    pub fn new(service: impl Into<String>) -> Result<Self, SecretStoreError> {
        let service = service.into();
        validate_id("secret service", &service)
            .map_err(|_| SecretStoreError::InvalidEnvironmentName)?;
        Ok(Self { service })
    }

    pub fn service(&self) -> &str {
        &self.service
    }
}

impl SecretStore for PlatformSecretStore {
    fn load(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError> {
        platform::load(&self.service, key.as_str())
    }
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

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;
    use std::ptr;
    use std::slice;

    use super::{MAX_SECRET_BYTES, SecretStoreError, SecretValue};

    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25_300;

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecKeychainFindGenericPassword(
            keychain_or_array: *const c_void,
            service_name_length: u32,
            service_name: *const u8,
            account_name_length: u32,
            account_name: *const u8,
            password_length: *mut u32,
            password_data: *mut *mut c_void,
            item_ref: *mut *mut c_void,
        ) -> i32;

        fn SecKeychainItemFreeContent(attribute_list: *const c_void, data: *mut c_void) -> i32;
    }

    pub(super) fn load(
        service: &str,
        account: &str,
    ) -> Result<Option<SecretValue>, SecretStoreError> {
        let service_len = u32::try_from(service.len()).map_err(|_| SecretStoreError::InvalidKey)?;
        let account_len = u32::try_from(account.len()).map_err(|_| SecretStoreError::InvalidKey)?;
        let mut password_len = 0_u32;
        let mut password_data = ptr::null_mut();
        // SAFETY: validated Rust strings remain alive for the call, lengths match their UTF-8
        // byte buffers, and all out-pointers refer to initialized local storage.
        let status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null(),
                service_len,
                service.as_ptr(),
                account_len,
                account.as_ptr(),
                &mut password_len,
                &mut password_data,
                ptr::null_mut(),
            )
        };
        if status == ERR_SEC_ITEM_NOT_FOUND {
            return Ok(None);
        }
        if status != 0 || password_data.is_null() {
            return Err(SecretStoreError::Unavailable);
        }
        let result = if password_len as usize > MAX_SECRET_BYTES {
            Err(SecretStoreError::InvalidValue)
        } else {
            // SAFETY: Security.framework owns a readable password buffer of password_len bytes
            // after a successful lookup; it remains valid until the paired free call below.
            let bytes =
                unsafe { slice::from_raw_parts(password_data.cast::<u8>(), password_len as usize) };
            match std::str::from_utf8(bytes) {
                Ok(value) => SecretValue::new(value).map(Some),
                Err(_) => Err(SecretStoreError::InvalidValue),
            }
        };
        // SAFETY: password_data came from SecKeychainFindGenericPassword and is released once.
        let free_status = unsafe { SecKeychainItemFreeContent(ptr::null(), password_data) };
        if free_status != 0 {
            return Err(SecretStoreError::Unavailable);
        }
        result
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::ffi::{CString, c_char, c_int, c_void};
    use std::mem;
    use std::ptr;
    use std::slice;

    use super::{MAX_SECRET_BYTES, SecretStoreError, SecretValue};

    const SECRET_SCHEMA_DONT_MATCH_NAME: c_int = 2;
    const SECRET_SCHEMA_ATTRIBUTE_STRING: c_int = 0;

    type SecretSchemaNew = unsafe extern "C" fn(*const c_char, c_int, ...) -> *mut c_void;
    type SecretSchemaUnref = unsafe extern "C" fn(*mut c_void);
    type SecretPasswordLookupSync =
        unsafe extern "C" fn(*const c_void, *mut c_void, *mut *mut c_void, ...) -> *mut c_char;
    type SecretPasswordFree = unsafe extern "C" fn(*mut c_char);
    type GErrorFree = unsafe extern "C" fn(*mut c_void);

    struct Libraries {
        secret: *mut c_void,
        glib: *mut c_void,
        schema_new: SecretSchemaNew,
        schema_unref: SecretSchemaUnref,
        password_lookup: SecretPasswordLookupSync,
        password_free: SecretPasswordFree,
        error_free: GErrorFree,
    }

    impl Libraries {
        fn open() -> Result<Self, SecretStoreError> {
            // SAFETY: static NUL-terminated library names are valid for dlopen.
            let secret = unsafe {
                libc::dlopen(
                    c"libsecret-1.so.0".as_ptr(),
                    libc::RTLD_NOW | libc::RTLD_LOCAL,
                )
            };
            if secret.is_null() {
                return Err(SecretStoreError::Unavailable);
            }
            // SAFETY: static NUL-terminated library names are valid for dlopen.
            let glib = unsafe {
                libc::dlopen(
                    c"libglib-2.0.so.0".as_ptr(),
                    libc::RTLD_NOW | libc::RTLD_LOCAL,
                )
            };
            if glib.is_null() {
                // SAFETY: secret is a live handle returned by dlopen.
                unsafe { libc::dlclose(secret) };
                return Err(SecretStoreError::Unavailable);
            }

            let symbols = (|| {
                Ok(Self {
                    secret,
                    glib,
                    schema_new: unsafe {
                        mem::transmute::<*mut c_void, SecretSchemaNew>(symbol(
                            secret,
                            c"secret_schema_new",
                        )?)
                    },
                    schema_unref: unsafe {
                        mem::transmute::<*mut c_void, SecretSchemaUnref>(symbol(
                            secret,
                            c"secret_schema_unref",
                        )?)
                    },
                    password_lookup: unsafe {
                        mem::transmute::<*mut c_void, SecretPasswordLookupSync>(symbol(
                            secret,
                            c"secret_password_lookup_sync",
                        )?)
                    },
                    password_free: unsafe {
                        mem::transmute::<*mut c_void, SecretPasswordFree>(symbol(
                            secret,
                            c"secret_password_free",
                        )?)
                    },
                    error_free: unsafe {
                        mem::transmute::<*mut c_void, GErrorFree>(symbol(glib, c"g_error_free")?)
                    },
                })
            })();
            if symbols.is_err() {
                // SAFETY: both handles are live and have not been transferred into Libraries.
                unsafe {
                    libc::dlclose(glib);
                    libc::dlclose(secret);
                }
            }
            symbols
        }
    }

    impl Drop for Libraries {
        fn drop(&mut self) {
            // SAFETY: handles were opened successfully and are closed exactly once.
            unsafe {
                libc::dlclose(self.glib);
                libc::dlclose(self.secret);
            }
        }
    }

    unsafe fn symbol(
        library: *mut c_void,
        name: &std::ffi::CStr,
    ) -> Result<*mut c_void, SecretStoreError> {
        // SAFETY: library is a live dlopen handle and name is NUL-terminated.
        let value = unsafe { libc::dlsym(library, name.as_ptr()) };
        (!value.is_null())
            .then_some(value)
            .ok_or(SecretStoreError::Unavailable)
    }

    pub(super) fn load(
        service: &str,
        account: &str,
    ) -> Result<Option<SecretValue>, SecretStoreError> {
        let libraries = Libraries::open()?;
        let service = CString::new(service).map_err(|_| SecretStoreError::InvalidKey)?;
        let account = CString::new(account).map_err(|_| SecretStoreError::InvalidKey)?;
        // SAFETY: every variadic argument has the type required by libsecret and the list ends
        // with a null pointer sentinel.
        let schema = unsafe {
            (libraries.schema_new)(
                service.as_ptr(),
                SECRET_SCHEMA_DONT_MATCH_NAME,
                c"service".as_ptr(),
                SECRET_SCHEMA_ATTRIBUTE_STRING,
                c"account".as_ptr(),
                SECRET_SCHEMA_ATTRIBUTE_STRING,
                ptr::null::<c_void>(),
            )
        };
        if schema.is_null() {
            return Err(SecretStoreError::Unavailable);
        }
        let mut error = ptr::null_mut();
        // SAFETY: schema is live, the error out-pointer is initialized, attribute strings are
        // valid C strings, and the variadic list has a null terminator.
        let password = unsafe {
            (libraries.password_lookup)(
                schema,
                ptr::null_mut(),
                &mut error,
                c"service".as_ptr(),
                service.as_ptr(),
                c"account".as_ptr(),
                account.as_ptr(),
                ptr::null::<c_void>(),
            )
        };
        // SAFETY: schema came from secret_schema_new and is released exactly once.
        unsafe { (libraries.schema_unref)(schema) };
        if !error.is_null() {
            // SAFETY: libsecret returned a GError owned by the caller.
            unsafe { (libraries.error_free)(error) };
            if !password.is_null() {
                // SAFETY: a non-null password returned by libsecret uses this paired free API.
                unsafe { (libraries.password_free)(password) };
            }
            return Err(SecretStoreError::Unavailable);
        }
        if password.is_null() {
            return Ok(None);
        }
        // SAFETY: password is NUL-terminated storage from libsecret; strnlen is capped one byte
        // beyond the accepted maximum so malformed or oversized values are rejected.
        let length = unsafe { libc::strnlen(password, MAX_SECRET_BYTES + 1) };
        let result = if length > MAX_SECRET_BYTES {
            Err(SecretStoreError::InvalidValue)
        } else {
            // SAFETY: strnlen proved that length readable bytes precede the NUL terminator.
            let bytes = unsafe { slice::from_raw_parts(password.cast::<u8>(), length) };
            match std::str::from_utf8(bytes) {
                Ok(value) => SecretValue::new(value).map(Some),
                Err(_) => Err(SecretStoreError::InvalidValue),
            }
        };
        // SAFETY: password is non-null and owned by the caller according to libsecret.
        unsafe { (libraries.password_free)(password) };
        result
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use super::{SecretStoreError, SecretValue};

    pub(super) fn load(
        _service: &str,
        _account: &str,
    ) -> Result<Option<SecretValue>, SecretStoreError> {
        Ok(None)
    }
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
        assert!(PlatformSecretStore::new("bad/service").is_err());
        assert_eq!(
            PlatformSecretStore::new("org.gewyvern.leserpent.adapters")
                .unwrap()
                .service(),
            "org.gewyvern.leserpent.adapters"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_keychain_missing_alias_is_a_clean_miss() {
        let store = PlatformSecretStore::new("org.gewyvern.leserpent.tests.missing").unwrap();
        let key = SecretKey::new("missing-7f8c1d2e").unwrap();
        assert!(store.load(&key).unwrap().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_secret_service_lookup_loads_native_abi_and_fails_closed() {
        let store = PlatformSecretStore::new("org.gewyvern.leserpent.tests.missing").unwrap();
        let key = SecretKey::new("missing-7f8c1d2e").unwrap();
        let result = store.load(&key);
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
            assert!(matches!(result, Ok(None)));
        } else {
            assert!(matches!(
                result,
                Ok(None) | Err(SecretStoreError::Unavailable)
            ));
        }
    }
}
