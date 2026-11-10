use super::*;
use sheprd_client::ssh::{HostKeyFingerprint, HostKeyScan, NewHostKey, SshFailure};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const KEY: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIHooS6ZFNGKNoEEPg0yvrG/ZYGJxNIk98w7H1XHhFMkt";
const FINGERPRINT: &str = "ssh-ed25519 SHA256:JOTFRvCkuEl2HbyffolO4A+q4cUOVf0UHhWQLrlaEiU";
// Known key and expected digest from herdr-ssh/src/host_key/tests.rs at 398c132a0.
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Result<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/p2-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&root)?;
        Ok(Self(root))
    }
    fn store(&self) -> Store {
        Store {
            root: self.0.clone(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn untrusted_host_key_refuses_authentication() -> Result<()> {
    let f = Fixture::new()?;
    assert!(f.store().fingerprints("a@mac").is_err());
    let scan = HostKeyScan::New(NewHostKey::fixture_for_tests(
        f.0.join("known_hosts"),
        vec![HostKeyFingerprint {
            kind: "ED25519".into(),
            bits: 256,
            fingerprint: FINGERPRINT.into(),
        }],
    ));
    assert!(authentication_gate(&scan).is_err());
    Ok(())
}

#[test]
fn mismatched_host_key_refuses_authentication() {
    assert!(authentication_gate(&HostKeyScan::Refused(SshFailure::HostKey)).is_err());
    assert!(authentication_gate(&HostKeyScan::Refused(SshFailure::Auth)).is_err());
    assert!(authentication_gate(&HostKeyScan::Trusted).is_ok()); // positive control
}

#[test]
fn fingerprint_is_identical_after_restart() -> Result<()> {
    let f = Fixture::new()?;
    fs::create_dir_all(f.0.join("ssh"))?;
    fs::write(
        f.0.join("ssh/known_hosts"),
        format!("mac ssh-ed25519 {KEY}\n"),
    )?;
    {
        let store = f.store();
        store.save("a@mac")?;
        assert_eq!(store.fingerprints("a@mac")?, FINGERPRINT);
    }
    let reopened = f.store();
    assert_eq!(reopened.load()?, "a@mac");
    assert_eq!(reopened.fingerprints(&reopened.load()?)?, FINGERPRINT);
    // Even a disk-editing error must not display a cached fingerprint.
    fs::write(f.0.join("ssh/known_hosts"), "")?;
    assert!(reopened.fingerprints(&reopened.load()?).is_err());
    Ok(())
}

#[test]
fn save_failure_is_not_success_and_leaves_old_target() -> Result<()> {
    let f = Fixture::new()?;
    let store = f.store();
    store.save("a@mac")?;
    fs::create_dir(store.root.join("yebisu-target.pending"))?;
    let error = store.save("b@other");
    assert!(error.is_err());
    assert_eq!(store.load()?, "a@mac");
    Ok(())
}

#[test]
fn trust_does_not_overwrite_existing_pins_and_reports_disk_failure() -> Result<()> {
    let f = Fixture::new()?;
    let path = f.0.join("ssh/known_hosts");
    fs::create_dir_all(f.0.join("ssh"))?;
    fs::write(&path, format!("mac ssh-ed25519 {KEY}\n"))?;
    let pins = KnownHosts::new(&path)
        .pins(&parse("a@mac")?)
        .map_err(|e| Error::Ssh(e.to_string()))?;
    let Pins::Keys(keys) = pins else {
        return Err(Error::UntrustedHost);
    };
    assert!(
        KnownHosts::new(&path)
            .trust(&parse("a@mac")?, &keys)
            .is_err()
    );
    let broken = f.0.join("file");
    fs::write(&broken, "not a directory")?;
    assert!(
        KnownHosts::new(broken.join("known_hosts"))
            .trust(&parse("a@new")?, &keys)
            .is_err()
    );
    assert_eq!(f.store().fingerprints("a@mac")?, FINGERPRINT);
    Ok(())
}

#[test]
fn multiple_hosts_survive_restart_and_legacy_migration() -> Result<()> {
    let f = Fixture::new()?;
    fs::write(f.0.join("yebisu-target"), "a@old")?;
    f.store().save("b@new")?;
    f.store().save("a@old")?;
    let reopened = f.store().load_hosts()?;
    assert_eq!(reopened.targets, ["a@old", "b@new"]);
    assert_eq!(reopened.selected, "a@old");
    assert!(
        f.store()
            .pins("b@new")
            .is_ok_and(|pins| matches!(pins, Pins::None))
    );
    Ok(())
}
#[test]
fn corrupt_or_future_format_is_not_silently_overwritten() -> Result<()> {
    let f = Fixture::new()?;
    for bytes in [
        "not json",
        r#"{"version":2,"targets":[],"selected":""}"#,
        r#"{"version":1,"targets":["a@one"],"selected":"b@two"}"#,
    ] {
        fs::write(f.0.join("yebisu-hosts.json"), bytes)?;
        assert!(f.store().save("c@three").is_err());
        assert_eq!(fs::read_to_string(f.0.join("yebisu-hosts.json"))?, bytes);
    }
    Ok(())
}
