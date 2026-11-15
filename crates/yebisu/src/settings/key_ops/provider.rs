use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    // Native device production always supports this provider; other targets may not.
    #[cfg_attr(
        all(target_os = "ios", not(target_abi = "sim"), not(test)),
        allow(dead_code)
    )]
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
    // Capability provenance comes from the selected object, never environment flags.
    fn injected(&self) -> bool {
        false
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
// Classify by the SDK's typed device-key variants, never by display text. A renamed or
// removed `Locked` or `Missing` variant is then a compile error in the iOS build
// (`just lint-ios`) instead of a silent fall-through to `Provider`. The chain is
// `Error::Ssh(InProcess(DeviceKey(Credentials::DeviceKey(device_key::Error))))`; the
// device-key module exists only for iOS, so no host test can reach it.
#[cfg(all(target_os = "ios", not(test)))]
fn classify(error: sheprd_client::Error) -> Failure {
    use sheprd_client::{
        Error as Client,
        ssh::{Error as Ssh, in_process::Error as InProcess},
    };
    use sheprd_credentials::{Error as Credentials, device_key::Error as Key};
    match error {
        Client::Ssh(Ssh::InProcess(InProcess::DeviceKey(Credentials::DeviceKey(Key::Locked)))) => {
            Failure::Locked
        }
        Client::Ssh(Ssh::InProcess(InProcess::DeviceKey(Credentials::DeviceKey(Key::Missing)))) => {
            Failure::Missing
        }
        _ => Failure::Provider,
    }
}

#[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
struct Injected;
#[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
impl Provider for Injected {
    fn injected(&self) -> bool {
        true
    }
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
