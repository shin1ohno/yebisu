#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Herdr 2: {0}")]
    Core(#[from] sheprd_core_client::Error),
    #[error("terminal backend: {0}")]
    Backend(#[from] sheprd_backend::Error),
    #[error("Herdr 2 input queue is full or disconnected")]
    CoreQueue,
    #[error("invalid saved Herdr 2 workspace")]
    CoreState,
    #[error("Herdr 2 program ended; reattach or open a new terminal")]
    CoreEnded,
    #[error("app log HOME is unavailable")]
    LogHome,
    #[error("app log subscriber is already installed")]
    LogSubscriber,
    #[error("terminal image exceeds limits")]
    PaneImageLimit,
    #[error("terminal image decoding failed: {0}")]
    PaneImageDecode(image::ImageError),
    #[error("invalid authorized_keys line or unexpected key comment")]
    InvalidKeyLine,
    #[error("host is untrusted or its key has changed; authentication refused")]
    UntrustedHost,
    #[error("could not save or read the connection: {0}")]
    Storage(#[from] std::io::Error),
    #[error("SSH operation failed: {0}")]
    Ssh(String),
    #[error("could not open the GPUI window: {0}")]
    Window(String),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
