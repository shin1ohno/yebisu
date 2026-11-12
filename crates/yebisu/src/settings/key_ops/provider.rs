use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    Unsupported,
    Locked,
    Missing,
    Format,
    Provider,
    Worker,
    Queue,
    Timeout,
}
pub(crate) const IDENTITY: &str =
    "herdr-ssh|herdr-ssh.replacement|herdr-ssh.retired|EC-private-P256|local|default-app-group";
pub(crate) trait Provider: Send + Sync {
    fn identity(&self) -> &'static str {
        IDENTITY
    }
    fn supported(&self) -> bool;
    fn read_or_initialize(&self) -> Result<String, Failure>;
    fn replace(&self) -> Result<String, Failure>;
}
struct Platform;
pub(super) fn platform() -> Arc<dyn Provider> {
    #[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
    if std::env::var_os("YEBISU_P11_KEYS_QA").is_some() {
        return Arc::new(Injected);
    }
    Arc::new(Platform)
}
impl Provider for Platform {
    fn supported(&self) -> bool {
        cfg!(all(target_os = "ios", not(test)))
    }
    fn read_or_initialize(&self) -> Result<String, Failure> {
        #[cfg(all(target_os = "ios", not(test)))]
        {
            sheprd_client::ssh::authorized_key().map_err(classify)
        }
        #[cfg(any(not(target_os = "ios"), test))]
        {
            Err(Failure::Unsupported)
        }
    }
    fn replace(&self) -> Result<String, Failure> {
        #[cfg(all(target_os = "ios", not(test)))]
        {
            sheprd_client::ssh::replace_authorized_key().map_err(classify)
        }
        #[cfg(any(not(target_os = "ios"), test))]
        {
            Err(Failure::Unsupported)
        }
    }
}
// Match the pinned SDK's fixed, credential-free Display categories. Never log errors.
#[cfg(all(target_os = "ios", not(test)))]
fn classify(error: sheprd_client::Error) -> Failure {
    match error.to_string().as_str() {
        "The iPad is locked. Unlock it and try again." => Failure::Locked,
        "The device key was not in the Keychain after it was created. Try again." => {
            Failure::Missing
        }
        _ => Failure::Provider,
    }
}

#[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
struct Injected;
#[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
impl Provider for Injected {
    fn supported(&self) -> bool {
        true
    }
    fn read_or_initialize(&self) -> Result<String, Failure> {
        Ok("restrict ssh-ed25519 RkFLRV9QMl9RQQ== herdr-ipad-injected-qa".into())
    }
    fn replace(&self) -> Result<String, Failure> {
        Err(Failure::Unsupported)
    }
}
