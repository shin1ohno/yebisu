//! The menu bar's Host / Space items name their target as the sidebar does.
use super::super::*;
use super::support::smoke;
use gpui::TestAppContext;

#[gpui::test]
fn space_menu_names_the_space(cx: &mut TestAppContext) {
    let (view, cx) = smoke(cx, "alpha");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // The fixture's focused space is `w1`, number 1, labelled "repo".
            assert_eq!(this.menu_space_name().as_deref(), Some("Space 1 · repo"));
            this.install_native_menus(window, cx);
            let labels = this
                .native_menus
                .host_menus
                .iter()
                .flat_map(|(title, rows)| rows.iter().map(move |row| (*title, row.1.clone())))
                .collect::<Vec<_>>();
            assert!(
                labels.contains(&(HOST_MENU, "Reconnect — alpha".to_owned())),
                "{labels:?}"
            );
            let space = labels
                .iter()
                .filter(|(title, _)| *title == SPACE_MENU)
                .collect::<Vec<_>>();
            assert!(!space.is_empty(), "{labels:?}");
            for (_, label) in space {
                assert!(label.ends_with(" — Space 1 · repo"), "{label}");
                assert!(!label.contains("w1"), "{label}");
            }
        })
    });
}
