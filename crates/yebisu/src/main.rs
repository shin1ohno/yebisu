mod error;
#[cfg(any(target_os = "ios", test))]
mod hosts;
#[cfg(any(target_os = "ios", test))]
mod keys;
#[cfg(any(target_os = "ios", test))]
mod setup;
pub use error::{Error, Result};
#[cfg(target_os = "ios")]
mod app;
#[cfg(target_os = "ios")]
mod app_setup;
#[cfg(any(target_os = "ios", test))]
mod input;

fn main() {
    #[cfg(target_os = "ios")]
    app::run();
}

#[cfg(target_os = "ios")]
mod observation;
