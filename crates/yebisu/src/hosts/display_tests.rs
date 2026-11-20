use super::*;

#[test]
fn display_name_is_the_first_label_of_the_destination_host() {
    // AddHost.dc.html:61 enters `shin1ohno@pro-dev.home.local`; Main.dc.html:41
    // and :182 show `pro-dev`.
    let name = HostName::new("shin1ohno@pro-dev.home.local");
    assert_eq!(name.display(), "pro-dev");
    assert_eq!(name.destination(), "shin1ohno@pro-dev.home.local");
    assert_eq!(name.to_string(), "pro-dev");
    assert_eq!(HostName::new("me@mini").display(), "mini");
    assert_eq!(HostName::new("me@Mini.Local.:2222").display(), "mini");
}

#[test]
fn display_name_keeps_an_ip_address_whole() {
    assert_eq!(HostName::new("me@192.168.1.20").display(), "192.168.1.20");
    assert_eq!(HostName::new("me@[fe80::1]:2222").display(), "fe80::1");
}

#[test]
fn destination_stays_the_identity() {
    let a = HostName::new("me@pro-dev.home.local");
    let b = HostName::new("root@pro-dev:2222");
    assert_eq!(a.display(), b.display());
    assert_ne!(a, b);
    assert_eq!(b.destination(), "root@pro-dev:2222");
}

#[test]
fn a_string_that_is_not_a_target_shows_as_written() {
    for raw in ["qa-alpha", "", "ssh://me@host"] {
        let name = HostName::new(raw);
        assert_eq!(name.display(), raw);
        assert_eq!(name.destination(), raw);
    }
}
