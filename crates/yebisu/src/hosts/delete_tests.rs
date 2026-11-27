//! #250: deleting a saved host forgets every record of it on this iPad,
//! keeps a pin another saved target still uses, and never leaves the hosts
//! file half written.
use super::*;
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const KEY: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIHooS6ZFNGKNoEEPg0yvrG/ZYGJxNIk98w7H1XHhFMkt";
const A: &str = "a@mac";
const B: &str = "b@mac";
const PORT: &str = "c@mac:2222";
const MINI: &str = "d@mini";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/host-delete-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(root.join("ssh"))?;
        fs::create_dir_all(root.join("herdr2"))?;
        Ok(Self(root))
    }

    fn store(&self) -> Store {
        Store {
            root: self.0.clone(),
        }
    }

    fn layouts(&self) -> PathBuf {
        self.0.join("herdr2")
    }

    fn hosts_file(&self) -> PathBuf {
        self.0.join("yebisu-hosts.json")
    }

    /// Saves `targets` (the last one selected) with a name for each and a
    /// pin for each known-hosts name.
    fn save(&self, targets: &[&str]) -> Result<()> {
        let store = self.store();
        let mut pins = String::new();
        for target in targets {
            store.save_with_name(target, Some(&format!("name of {target}")))?;
            let name = parse(target)?.known_hosts_name();
            if !pins.contains(&format!("{name} ")) {
                pins.push_str(&format!("{name} ssh-ed25519 {KEY}\n"));
            }
        }
        fs::write(self.0.join("ssh/known_hosts"), pins)?;
        Ok(())
    }

    fn pinned(&self, target: &str) -> Result<bool> {
        Ok(matches!(self.store().pins(target)?, Pins::Keys(keys) if !keys.is_empty()))
    }

    /// Every file a Herdr 2 connection leaves for `target`.
    fn write_layouts(&self, target: &str) -> Result<Vec<PathBuf>> {
        let prefix = layout_prefix(target);
        let files = [
            format!("{prefix}default.json"),
            format!("{prefix}{}.json", hex("space-1795")),
            format!("{prefix}default.v2.json"),
            format!("{prefix}default.pending"),
        ]
        .map(|name| self.layouts().join(name));
        for file in &files {
            fs::write(file, "{}")?;
        }
        Ok(files.into())
    }

    fn pending_left(&self) -> bool {
        self.0.join("yebisu-target.pending").is_file()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn hex(value: &str) -> String {
    value.bytes().map(|b| format!("{b:02x}")).collect()
}

/// `core/store.rs` `prefix`, written out so this test does not trust it.
fn layout_prefix(target: &str) -> String {
    format!("core-{}-", hex(target))
}

fn exists_all(files: &[PathBuf]) -> bool {
    files.iter().all(|file| file.exists())
}

fn exists_none(files: &[PathBuf]) -> bool {
    files.iter().all(|file| !file.exists())
}

#[test]
fn delete_forgets_every_record_of_the_host_and_only_that_host() -> Result<()> {
    let f = Fixture::new()?;
    f.save(&[MINI, A])?;
    let deleted = f.write_layouts(A)?;
    let kept = f.write_layouts(MINI)?;
    let unrelated = f.layouts().join("notes.json");
    fs::write(&unrelated, "{}")?;
    fs::write(f.0.join("yebisu-target"), A)?;

    let after = f.store().delete(A, Some(&f.layouts()))?;

    let expected = SavedHosts {
        targets: vec![MINI.into()],
        selected: MINI.into(),
        names: [(MINI.into(), format!("name of {MINI}"))].into(),
    };
    assert_eq!(after, expected);
    // What a relaunch reads is the same.
    assert_eq!(f.store().load_hosts()?, expected);
    assert!(!f.pinned(A)?, "the deleted host's pin is gone");
    assert!(f.pinned(MINI)?, "another host's pin stays");
    assert!(exists_none(&deleted), "every layout of the host is gone");
    assert!(exists_all(&kept), "another host's layouts stay");
    assert!(unrelated.exists());
    assert!(!f.0.join("yebisu-target").exists());
    assert!(!f.pending_left());
    Ok(())
}

#[test]
fn a_pin_another_saved_target_uses_is_kept_until_the_last_one_goes() -> Result<()> {
    let f = Fixture::new()?;
    f.save(&[A, B, PORT])?;
    f.store().delete(A, Some(&f.layouts()))?;
    assert!(f.pinned(B)?, "b@mac still uses the pin for mac");
    // Another port is another known-hosts name: not shared.
    f.store().delete(PORT, Some(&f.layouts()))?;
    assert!(!f.pinned(PORT)?, "[mac]:2222 had no other user");
    assert!(f.pinned(B)?);
    f.store().delete(B, Some(&f.layouts()))?;
    assert!(!f.pinned(B)?, "the last target on mac took the pin with it");
    assert_eq!(f.store().load_hosts()?, SavedHosts::default());
    Ok(())
}

#[test]
fn selection_moves_to_the_next_saved_host_and_to_none_at_the_end() -> Result<()> {
    let f = Fixture::new()?;
    f.save(&[A, MINI, PORT])?;
    f.store().save(MINI)?;
    let delete = |target| f.store().delete(target, Some(&f.layouts()));
    assert_eq!(delete(MINI)?.selected, PORT, "the host after it");
    assert_eq!(
        delete(PORT)?.selected,
        A,
        "the last host: the one before it"
    );
    f.store().save_with_name(MINI, None)?;
    assert_eq!(delete(A)?.selected, MINI, "deleting another host keeps it");
    let empty = delete(MINI)?;
    assert_eq!(empty, SavedHosts::default());
    assert_eq!(f.store().load_hosts()?, SavedHosts::default());
    Ok(())
}

#[test]
fn deleting_a_target_that_is_not_saved_changes_nothing() -> Result<()> {
    let f = Fixture::new()?;
    f.save(&[A])?;
    let layouts = f.write_layouts(B)?;
    let before = fs::read(f.hosts_file())?;
    let after = f.store().delete(B, Some(&f.layouts()))?;
    assert_eq!(after.targets, vec![A.to_owned()]);
    assert_eq!(fs::read(f.hosts_file())?, before);
    assert!(
        f.pinned(A)?,
        "b@mac is not saved, so a@mac's shared pin is untouched"
    );
    assert!(exists_all(&layouts));
    Ok(())
}

#[test]
fn a_single_host_from_before_the_host_list_is_deleted() -> Result<()> {
    let f = Fixture::new()?;
    fs::write(f.0.join("yebisu-target"), A)?;
    assert_eq!(f.store().load_hosts()?.targets, vec![A.to_owned()]);
    f.store().delete(A, Some(&f.layouts()))?;
    assert_eq!(f.store().load_hosts()?, SavedHosts::default());
    assert!(!f.0.join("yebisu-target").exists());
    Ok(())
}

/// Each step that fails stops the delete before the hosts file changes: it
/// is byte for byte the old one, no pending file is left, and the host is
/// still listed. Deleting again once the cause is gone finishes the job.
fn assert_failed_step_keeps_the_hosts_file(
    break_step: impl FnOnce(&Fixture) -> Result<()>,
    repair: impl FnOnce(&Fixture) -> Result<()>,
) -> Result<()> {
    let f = Fixture::new()?;
    f.save(&[MINI, A])?;
    let layouts = f.write_layouts(A)?;
    let before = fs::read(f.hosts_file())?;
    break_step(&f)?;

    assert!(f.store().delete(A, Some(&f.layouts())).is_err());

    assert_eq!(
        fs::read(f.hosts_file())?,
        before,
        "hosts file is the old one"
    );
    assert!(!f.pending_left());
    assert!(f.store().load_hosts()?.targets.contains(&A.to_owned()));

    repair(&f)?;
    let after = f.store().delete(A, Some(&f.layouts()))?;
    assert_eq!(after.targets, vec![MINI.to_owned()]);
    assert_eq!(f.store().load_hosts()?, after);
    assert!(!f.pinned(A)?);
    assert!(exists_none(&layouts));
    Ok(())
}

fn replace_with_dir(path: &Path) -> Result<()> {
    let _ = fs::remove_file(path);
    fs::create_dir_all(path.join("blocker"))?;
    Ok(())
}

#[test]
fn a_pin_file_that_cannot_be_read_stops_the_delete_before_the_hosts_file() -> Result<()> {
    let parked = |f: &Fixture| f.0.join("ssh/known_hosts.parked");
    assert_failed_step_keeps_the_hosts_file(
        |f| {
            fs::rename(f.0.join("ssh/known_hosts"), parked(f))?;
            replace_with_dir(&f.0.join("ssh/known_hosts"))
        },
        |f| {
            fs::remove_dir_all(f.0.join("ssh/known_hosts"))?;
            fs::rename(parked(f), f.0.join("ssh/known_hosts"))?;
            Ok(())
        },
    )
}

#[test]
fn a_layout_directory_that_cannot_be_read_stops_the_delete_before_the_hosts_file() -> Result<()> {
    let parked = |f: &Fixture| f.0.join("herdr2-parked");
    assert_failed_step_keeps_the_hosts_file(
        |f| {
            fs::rename(f.layouts(), parked(f))?;
            fs::write(f.layouts(), "not a directory")?;
            Ok(())
        },
        |f| {
            fs::remove_file(f.layouts())?;
            fs::rename(parked(f), f.layouts())?;
            Ok(())
        },
    )
}

#[test]
fn a_hosts_file_that_cannot_be_written_is_left_whole() -> Result<()> {
    assert_failed_step_keeps_the_hosts_file(
        |f| replace_with_dir(&f.0.join("yebisu-target.pending")),
        |f| Ok(fs::remove_dir_all(f.0.join("yebisu-target.pending"))?),
    )
}
