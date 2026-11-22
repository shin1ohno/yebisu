use super::*;

#[test]
fn reply_omits_default_port() {
    assert_eq!(
        resolved_device(c"Studio.local.", 22_u16.to_be(), c"Studio Mac"),
        Some(Device {
            address: "Studio.local".into(),
            name: "Studio Mac".into(),
        })
    );
}

#[test]
fn reply_strips_control_characters_from_the_name() {
    assert_eq!(
        resolved_device(c"studio.local.", 2222_u16.to_be(), c"Studio\n Mac\t\x7f")
            .map(|device| device.name)
            .as_deref(),
        Some("Studio Mac")
    );
}

#[test]
fn reply_preserves_other_ports_and_decodes_names_lossily() {
    assert_eq!(
        resolved_device(c"studio.local.", 2222_u16.to_be(), c"Studio \xff"),
        Some(Device {
            address: "studio.local:2222".into(),
            name: "Studio �".into(),
        })
    );
}

#[test]
fn reply_rejects_invalid_ssh_targets() {
    for (host, port) in [
        (c".", 22_u16),
        (c"bad host.local.", 22),
        (c"alice@studio.local.", 22),
        (c"studio.local.", 0),
        (c"st\xffdio.local.", 22),
    ] {
        assert_eq!(
            resolved_device(host, port.to_be(), c"Studio"),
            None,
            "{host:?}"
        );
    }
}
