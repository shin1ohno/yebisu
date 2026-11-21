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
        assert_eq!(status.dot_color(), canvas, "{status:?}");
    }
    let mut covered = legend.map(|(status, _)| status).to_vec();
    covered.sort();
    assert_eq!(covered, SidebarStatus::ALL);
    // The pre-Wave 0a swap: Working was the accent and Idle grey.
    assert_ne!(
        SidebarStatus::Working.dot_color(),
        SidebarStatus::Idle.dot_color()
    );
}

#[test]
fn agents_status_word_is_bold_yellow_needs_you_or_muted() {
    use SidebarStatus::*;
    for state in SidebarStatus::ALL {
        let (ink, font) = match state {
            NeedsYou => (0xebcb8b, weight::BOLD),
            Error => (0xbf616a, weight::BOLD),
            _ => (0xacb2bb, weight::REGULAR),
        };
        assert_eq!(state.label_color(), ink, "{state:?}");
        assert_eq!(state.label_weight(), font, "{state:?}");
    }
}

#[test]
fn agent_status_words_meet_sidebar_contrast_except_canvas_error() {
    use crate::shell::style::color::SURFACE;

    for status in SidebarStatus::ALL
        .into_iter()
        .filter(|status| status.is_agent())
    {
        let ratio = contrast(status.label_color(), SURFACE);
        if status == SidebarStatus::Error {
            // ModsMax.dc.html:85 specifies #bf616a on #1c1c22 (4.14:1).
            // Keep this single canvas exception visible when the palette changes.
            assert_eq!((ratio * 100.).round(), 414.);
        } else {
            assert!(ratio >= 4.5, "{status:?}: {ratio:.2}:1");
        }
    }
    assert!(contrast(SidebarStatus::Unknown.dot_color(), SURFACE) < 4.5);
}

fn contrast(a: u32, b: u32) -> f64 {
    fn luminance(rgb: u32) -> f64 {
        let channel = |shift: u32| {
            let value = f64::from((rgb >> shift) & 0xff) / 255.;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
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
