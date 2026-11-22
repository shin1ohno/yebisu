use super::*;
use crate::hosts::test_support::{MINI, STUDIO, named_setup};
use crate::shell::sidebar::model::{OfflineReason, SpaceInput, space_entry};

fn space(kind: HerdrKind, name: &str) -> SpaceEntry {
    space_entry(
        kind,
        SpaceInput {
            id: name.to_owned(),
            name: name.to_owned(),
            tabs: 1,
            branch: None,
            current: false,
        },
        &[],
    )
}

fn host(name: &str, kind: HerdrKind, link: HostLink, spaces: &[&str]) -> HostEntry {
    HostEntry::new(name, kind, link)
        .with_content(spaces.iter().map(|s| space(kind, s)).collect(), Vec::new())
}

/// What a row draws, for comparing whole lists.
fn shape(rows: &[ListRow]) -> Vec<String> {
    rows.iter()
        .map(|row| match row {
            ListRow::Host(host) => format!(
                "host {} {:?}",
                host.name.display(),
                host.connections.iter().map(|c| c.kind).collect::<Vec<_>>()
            ),
            ListRow::Space(space) => {
                format!("  {:?} {} on {}", space.kind, space.entry.name, space.host)
            }
        })
        .collect()
}

#[test]
fn host_rows_show_saved_name_and_keep_the_ssh_destination() {
    let hosts = [
        host(STUDIO, HerdrKind::Herdr1, HostLink::Connected, &[]),
        host(STUDIO, HerdrKind::Herdr2, HostLink::Connected, &[]),
        host(MINI, HerdrKind::Herdr1, HostLink::Connected, &[]),
    ];
    let rows = merge(&hosts, &named_setup());
    assert_eq!(rows.len(), 2);
    for (row, (label, destination)) in rows.iter().zip([("Studio Mac", STUDIO), ("mini", MINI)]) {
        let ListRow::Host(host) = row else {
            panic!("expected a merged host row");
        };
        assert_eq!(host.name.display(), label);
        assert_eq!(host.name.destination(), destination);
    }
}

#[test]
fn one_host_row_with_herdr1_spaces_then_herdr2_sessions() {
    // SidebarMixed.dc.html:28-53: pro-dev answers on both Herdrs; its Herdr 1
    // spaces (yebisu, session-search) come before its Herdr 2 sessions
    // (setup, scratch); mini answers on neither.
    use HerdrKind::*;
    let hosts = [
        host(
            "me@pro-dev.home.local",
            Herdr1,
            HostLink::Connected,
            &["yebisu", "session-search"],
        ),
        host(
            "me@pro-dev.home.local",
            Herdr2,
            HostLink::Connected,
            &["setup", "scratch"],
        ),
        host(
            "me@mini",
            Herdr1,
            HostLink::Offline(OfflineReason::NotConnected),
            &["gone"],
        ),
    ];
    let rows = merge(&hosts, &Setup::default());
    assert_eq!(
        shape(&rows),
        [
            "host pro-dev [Herdr1, Herdr2]",
            "  Herdr1 yebisu on me@pro-dev.home.local",
            "  Herdr1 session-search on me@pro-dev.home.local",
            "  Herdr2 setup on me@pro-dev.home.local",
            "  Herdr2 scratch on me@pro-dev.home.local",
            "host mini [Herdr1]",
        ]
    );
    let ListRow::Host(mini) = &rows[5] else {
        panic!("mini is not a host row: {:?}", rows[5]);
    };
    assert_eq!(
        mini.connections,
        [HostConnection {
            kind: Herdr1,
            link: HostLink::Offline(OfflineReason::NotConnected),
        }]
    );
}

#[test]
fn a_host_stands_where_its_first_entry_stands_herdr1_first() {
    use HerdrKind::*;
    let hosts = [
        host("me@pro-dev", Herdr2, HostLink::Connecting, &[]),
        host("me@mini", Herdr2, HostLink::Connected, &["build"]),
        host("me@pro-dev", Herdr1, HostLink::Connected, &["yebisu"]),
    ];
    let rows = merge(&hosts, &Setup::default());
    assert_eq!(
        shape(&rows),
        [
            "host pro-dev [Herdr1, Herdr2]",
            "  Herdr1 yebisu on me@pro-dev",
            "host mini [Herdr2]",
            "  Herdr2 build on me@mini",
        ]
    );
    let ListRow::Host(pro_dev) = &rows[0] else {
        panic!("pro-dev is not a host row: {:?}", rows[0]);
    };
    assert_eq!(pro_dev.connections[1].link, HostLink::Connecting);
    assert_eq!(pro_dev.name.destination(), "me@pro-dev");
}

#[test]
fn the_host_row_is_selected_when_either_connection_is() {
    use HerdrKind::*;
    let mut herdr2 = host("me@pro-dev", Herdr2, HostLink::Connected, &[]);
    herdr2.selected = true;
    let rows = merge(
        &[host("me@pro-dev", Herdr1, HostLink::Connected, &[]), herdr2],
        &Setup::default(),
    );
    assert!(matches!(&rows[..], [ListRow::Host(h)] if h.selected));
    assert!(merge(&[], &Setup::default()).is_empty());
}

#[test]
fn a_herdr2_destination_from_both_sources_keeps_its_sessions() {
    use HerdrKind::*;
    // `host_entries` lists every reader; its Herdr 2 entries carry no
    // sessions. `herdr2_hosts` lists the same Herdr 2 destination with them.
    let host_entries = vec![
        host("me@mini", Herdr1, HostLink::Connected, &["yebisu"]),
        host("me@pro-dev", Herdr2, HostLink::Connected, &[]),
        host("me@quiet", Herdr2, HostLink::Connecting, &[]),
        host("me@nas", Herdr1, HostLink::Connected, &["backup"]),
    ];
    let herdr2_hosts = vec![host("me@pro-dev", Herdr2, HostLink::Connected, &["build"])];

    // Concatenated, the sessionless entry comes first and `merge` keeps it:
    // the session row is lost. This is the input `sources` exists to avoid.
    let naive = [host_entries.clone(), herdr2_hosts.clone()].concat();
    assert!(
        !shape(&merge(&naive, &Setup::default()))
            .contains(&"  Herdr2 build on me@pro-dev".to_owned())
    );

    let rows = merge(&sources(host_entries, herdr2_hosts), &Setup::default());
    assert_eq!(
        shape(&rows),
        [
            "host mini [Herdr1]",
            "  Herdr1 yebisu on me@mini",
            "host pro-dev [Herdr2]",
            "  Herdr2 build on me@pro-dev",
            "host quiet [Herdr2]",
            "host nas [Herdr1]",
            "  Herdr1 backup on me@nas",
        ]
    );
}

#[test]
fn sources_keep_herdr1_entries_and_append_unmatched_herdr2_hosts() {
    use HerdrKind::*;
    let host_entries = vec![host("me@mini", Herdr1, HostLink::Connected, &["yebisu"])];
    // A Herdr 1 entry in `herdr2_hosts` is ignored: Herdr 1 has one source.
    let herdr2_hosts = vec![
        host("me@mini", Herdr1, HostLink::Connected, &["other"]),
        host("me@pro-dev", Herdr2, HostLink::Connected, &["build"]),
    ];
    let rows = merge(&sources(host_entries, herdr2_hosts), &Setup::default());
    assert_eq!(
        shape(&rows),
        [
            "host mini [Herdr1]",
            "  Herdr1 yebisu on me@mini",
            "host pro-dev [Herdr2]",
            "  Herdr2 build on me@pro-dev",
        ]
    );
}
