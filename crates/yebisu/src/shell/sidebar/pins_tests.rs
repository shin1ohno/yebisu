use super::*;

#[test]
fn agent_pin_survives_restart_and_drops_closed_panes() -> Result<()> {
    let root = std::env::var_os("TMPDIR")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(format!("yebisu-sidebar-pins-{}", std::process::id()));
    let id = AgentIdentity {
        host: "test".into(),
        kind: HerdrKind::Herdr1,
        pane: "w1:p1".into(),
    };
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(crate::mock_peer::SNAPSHOT)
        .unwrap_or_else(|e| panic!("snapshot: {e}"));
    let Some(pane) = snapshot.panes.first_mut() else {
        panic!("fixture has no pane");
    };
    pane.pane_id.clone_from(&id.pane);
    let boot = snapshot.boot_id.clone();
    let mut pins = Pins::default();
    pins.toggle(id.clone(), boot.clone());
    pins.save(&root)?;
    let mut loaded = Pins::load(&root)?;
    assert_eq!(loaded.index(&id, &boot), Some(0));
    loaded.reconcile(&id.host, &snapshot);
    assert_eq!(loaded.index(&id, &boot), Some(0));
    snapshot.panes.clear();
    loaded.reconcile(&id.host, &snapshot);
    loaded.save(&root)?;
    assert_eq!(Pins::load(&root)?.index(&id, &boot), None);
    fs::remove_dir_all(root)?;
    Ok(())
}
