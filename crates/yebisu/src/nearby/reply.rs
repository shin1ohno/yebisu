use super::Device;
use std::ffi::CStr;

pub(super) fn resolved_device(host: &CStr, network_port: u16, name: &CStr) -> Option<Device> {
    let domain = host.to_str().ok()?.trim_end_matches('.');
    let port = u16::from_be(network_port);
    let address = if port == 22 {
        domain.to_owned()
    } else {
        format!("{domain}:{port}")
    };
    // SSH targets require a user; Bonjour only supplies the host and port.
    herdr_ssh::Target::parse(&format!("user@{address}")).ok()?;
    let name = name
        .to_string_lossy()
        .chars()
        .filter(|ch| !ch.is_control())
        .collect();
    Some(Device { address, name })
}

#[cfg(test)]
#[path = "reply_tests.rs"]
mod tests;
