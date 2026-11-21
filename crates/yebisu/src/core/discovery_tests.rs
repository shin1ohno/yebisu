use super::{DISCOVER, SessionInfo, SessionList, record_discovery};

const LIVE: &[u8] = br#"["external",["c0ffee",[{"address":"t1","state":"live","name":"agent 1","status":{"state":"working","title":"First"}}]]]"#;

fn poll(list: &SessionList, output: Option<&[u8]>) -> crate::Result<bool> {
    record_discovery(list, None, "test-host", output)
}

fn external(list: &SessionList) -> SessionInfo {
    let lists = list
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    lists
        .discovery
        .rows
        .iter()
        .flatten()
        .find(|row| row.name == "external")
        .cloned()
        .unwrap_or_else(|| panic!("external session must still be listed"))
}

#[test]
fn discovery_launches_python_in_isolated_mode_with_the_script_on_stdin() {
    assert_eq!(
        DISCOVER.lines().nth(1),
        Some("    exec python3 -I - <<'YEBISU_CORE_DISCOVERY'")
    );
    assert!(DISCOVER.contains(concat!(
        "\n",
        include_str!("../shell/herdr2_frame/discover.py"),
        "\nYEBISU_CORE_DISCOVERY\n"
    )));
}

#[test]
fn one_discovery_failure_keeps_confirmed_external_sessions() -> crate::Result<()> {
    let list = SessionList::default();
    assert!(poll(&list, Some(LIVE))?);
    let before = external(&list);
    assert!(before.confirmed);
    assert_eq!(before.core_id.as_deref(), Some("c0ffee"));
    assert_eq!(before.statuses.len(), 1);
    assert!(!poll(&list, None)?);
    assert_eq!(external(&list), before);
    Ok(())
}

#[test]
fn two_consecutive_discovery_failures_mark_external_sessions_unconfirmed() -> crate::Result<()> {
    let list = SessionList::default();
    poll(&list, Some(LIVE))?;
    let before = external(&list);
    assert_eq!(before.tabs.len(), 1);
    poll(&list, None)?;
    assert!(poll(&list, None)?);
    let after = external(&list);
    assert!(!after.confirmed);
    assert!(after.core_id.is_none());
    assert!(after.statuses.is_empty());
    assert_eq!(after.tabs, before.tabs);
    assert!(!poll(&list, None)?);
    Ok(())
}

#[test]
fn successful_discovery_restores_confirmation_and_resets_the_failure_streak() -> crate::Result<()> {
    let list = SessionList::default();
    poll(&list, Some(LIVE))?;
    let before = external(&list);
    poll(&list, None)?;
    poll(&list, None)?;
    assert!(poll(&list, Some(LIVE))?);
    assert_eq!(external(&list), before);
    assert!(!poll(&list, None)?);
    assert_eq!(external(&list), before);
    Ok(())
}

#[test]
fn successful_empty_discovery_removes_external_only_sessions() -> crate::Result<()> {
    let list = SessionList::default();
    poll(&list, Some(LIVE))?;
    assert!(poll(&list, Some(b""))?);
    let lists = list
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(lists.discovery.rows.as_deref(), Some([].as_slice()));
    Ok(())
}
