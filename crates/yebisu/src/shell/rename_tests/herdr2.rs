//! Herdr 2 (issue #144 D3, AC5 and AC14; #143 AC15): Rename Space and
//! Rename Tab write the `default` session's `herdr-app` document when it is
//! readable; Rename Pane stays disabled with "Not available on Herdr 2 yet".
use super::*;
use crate::{
    actions,
    core::{Session, herdr_doc_sync},
    core_mock::Host,
    shell::{
        ChromeState, HerdrKind, host_menus,
        rename::{
            AUTOMATIC_NAME, HERDR2,
            herdr2::{NO_DOCUMENT, NOT_IN_DOCUMENT},
        },
        sidebar_list::ListRow,
        single_pane::Mailbox,
    },
};
use gpui::TestAppContext;
use sheprd_commands::Command;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const BUILD: &str = "sp00000000000000000000000000000015";
const LOGS: &str = "tb00000000000000000000000000000016";

fn build_row() -> RenameObject {
    space(&format!("default\0{BUILD}"))
}

/// A `default` core on `pro-dev` reading the two-space fixture (#144 D2).
fn documented(cx: &mut TestAppContext) -> (Entity<Smoke>, Host, &mut VisualTestContext, PathBuf) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("yebisu-rename-doc-{}-{n}", std::process::id()));
    let host = Host::default();
    // Every terminal the document names is live: a space whose terminals
    // ended would be taken out of it (`core/ended_spaces.rs`).
    host.set_live(&["t1", "t2", "t30"]);
    host.doc().set_silent(
        serde_json::from_str(include_str!("../../../fixtures/herdr-app/two-spaces.json"))
            .unwrap_or_else(|e| panic!("fixture: {e}")),
    );
    let (backend, _) = host.recorded(false).unwrap_or_else(|e| panic!("mock: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let dir = root.clone();
    view.update(cx, |this, _| {
        this.select_host("pro-dev");
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core_mode = true;
        this.terminal.live = crate::live::Live::default();
        this.terminal.core = Some(
            Session::start_doc(
                "pro-dev".into(),
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                Some(dir),
                Mailbox::detached(),
                herdr_doc_sync::DocPlan {
                    source: herdr_doc_sync::DocSource::Core,
                    period: Duration::from_millis(200),
                },
            )
            .unwrap_or_else(|e| panic!("start: {e}")),
        );
    });
    wait_until(&view, cx, "the document is drawn", |this| {
        this.terminal.live.connected && this.doc_names().is_some_and(|d| d.selected.is_some())
    });
    (view, host, cx, root)
}

fn wait_until(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    description: &str,
    condition: impl Fn(&Smoke) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        condition(this)
    }) {
        assert!(Instant::now() < deadline, "{description}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn finish(view: &Entity<Smoke>, cx: &mut VisualTestContext, root: PathBuf) {
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
    let _ = std::fs::remove_dir_all(root);
}

fn available(this: &Smoke, command: Command) -> bool {
    actions::catalog(&this.terminal)
        .into_iter()
        .any(|action| action.info.command == command && action.available)
}

/// AC14: Rename Pane stays disabled on Herdr 2, with its reason, even
/// with a readable document; nothing is sent.
#[gpui::test]
fn pane_rename_disabled_on_herdr2(cx: &mut TestAppContext) {
    let (view, host, cx, root) = documented(cx);
    view.update(cx, |this, _| {
        assert!(!available(this, Command::RenamePane));
        assert_eq!(
            this.palette_reason(Command::RenamePane, false),
            Some(HERDR2)
        );
        let target = this
            .rename_target(pane("t1"))
            .unwrap_or_else(|| panic!("target"));
        this.begin_rename(target, RenameAnchor::PaneChip);
        assert!(this.shell.rename.session.is_none(), "a pane rename started");
        assert_eq!(this.terminal.live.status, HERDR2);
    });
    assert!(host.doc().sets().is_empty());
    finish(&view, cx, root);
}

/// AC5 (UI): with the document readable, the palette's Rename Space and
/// Rename Tab and the Space menu's Rename are on, for the drawn space and
/// its tabs.
#[gpui::test]
fn space_tab_rename_enabled_when_readable(cx: &mut TestAppContext) {
    let (view, _host, cx, root) = documented(cx);
    view.update(cx, |this, _| {
        assert!(available(this, Command::RenameWorkspace), "Rename Space");
        assert!(available(this, Command::RenameTab), "Rename Tab");
        let state = this.menu_chrome_state();
        assert!(state.herdr2_document);
        let rename = host_menus::menu_bar(&state, true)
            .into_iter()
            .flat_map(|(_, entries)| entries)
            .find(|e| e.item == host_menus::MenuItemId::Space(host_menus::SpaceMenuItem::Rename));
        assert!(rename.is_some_and(|e| e.enabled), "the Space menu's Rename");
        for object in [build_row(), tab(LOGS)] {
            let target = this
                .rename_target(object.clone())
                .unwrap_or_else(|| panic!("target"));
            assert_eq!(this.rename_unavailable(&target), None, "{object:?}");
        }
        // The palette's targets are the drawn space and its active tab.
        let (space_target, _) = this
            .command_rename_target(Command::RenameWorkspace)
            .unwrap_or_else(|| panic!("palette space"));
        assert_eq!(space_target.object, build_row());
        let (tab_target, _) = this
            .command_rename_target(Command::RenameTab)
            .unwrap_or_else(|| panic!("palette tab"));
        assert!(matches!(tab_target.object, RenameObject::Tab(ref id) if id.starts_with("tb")));
    });
    finish(&view, cx, root);
}

/// A committed name is one `doc.set` over the version read, with that
/// space's name only; it shows muted until a later read has it.
#[gpui::test]
fn rename_space_writes_the_document(cx: &mut TestAppContext) {
    let (view, host, cx, root) = documented(cx);
    let fake = install_fake(&view, cx);
    let (_, read_version) = host.doc().doc();
    opened(&view, &fake, cx, build_row(), RenameAnchor::SpaceRow);
    assert_eq!(
        fake.request().text,
        "Build",
        "the field starts with the name"
    );
    assert!(fake.request().allow_empty, "\"\" is the automatic name");
    commit(&fake, cx, "ship");
    view.update(cx, |this, _| {
        assert!(
            this.rename_label(&build_boot(this), &build_row())
                .is_some_and(|(text, _)| text == "ship"),
            "the sent name shows until a read has it"
        );
    });
    wait_until(&view, cx, "the new name is read back", |this| {
        this.doc_names()
            .is_some_and(|doc| doc.name(&build_row()) == "ship")
    });
    let sets = host.doc().sets();
    assert_eq!(sets.len(), 1, "{sets:?}");
    let (value, version) = &sets[0];
    assert_eq!(*version, read_version, "sent over the version read");
    assert_eq!(value["spaces"][0]["name"], "ship");
    assert_eq!(
        value["spaces"][1]["name"], "",
        "the other space is untouched"
    );
    wait_until(&view, cx, "the optimistic name ends", |this| {
        this.shell.rename.pending.is_none()
    });
    finish(&view, cx, root);
}

/// Design E: "" names a tab by its position again (Herdr 1 refuses it).
#[gpui::test]
fn an_empty_name_is_the_automatic_name(cx: &mut TestAppContext) {
    let (view, host, cx, root) = documented(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, tab(LOGS), RenameAnchor::TabLabel);
    assert_eq!(fake.request().text, "logs");
    assert_eq!(fake.request().placeholder, AUTOMATIC_NAME);
    commit(&fake, cx, "");
    wait_until(&view, cx, "the tab is named by its position", |this| {
        this.doc_names()
            .is_some_and(|doc| doc.name(&tab(LOGS)).is_empty())
    });
    let sets = host.doc().sets();
    assert_eq!(sets.len(), 1, "{sets:?}");
    assert_eq!(sets[0].0["spaces"][0]["tabs"][1]["name"], "");
    finish(&view, cx, root);
}

/// The name the document already holds is not written again (as on
/// Herdr 1), and no optimistic name is shown.
#[gpui::test]
fn an_unchanged_name_writes_nothing(cx: &mut TestAppContext) {
    let (view, host, cx, root) = documented(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, build_row(), RenameAnchor::SpaceRow);
    commit(&fake, cx, "Build");
    view.update(cx, |this, _| {
        assert!(this.shell.rename.pending.is_none(), "nothing to show");
    });
    std::thread::sleep(Duration::from_millis(300));
    view.update(cx, |this, cx| this.poll_terminal(cx));
    assert!(host.doc().sets().is_empty(), "{:?}", host.doc().sets());
    finish(&view, cx, root);
}

/// Without a readable document nothing can be renamed on Herdr 2, and
/// what the document does not hold (another session's space) says so.
#[gpui::test]
fn unreadable_or_outside_the_document_say_why(cx: &mut TestAppContext) {
    let state = ChromeState {
        kind: HerdrKind::Herdr2,
        connected: true,
        ..ChromeState::default()
    };
    let rename = host_menus::menu_bar(&state, true)
        .into_iter()
        .flat_map(|(_, entries)| entries)
        .find(|e| e.item == host_menus::MenuItemId::Space(host_menus::SpaceMenuItem::Rename));
    assert!(rename.is_some_and(|e| !e.enabled && e.reason == Some(NO_DOCUMENT)));

    let (view, host, cx, root) = documented(cx);
    view.update(cx, |this, _| {
        let target = this
            .rename_target(space("setup"))
            .unwrap_or_else(|| panic!("target"));
        assert_eq!(this.rename_unavailable(&target), Some(NOT_IN_DOCUMENT));
        this.terminal.core_mode = false;
        assert_eq!(
            this.palette_reason(Command::RenameTab, false),
            None,
            "Herdr 1 has no reason"
        );
        this.terminal.core_mode = true;
    });
    assert!(host.doc().sets().is_empty());
    finish(&view, cx, root);
}

/// The menu bar's disabled Rename items say why, item by item.
#[test]
fn subtitles_say_why_on_herdr2() {
    let rows = vec![
        ("rename_tab".to_owned(), "Rename Tab".to_owned(), false),
        ("rename_pane".to_owned(), "Rename Pane".to_owned(), false),
        ("next_tab".to_owned(), "Next Tab".to_owned(), false),
    ];
    let host = vec![(
        host_menus::SPACE_MENU,
        vec![(
            "space.rename".to_owned(),
            "Rename Space — Space 1".to_owned(),
            true,
        )],
    )];
    let reason = |name: &str| {
        if name == "rename_pane" {
            HERDR2
        } else {
            NO_DOCUMENT
        }
    };
    let subtitles = crate::native_menus::rename_subtitles(&rows, &host, Some(&reason));
    assert_eq!(
        subtitles,
        [
            ("Rename Tab".to_owned(), NO_DOCUMENT.to_owned()),
            ("Rename Pane".to_owned(), HERDR2.to_owned()),
        ],
        "an enabled item has none"
    );
    assert!(crate::native_menus::rename_subtitles(&rows, &host, None).is_empty());
}

fn build_boot(this: &Smoke) -> String {
    crate::board::snapshot(&this.terminal.live).map_or_else(String::new, |s| s.boot_id.clone())
}

/// A double tap on a Herdr 2 tab's label or space row's name only says why
/// (#143 plan §3.3): no field, no session. `begin_rename`'s Herdr 2 refusal
/// (`rename_unavailable`) is the one check on both paths. This session reads
/// no `herdr-app` document, so the reason is that only its spaces and tabs
/// can be renamed (#144 D3).
#[gpui::test]
fn double_tap_reason_only(cx: &mut TestAppContext) {
    let (backend, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("mock core: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(gpui::size(gpui::px(1210.), gpui::px(834.)));
    view.update(cx, |this, cx| {
        this.select_host("rename-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait(&view, cx, "the Herdr 2 host did not connect", |this| {
        this.terminal.live.connected && !this.row_tabs().is_empty()
    });
    let fake = install_fake(&view, cx);
    draw(&view, cx);
    let row = view.update(cx, |this, _| {
        this.sidebar_rows("")
            .rows
            .into_iter()
            .find_map(|row| match row {
                ListRow::Space(space) if space.kind == HerdrKind::Herdr2 => Some(format!(
                    "sidebar-space-name-{}-{}",
                    space.host, space.entry.id
                )),
                _ => None,
            })
    });
    let row: &'static str = Box::leak(
        row.unwrap_or_else(|| panic!("fixture: no Herdr 2 space row"))
            .into_boxed_str(),
    );
    for selector in ["shell-tab-body-0", row] {
        view.update(cx, |this, _| this.terminal.live.status.clear());
        double_click(cx, selector);
        draw(&view, cx);
        view.read_with(cx, |this, _| {
            assert!(
                this.shell.rename.session.is_none(),
                "{selector}: a Herdr 2 rename started"
            );
            assert_eq!(this.terminal.live.status, NOT_IN_DOCUMENT, "{selector}");
        });
        assert_eq!(fake.presents(), 0, "{selector}: the field was presented");
    }
}
