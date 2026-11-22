//! The guard: nothing of the QA accessibility layer reaches a device build
//! or a Simulator build started without `YEBISU_QA_AX`. The UIKit half
//! (`qa_ax_native.rs`) is compiled only for the Simulator (qa_ax.rs, checked
//! by scripts/ios/qa-ax-ids-check.py); these tests pin the runtime gate.
use super::*;
use crate::app::Smoke;
use gpui::TestAppContext;

#[test]
fn qa_ax_only_in_simulator_with_env() {
    assert!(!allowed(true, |_| false), "Simulator build, no QA variable");
    assert!(
        !allowed(false, |_| true),
        "device or host build, variable set"
    );
    assert!(!allowed(false, |_| false));
    assert!(allowed(true, |name| name == ENV));
    assert_eq!(ENV, "YEBISU_QA_AX");
    // QAAx.swift names the state element with this literal.
    assert_eq!(ids::STATE, "qa.state");
    // Another QA mode's variable does not turn the layer on.
    for other in crate::shell::qa_strip::QA_ENV {
        assert!(!allowed(true, |name| name == other), "{other}");
    }
    assert!(!allowed(true, |name| name == "YEBISU_SMOKE_OBSERVE"));
}

#[test]
fn qa_ax_host_build_is_not_a_simulator_build() {
    const { assert!(!BUILD, "host tests are not a Simulator build") };
    // Even with every variable set, this build may not build the layer.
    assert!(!allowed(BUILD, |_| true));
    assert!(!enabled());
    assert!(mark(ids::P0_TAP, "Tap / type").is_none());
    assert!(!report("key_ready", || serde_json::json!(true)));
}

#[gpui::test]
fn qa_ax_root_hook_is_off_in_a_non_qa_build(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.run_until_parked();
    assert!(view.update(cx, |this, cx| this.qa_ax_root(cx).is_none()));
    assert!(
        layer::published().is_none(),
        "the app root published nothing"
    );
}

#[gpui::test]
fn qa_ax_root_fields_name_the_screen(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let fields = view.update_in(cx, |this, window, _| {
        this.smoke_mode = true;
        this.qa_ax_fields(window)
    });
    let names: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        names,
        [
            "screen",
            "sheet",
            "menu",
            "taps",
            "settings_page",
            "compact",
            "drag_select",
            "connected",
            "surface",
            "pane",
            "core",
            "input_pane",
            "generation",
            "rows",
            "selecting",
            "copy_on_select",
            "settings_saved",
            "pasting",
            "second",
            "peer",
            "p0_input",
            "font_size",
            "theme",
            "host",
        ]
    );
    assert_eq!(fields[0].1, "p0");
    assert!(fields[1].1.is_null() && fields[2].1.is_null());
    assert_eq!(fields[3].1, 0);
    for (key, _) in &fields {
        assert!(!layer::RESERVED.contains(key), "{key} is the layer's own");
    }
    let fields = view.update_in(cx, |this, window, _| {
        this.settings_modal = true;
        this.shell.frame.overflow_open = true;
        this.qa_ax_fields(window)
    });
    assert_eq!(fields[1].1, "settings");
    assert_eq!(fields[2].1, "overflow");
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone())
    };
    // The open sheet's page (Page::id); the terminal starts disconnected,
    // with no surface, no pane and no core layout.
    assert_eq!(
        field("settings_page"),
        Some(serde_json::json!(crate::settings_pages::Page::Fonts.id()))
    );
    assert_eq!(field("compact"), Some(serde_json::json!(false)));
    // S6: the P0 input text, the font size the terminal draws, the local
    // theme and the selected host (none yet).
    assert_eq!(field("p0_input"), Some(serde_json::json!("")));
    assert!(
        field("font_size")
            .and_then(|size| size.as_f64())
            .is_some_and(|size| size > 0.)
    );
    assert_eq!(field("theme"), Some(serde_json::json!("owner-default")));
    assert_eq!(field("host"), Some(serde_json::json!("")));
    assert_eq!(
        field("drag_select"),
        Some(serde_json::json!(true)),
        "the product default"
    );
    assert_eq!(field("connected"), Some(serde_json::json!(false)));
    for name in ["surface", "pane", "core", "input_pane", "rows"] {
        assert_eq!(field(name), Some(serde_json::Value::Null), "{name}");
    }
    // S4: nothing selected, the product's copy default, the first scene, and
    // a peer that received nothing (the layer is off on the host).
    assert_eq!(field("selecting"), Some(serde_json::json!(false)));
    assert_eq!(field("copy_on_select"), Some(serde_json::json!(false)));
    assert_eq!(field("pasting"), Some(serde_json::json!(false)));
    assert_eq!(field("second"), Some(serde_json::json!(false)));
    assert_eq!(
        field("peer"),
        Some(serde_json::json!({"inputs": 0, "pastes": []}))
    );
    let selecting = view.update_in(cx, |this, window, _| {
        this.handoff.select_mode = true;
        this.qa_ax_fields(window)
    });
    let selecting = selecting.iter().find(|(key, _)| *key == "selecting");
    assert_eq!(
        selecting.map(|(_, value)| value.clone()),
        Some(serde_json::json!(true)),
        "Select text on pauses typing, as the status bar says"
    );
}

#[test]
fn lifecycle_phases_have_their_own_counters() {
    use gpui::AppLifecyclePhase as Phase;
    let keys: Vec<&str> = [
        Phase::Active,
        Phase::Inactive,
        Phase::Background,
        Phase::Foreground,
        Phase::Disconnected,
    ]
    .into_iter()
    .map(lifecycle_key)
    .collect();
    assert_eq!(
        keys,
        [
            "lifecycle_active",
            "lifecycle_inactive",
            "lifecycle_background",
            "lifecycle_foreground",
            "lifecycle_disconnected",
        ]
    );
    for key in keys {
        assert!(!layer::RESERVED.contains(&key), "{key}");
    }
}

#[test]
fn counters_and_frame_requests_are_off_in_a_host_build() {
    count("sh152_start");
    peer::input(["sh1-152-start-E1-text-0"]);
    assert!(
        layer::published().is_none(),
        "a counter outside a QA build publishes nothing"
    );
    assert_eq!(
        peer::field(),
        serde_json::json!({"inputs": 0, "pastes": []}),
        "the peer slot stays empty outside a QA build"
    );
}

#[test]
fn start_selection_comes_from_launch_only_in_a_simulator() {
    assert!(start_drag_select(true, None), "the product default is Drag");
    assert!(!start_drag_select(true, Some("double-tap")));
    assert!(start_drag_select(true, Some("drag")));
    assert!(
        start_drag_select(true, Some("Double tap")),
        "only the exact value"
    );
    assert!(
        start_drag_select(false, Some("double-tap")),
        "a device or host build ignores the variable"
    );
    assert_eq!(START_SELECTION_ENV, "YEBISU_QA_START_SELECTION");
    assert!(launch_drag_select(), "host tests are not a Simulator build");
}

#[gpui::test]
fn qa_ax_window_fields_name_the_keyboard_and_viewport(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let fields = view.update_in(cx, |this, window, _| this.qa_ax_window_fields(window));
    let names: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
    assert_eq!(names, ["keyboard", "viewport", "active"]);
    assert_eq!(fields[0].1, 0.0, "no keyboard in a test window");
    let viewport = fields[1].1.as_array().map(Vec::len);
    assert_eq!(viewport, Some(2), "{:?}", fields[1].1);
}
