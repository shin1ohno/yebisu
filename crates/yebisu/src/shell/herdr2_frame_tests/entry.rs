use super::*;
use crate::shell::sidebar::model::{HostEntry, agents_list, order_hosts};

#[test]
fn new_space_offers_only_herdr1_until_sessions_task() {
    let mixed = new_space_choices(&[HerdrKind::Herdr2, HerdrKind::Herdr1]);
    assert_eq!(
        mixed,
        [
            NewSpaceChoice {
                kind: HerdrKind::Herdr1,
                enabled: true,
                reason: None,
            },
            NewSpaceChoice {
                kind: HerdrKind::Herdr2,
                enabled: false,
                reason: Some(HERDR2_NEW_SPACE),
            },
        ]
    );
    // A Herdr 2 only host offers New space disabled, with the reason.
    let only = new_space_choices(&[HerdrKind::Herdr2]);
    assert!(only.iter().all(|c| !c.enabled && c.reason.is_some()));
}

#[test]
fn mixed_host_lists_herdr1_first() {
    let state = SidebarState::default();
    let two = connected_host("pro-dev", &[raw("t1", Some("working"))], &state);
    let one = HostEntry::new("pro-dev", HerdrKind::Herdr1, HostLink::Connected);
    let mut hosts = vec![two, one];
    order_hosts(&mut hosts, &["pro-dev".to_owned()]);
    assert_eq!(
        hosts.iter().map(|h| h.kind).collect::<Vec<_>>(),
        [HerdrKind::Herdr1, HerdrKind::Herdr2]
    );
    // Herdr 1's agent leads a first batch of equal statuses on that host.
    let mut herdr1 = connected_host("pro-dev", &[raw("a", Some("working"))], &state);
    herdr1.kind = HerdrKind::Herdr1;
    for line in &mut herdr1.agents {
        line.row.identity.kind = HerdrKind::Herdr1;
    }
    let mut hosts = vec![hosts.remove(1), herdr1];
    order_hosts(&mut hosts, &["pro-dev".to_owned()]);
    let kinds = agents_list(&hosts)
        .iter()
        .map(|l| l.row.identity.kind)
        .collect::<Vec<_>>();
    assert_eq!(kinds, [HerdrKind::Herdr1, HerdrKind::Herdr2]);
}
