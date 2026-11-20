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
