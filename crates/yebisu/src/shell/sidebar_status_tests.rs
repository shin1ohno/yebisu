use super::*;

#[test]
fn status_dots_follow_the_sidebar_spec_legend() {
    // SidebarSpec.dc.html:38: `<span style="color: #bf616a">●</span> Needs you`
    // ... `#ebcb8b` Working, `#a3be8c` Done, `#88c0d0` Idle, `#bf616a` Error,
    // `#4c566a` エージェントなし・状態なし. Unknown is not in the legend and
    // takes the no-status slate (Wave 0a brief).
    let legend = [
        (SidebarStatus::NeedsYou, 0xbf616a),
        (SidebarStatus::Error, 0xbf616a),
        (SidebarStatus::Working, 0xebcb8b),
        (SidebarStatus::Done, 0xa3be8c),
        (SidebarStatus::Idle, 0x88c0d0),
        (SidebarStatus::Unknown, 0x4c566a),
        (SidebarStatus::NoStatus, 0x4c566a),
    ];
    for (status, canvas) in legend {
        assert_eq!(status.color(), canvas, "{status:?}");
    }
    let mut covered = legend.map(|(status, _)| status).to_vec();
    covered.sort();
    assert_eq!(covered, SidebarStatus::ALL);
    // The pre-Wave 0a swap: Working was the accent and Idle grey.
    assert_ne!(SidebarStatus::Working.color(), SidebarStatus::Idle.color());
}

#[test]
fn agents_order_needs_you_error_pinned_then_working_done_idle() {
    use SidebarStatus::*;
    let mut rows = [
        (Idle, false),
        (Done, true),
        (Working, false),
        (Unknown, false),
        (Error, false),
        (Done, false),
        (Idle, true),
        (NeedsYou, false),
    ];
    rows.sort_by_key(|&(status, pinned)| AgentGroup::of(status, pinned));
    assert_eq!(
        rows.map(|(status, pinned)| AgentGroup::of(status, pinned)),
        [
            AgentGroup::NeedsYou,
            AgentGroup::Error,
            AgentGroup::Pinned,
            AgentGroup::Pinned,
            AgentGroup::Working,
            AgentGroup::Done,
            AgentGroup::Idle,
            AgentGroup::Unknown,
        ]
    );
}

#[test]
fn a_pin_never_moves_an_agent_that_waits_or_failed() {
    assert_eq!(
        AgentGroup::of(SidebarStatus::NeedsYou, true),
        AgentGroup::NeedsYou
    );
    assert_eq!(
        AgentGroup::of(SidebarStatus::Error, true),
        AgentGroup::Error
    );
    // Every calm state rises to Pinned, just under Error.
    for status in [
        SidebarStatus::Working,
        SidebarStatus::Done,
        SidebarStatus::Idle,
        SidebarStatus::Unknown,
    ] {
        let pinned = AgentGroup::of(status, true);
        assert_eq!(pinned, AgentGroup::Pinned, "{status:?}");
        assert!(pinned > AgentGroup::Error && pinned < AgentGroup::Working);
    }
    // A terminal without a status is no agent, pinned or not (Q4).
    assert_eq!(
        AgentGroup::of(SidebarStatus::NoStatus, true),
        AgentGroup::NoStatus
    );
}

#[test]
fn without_pins_the_group_keeps_the_status_rank() {
    for a in SidebarStatus::ALL {
        for b in SidebarStatus::ALL {
            assert_eq!(
                AgentGroup::of(a, false).cmp(&AgentGroup::of(b, false)),
                a.rank().cmp(&b.rank()),
                "{a:?} {b:?}"
            );
        }
    }
}
