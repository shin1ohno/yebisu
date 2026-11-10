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

#[cfg(any(target_os = "ios", test))]
mod link;
#[cfg(any(target_os = "ios", test))]
mod live;
#[cfg(any(target_os = "ios", test))]
#[allow(dead_code, unused_imports)]
mod upstream;
#[cfg(any(target_os = "ios", test))]
use upstream::{font_weight, osc52, terminal, terminal_painter};
#[cfg(any(target_os = "ios", test))]
mod theme;

#[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
mod mock_peer;
#[cfg(target_os = "ios")]
mod terminal_view;
