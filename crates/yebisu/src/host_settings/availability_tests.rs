use super::*;

#[test]
fn disconnect_and_connected_provider_never_advertise_read_support() {
    assert_eq!(current(false), Availability::Disconnected);
    assert_eq!(current(true), Availability::CredentialUnavailable);
    assert!(current(true).message().starts_with("Unavailable"));
    assert_eq!(current(false), Availability::Disconnected);
}
