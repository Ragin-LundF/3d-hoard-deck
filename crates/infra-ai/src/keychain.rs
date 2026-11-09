use hoarddeck_application::ports::SecretStore;

const SERVICE: &str = "3D Hoard Deck";

/// API keys in the system's keychain, one per AI provider.
pub struct Keychain;

impl SecretStore for Keychain {
    fn secret(&self, name: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(|error| error.to_string())?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn set_secret(&self, name: &str, value: Option<&str>) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(|error| error.to_string())?;
        match value {
            Some(secret) => entry.set_password(secret).map_err(|error| error.to_string()),
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(error.to_string()),
            },
        }
    }
}
