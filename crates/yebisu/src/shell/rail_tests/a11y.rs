use super::{host, space};
use crate::shell::{
    rail::{RailItemKind, RailTarget, rail_items},
    sidebar_status::SidebarStatus::{Error, Idle, NeedsYou, Working},
};

#[test]
fn rail_full_name_in_accessibility_label() {
    let long = "session-search-with-a-name-far-too-long-for-64-points";
    let hosts = vec![host(
        "pro-dev",
        true,
        vec![space(
            "w1",
            long,
            NeedsYou,
            &[NeedsYou, NeedsYou, Error, Idle],
        )],
    )];
    let items = rail_items(&hosts);
    let label = items[1].accessibility_label();
    assert!(label.starts_with(long), "{label}");
    assert!(label.contains("pro-dev"), "{label}");
    assert!(label.contains("Needs you"), "{label}");
    assert!(label.contains("2 agents waiting"), "{label}");
    assert!(label.contains("1 agent in error"), "{label}");
    // A host row says connected and which Herdr, not a space status.
    let host_label = items[0].accessibility_label();
    assert_eq!(host_label, "pro-dev, connected, Herdr 1");
    let off = rail_items(&[host("mini", false, vec![])]);
    assert_eq!(off[0].accessibility_label(), "mini, not connected, Herdr 1");
}

#[test]
fn rows_list_hosts_then_their_spaces() {
    let mut current = host(
        "pro-dev",
        true,
        vec![
            space("w1", "yebisu", Working, &[Working]),
            space("w2", "", Idle, &[]),
        ],
    );
    current.current = true;
    current.spaces[0].focused = true;
    let offline = host("mini", false, vec![space("w9", "stale", Idle, &[Idle])]);
    let items = rail_items(&[current, offline]);
    let names: Vec<_> = items.iter().map(|i| (i.kind, i.name.as_str())).collect();
    assert_eq!(
        names,
        [
            (RailItemKind::Host, "pro-dev"),
            (RailItemKind::Space, "yebisu"),
            (RailItemKind::Space, "Space 1"),
            (RailItemKind::Host, "mini"),
        ],
        "an offline host keeps no spaces and an unnamed space gets its number"
    );
    assert!(items[0].current && items[1].current && !items[2].current);
    assert!(items[1].accessibility_label().ends_with(", current"));
    assert_eq!(items[0].target, RailTarget::Host("pro-dev".into()));
    assert_eq!(
        items[1].target,
        RailTarget::Space {
            host: "pro-dev".into(),
            workspace_id: "w1".into(),
            boot: "boot-1".into(),
            generation: 3,
        }
    );
    assert_eq!((items[1].needs, items[1].error), (0, 0));
}
