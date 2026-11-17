//! D7: "Saved" only after the write succeeded; a failed write keeps the
//! draft, shows the reason and offers Retry.
use super::support::{directory, open, wait};
use crate::settings::{SaveStatus, SettingsHost};

fn on_disk(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("yebisu-settings.json")).unwrap_or_default()
}

#[test]
fn saved_appears_only_after_write_success() {
    let root = directory("saved");
    let mut host = open(&root);
    let mut values = host.draft.clone();
    values.option_alt = true;
    host.edit(values);
    assert_eq!(host.save_status(), SaveStatus::Saving, "edited, not sent");
    host.flush();
    assert_eq!(
        host.save_status(),
        SaveStatus::Saving,
        "sent, not confirmed"
    );
    wait(&mut host, |h| h.save_status() == SaveStatus::Saved);
    assert!(
        on_disk(&root).contains(r#""option_key":"alt""#),
        "Saved is shown only with the value on disk: {}",
        on_disk(&root)
    );
}

#[test]
fn failed_write_keeps_draft_and_offers_retry() {
    let root = directory("failed");
    let mut host = open(&root);
    // A directory where the writer creates its pending file fails the write.
    let blocker = root.join("yebisu-settings.pending");
    std::fs::create_dir_all(&blocker).unwrap_or_else(|e| panic!("{e}"));
    let mut values = host.draft.clone();
    values.caret_anchor = true;
    host.edit(values);
    wait(&mut host, |h| h.error.is_some());
    match host.save_status() {
        SaveStatus::Failed { reason, retry } => {
            assert!(!reason.is_empty(), "the reason is shown");
            assert!(retry, "a failed write offers Retry");
        }
        status => panic!("expected a failure, got {status:?}"),
    }
    assert!(
        host.draft.caret_anchor,
        "the edited value stays in the draft"
    );
    assert!(!on_disk(&root).contains("caret"), "nothing was written");
    std::fs::remove_dir(&blocker).unwrap_or_else(|e| panic!("{e}"));
    host.retry();
    wait(&mut host, SettingsHost::ready);
    assert_eq!(host.save_status(), SaveStatus::Saved);
    drop(host);
    assert!(open(&root).draft.caret_anchor, "Retry wrote the kept draft");
}
