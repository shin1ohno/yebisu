mod error;
pub use error::{Error, Result};
#[cfg(target_os = "ios")]
mod app;
#[cfg(any(target_os = "ios", test))]
mod input;

fn main() {
    #[cfg(target_os = "ios")]
    app::run();
}
