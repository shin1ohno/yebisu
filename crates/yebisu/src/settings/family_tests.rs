//! Deliver real worker Loaded independently from the font enumeration callback.
//! The regression enters through the rendered production family button.
use super::{
    SettingsHost, Values, font::TerminalFont, store, tests::directory, worker::Completion,
};
use crate::app::Smoke;
use gpui::{AppContext as _, AppLifecyclePhase, TestAppContext, px, size};
use std::{fs, sync::mpsc, time::Duration};

struct HeldLoad {
    completion: Completion,
    worker: mpsc::Receiver<Completion>,
    deliver: mpsc::SyncSender<Completion>,
}
impl HeldLoad {
    fn take(host: &mut SettingsHost) -> Self {
        // Capture the actual disk worker result, not a fabricated Values/Loaded.
        let completion = host
            .receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|e| panic!("disk load: {e}"));
        assert!(matches!(&completion, Completion::Loaded(Ok(_))));
        let (deliver, pending) = mpsc::sync_channel(2);
        let worker = std::mem::replace(&mut host.receiver, pending);
        Self {
            completion,
            worker,
            deliver,
        }
    }
    fn release(self, view: &gpui::Entity<Smoke>, cx: &mut gpui::VisualTestContext) {
        self.deliver
            .send(self.completion)
            .unwrap_or_else(|e| panic!("{e}"));
        view.update(cx, |this, cx| {
            this.poll_settings(cx);
            let host = this.settings.as_mut().unwrap_or_else(|| panic!("host"));
            assert!(host.loaded);
            // Save requests and their acknowledgments still use the real worker.
            host.receiver = self.worker;
        });
    }
}

fn durable(view: &gpui::Entity<Smoke>, cx: &mut gpui::VisualTestContext) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if view.update(cx, |this, cx| {
            this.poll_settings(cx);
            this.settings.as_ref().is_some_and(SettingsHost::ready)
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "durable family timeout"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn disk_values() -> Values {
    Values {
        compact: true,
        font: TerminalFont {
            size: Some(18.),
            family: "Menlo".into(),
        },
        terminal_priority: true,
        copy_on_select: true,
        drag_select: false,
        ..Values::default()
    }
}

#[gpui::test]
fn default_family_button_before_loaded_survives_background_and_reopen(cx: &mut TestAppContext) {
    for prior_edit in [false, true] {
        let root = directory("family-intent");
        let stored = disk_values();
        store::save(&root, &stored).unwrap_or_else(|e| panic!("{e}"));
        let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
        let held = HeldLoad::take(&mut host);
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        cx.simulate_resize(size(px(1100.), px(900.)));
        view.update(cx, |this, _| {
            this.settings = Some(host);
            // Enumeration is pending independently from the already-read disk.
            this.settings_fonts_loading = true;
        });
        cx.update(|window, cx| view.update(cx, |this, cx| this.begin_settings_modal(window, cx)));
        view.update(cx, |this, cx| {
            if prior_edit {
                let mut values = this.settings_values();
                values.board_hidden = true;
                this.apply_settings(values, cx);
            }
            // Deliver exactly the completion used by the production async task.
            this.complete_settings_font_enumeration(
                vec!["SauceCodePro Nerd Font Mono".into(), "Menlo".into()],
                cx,
            );
            this.poll_settings(cx);
            assert!(
                !this
                    .settings
                    .as_ref()
                    .unwrap_or_else(|| panic!("host"))
                    .loaded
            );
            assert_eq!(this.terminal.font.family, "SauceCodePro Nerd Font Mono");
        });
        cx.run_until_parked();
        let family = cx
            .debug_bounds("font-family-0")
            .unwrap_or_else(|| panic!("family button"));
        cx.simulate_click(family.center(), Default::default());
        view.update(cx, |this, cx| {
            let host = this.settings.as_ref().unwrap_or_else(|| panic!("host"));
            assert!(!host.loaded, "real listener must run before Loaded");
            assert!(!host.ready(), "pending intent cannot claim Saved");
            this.select_host("family-other-host");
            this.lifecycle(AppLifecyclePhase::Background, cx);
            this.poll_settings(cx);
            assert_eq!(this.terminal.font.family, "SauceCodePro Nerd Font Mono");
        });
        assert_eq!(store::load(&root).unwrap_or_else(|e| panic!("{e}")), stored);
        held.release(&view, cx);
        durable(&view, cx);
        let mut expected = stored;
        expected.font.family = "SauceCodePro Nerd Font Mono".into();
        expected.board_hidden = prior_edit;
        assert_eq!(
            store::load(&root).unwrap_or_else(|e| panic!("{e}")),
            expected
        );
        let json: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(json["terminal_font_family"], "SauceCodePro Nerd Font Mono");
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                assert_eq!(this.settings_values(), expected);
                this.end_settings_modal(window, cx);
                this.begin_settings_modal(window, cx);
                this.select_host("family-returned-host");
                this.poll_settings(cx);
                assert_eq!(this.settings_values(), expected);
            })
        });
        let reopened = cx.new(Smoke::new);
        reopened.update(cx, |this, _| {
            this.settings =
                Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
        });
        durable(&reopened, cx);
        reopened.update(cx, |this, cx| {
            this.select_host("family-relaunched-host");
            this.poll_settings(cx);
            assert_eq!(this.settings_values(), expected);
        });
        fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[gpui::test]
fn enumeration_and_poll_without_family_click_preserve_disk_family(cx: &mut TestAppContext) {
    for enumeration_first in [false, true] {
        for prior_edit in [false, true] {
            let root = directory("family-no-intent");
            let stored = disk_values();
            store::save(&root, &stored).unwrap_or_else(|e| panic!("{e}"));
            let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
            let held = HeldLoad::take(&mut host);
            let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
            view.update(cx, |this, _| {
                this.settings = Some(host);
                this.settings_fonts_loading = true;
            });
            cx.update(|window, cx| {
                view.update(cx, |this, cx| this.begin_settings_modal(window, cx))
            });
            view.update(cx, |this, cx| {
                if prior_edit {
                    let mut values = this.settings_values();
                    values.board_hidden = true;
                    this.apply_settings(values, cx);
                }
                if enumeration_first {
                    this.complete_settings_font_enumeration(
                        vec!["SauceCodePro Nerd Font Mono".into()],
                        cx,
                    );
                }
                this.poll_settings(cx);
                this.select_host("enumeration-other-host");
                this.lifecycle(AppLifecyclePhase::Background, cx);
            });
            held.release(&view, cx);
            view.update(cx, |this, cx| {
                if !enumeration_first {
                    this.complete_settings_font_enumeration(
                        vec!["SauceCodePro Nerd Font Mono".into()],
                        cx,
                    );
                }
                this.poll_settings(cx);
            });
            durable(&view, cx);
            let mut expected = stored;
            expected.board_hidden = prior_edit;
            assert_eq!(
                view.read_with(cx, |this, _| this.settings_values()),
                expected
            );
            assert_eq!(
                store::load(&root).unwrap_or_else(|e| panic!("{e}")),
                expected
            );
            fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
        }
    }
}
