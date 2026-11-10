#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not open the GPUI window: {0}")]
    Window(String),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
