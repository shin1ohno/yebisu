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
// Match the pinned SDK's fixed, credential-free Display categories. Never log errors.
// The strings are pinned by a non-iOS test, so an SDK wording change fails CI instead of
// silently turning Locked and Missing into Provider.
#[cfg(any(test, target_os = "ios"))]
pub(super) const LOCKED_MESSAGE: &str = "The iPad is locked. Unlock it and try again.";
#[cfg(any(test, target_os = "ios"))]
pub(super) const MISSING_MESSAGE: &str =
    "The device key was not in the Keychain after it was created. Try again.";
#[cfg(any(test, target_os = "ios"))]
pub(super) fn classify_message(message: &str) -> Failure {
    match message {
        LOCKED_MESSAGE => Failure::Locked,
        MISSING_MESSAGE => Failure::Missing,
        _ => Failure::Provider,
    }
}
#[cfg(all(target_os = "ios", not(test)))]
fn classify(error: sheprd_client::Error) -> Failure {
    classify_message(&error.to_string())
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
