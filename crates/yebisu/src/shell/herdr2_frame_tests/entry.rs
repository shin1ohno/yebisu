use super::*;
use crate::shell::sidebar::model::{HostEntry, agents_list, order_hosts};

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
