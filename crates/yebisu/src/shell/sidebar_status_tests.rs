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
fn status_words_keep_their_colours_while_the_dots_change() {
    // Wave 0a recolours dots only. The AGENTS status word keeps the colour it
    // had before (Herdr 1 `sidebar/view.rs` and Herdr 2
    // `herdr2_frame/sidebar.rs` both draw `label_color`) until stream A
    // restyles it per Main.dc.html:94, :101, :108.
    let words = [
        (SidebarStatus::NeedsYou, 0xbf616a),
        (SidebarStatus::Error, 0xbf616a),
        (SidebarStatus::Working, 0x88c0d0),
        (SidebarStatus::Done, 0xa3be8c),
        (SidebarStatus::Idle, 0x9ca3af),
        (SidebarStatus::Unknown, 0xb48ead),
        (SidebarStatus::NoStatus, 0x6b7280),
    ];
    for (status, before) in words {
        assert_eq!(status.label_color(), before, "{status:?}");
    }
    let mut covered = words.map(|(status, _)| status).to_vec();
    covered.sort();
    assert_eq!(covered, SidebarStatus::ALL);
    // The two palettes differ where the review saw the dot colour leak into
    // the word: Unknown's slate dot, Working's yellow dot, Idle's accent dot.
    for status in [
        SidebarStatus::Unknown,
        SidebarStatus::Working,
        SidebarStatus::Idle,
    ] {
        assert_ne!(status.label_color(), status.dot_color(), "{status:?}");
    }
}

#[test]
fn no_agent_status_word_loses_contrast_on_the_sidebar() {
    // The sidebar background (`shell/frame.rs` `.bg(rgb(0x16161b))`).
    const SIDEBAR: u32 = 0x16161b;
    // Contrast before Wave 0a, per word, in hundredths (WCAG 2 ratio).
    let before = [
        (SidebarStatus::NeedsYou, 441),
        (SidebarStatus::Error, 441),
        (SidebarStatus::Working, 901),
        (SidebarStatus::Done, 884),
        (SidebarStatus::Idle, 710),
        (SidebarStatus::Unknown, 636),
    ];
    for (status, hundredths) in before {
        assert!(status.is_agent());
        let ratio = contrast(status.label_color(), SIDEBAR);
        assert!(
            (ratio * 100.0).round() >= f64::from(hundredths),
            "{status:?}: {ratio:.2}:1"
        );
        // The calm words already pass 4.5:1; the red Needs you / Error word
        // (4.41:1) is unchanged here and becomes the canvas's #ebcb8b /
        // #acb2bb in stream A.
        if !matches!(status, SidebarStatus::NeedsYou | SidebarStatus::Error) {
            assert!(ratio >= 4.5, "{status:?}: {ratio:.2}:1");
        }
    }
    // The slate dot colour as a word would fail: the regression this guards.
    assert!(contrast(SidebarStatus::Unknown.dot_color(), SIDEBAR) < 4.5);
    // Stream A's canvas words pass on the same background.
    for canvas in [0xebcb8b, 0xacb2bb] {
        assert!(contrast(canvas, SIDEBAR) >= 4.5, "{canvas:06x}");
    }
}

/// WCAG 2 contrast ratio of two `0xRRGGBB` colours.
fn contrast(a: u32, b: u32) -> f64 {
    fn luminance(rgb: u32) -> f64 {
        let channel = |shift: u32| {
            let c = f64::from((rgb >> shift) & 0xff) / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
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
