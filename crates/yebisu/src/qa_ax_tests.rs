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
    let fields = view.update(cx, |this, _| {
        this.smoke_mode = true;
        this.qa_ax_fields()
    });
    let names: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
    assert_eq!(names, ["screen", "sheet", "menu", "taps"]);
    assert_eq!(fields[0].1, "p0");
    assert!(fields[1].1.is_null() && fields[2].1.is_null());
    assert_eq!(fields[3].1, 0);
    for (key, _) in &fields {
        assert!(!layer::RESERVED.contains(key), "{key} is the layer's own");
    }
    let fields = view.update(cx, |this, _| {
        this.settings_modal = true;
        this.shell.frame.overflow_open = true;
        this.qa_ax_fields()
    });
    assert_eq!(fields[1].1, "settings");
    assert_eq!(fields[2].1, "overflow");
}
