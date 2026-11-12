use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
const RAW: &str = "restrict ssh-ed25519 RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM herdr-ipad-injected";
struct Fake {
    reads: AtomicUsize,
    replacements: AtomicUsize,
    result: Mutex<Result<String, Failure>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}
impl Fake {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            reads: AtomicUsize::new(0),
            replacements: AtomicUsize::new(0),
            result: Mutex::new(Ok(RAW.into())),
            release: Mutex::new(None),
        })
    }
}
impl Provider for Fake {
    fn supported(&self) -> bool {
        true
    }
    fn read_or_initialize(&self) -> Result<String, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.result
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn replace(&self) -> Result<String, Failure> {
        self.replacements.fetch_add(1, Ordering::SeqCst);
        if let Some(wait) = self
            .release
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = wait.recv();
        }
        self.result
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
fn settle(owner: &mut Owner) {
    let until = Instant::now() + Duration::from_secs(3);
    while owner.busy() {
        owner.poll(Instant::now());
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn ready(fake: &Arc<Fake>) -> Owner {
    let mut owner = Owner::new(fake.clone());
    assert!(owner.read_or_initialize());
    settle(&mut owner);
    assert!(owner.line().is_some());
    owner
}

#[test]
fn f01_f02_cancel_closed_and_old_owner_tickets_cannot_submit() {
    let fake = Fake::new();
    let mut owner = ready(&fake);
    owner.view_open();
    let a = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
    owner.cancel();
    assert!(!owner.confirm(a, true, false));
    let a = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
    owner.view_open();
    let b = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
    assert_ne!(a, b);
    assert!(!owner.confirm(a, true, false));
    assert!(!owner.confirm(b, false, false));
    assert_eq!(fake.replacements.load(Ordering::SeqCst), 0);
}
#[test]
fn f03_f04_f07_f08_double_busy_close_and_immediate_copy_invalidation() {
    let fake = Fake::new();
    let (release, wait) = mpsc::sync_channel(1);
    *fake.release.lock().unwrap_or_else(|e| e.into_inner()) = Some(wait);
    let mut owner = ready(&fake);
    let t = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
    assert!(!owner.confirm(t, true, true));
    assert!(owner.confirm(t, true, false));
    assert!(owner.line().is_none());
    assert!(!owner.confirm(t, true, false));
    owner.cancel();
    owner.view_open();
    assert!(owner.busy());
    assert!(!owner.read_or_initialize());
    assert!(owner.confirmation().is_none());
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut owner);
    assert_eq!(fake.replacements.load(Ordering::SeqCst), 1);
    assert!(owner.line().is_none());
    assert_eq!(owner.state, State::Unknown);
}
#[test]
fn f10_f11_f14_commit_error_watchdog_requires_termination_and_reconciliation() {
    let fake = Fake::new();
    let mut owner = ready(&fake);
    let (release, wait) = mpsc::sync_channel(1);
    *fake.release.lock().unwrap_or_else(|e| e.into_inner()) = Some(wait);
    *fake.result.lock().unwrap_or_else(|e| e.into_inner()) = Err(Failure::Provider);
    let t = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
    assert!(owner.confirm(t, true, false));
    owner.poll(Instant::now() + Duration::from_secs(16));
    assert_eq!(owner.state, State::Unknown);
    assert!(owner.busy());
    assert!(!owner.read_or_initialize());
    assert!(owner.confirmation().is_none());
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut owner);
    assert!(owner.read_or_initialize());
    settle(&mut owner);
    assert!(owner.line().is_none());
    assert!(owner.confirmation().is_none());
    assert_eq!(fake.replacements.load(Ordering::SeqCst), 1);
    *fake.result.lock().unwrap_or_else(|e| e.into_inner()) = Ok(RAW.into());
    assert!(owner.read_or_initialize());
    settle(&mut owner);
    assert!(owner.confirmation().is_some());
    assert_eq!(fake.replacements.load(Ordering::SeqCst), 1);
}
#[test]
fn f13_f17_f23_raw_format_and_provider_failures_never_become_copyable() {
    for result in [
        Err(Failure::Locked),
        Err(Failure::Missing),
        Err(Failure::Unsupported),
        Ok("FAKE_PRIVATE_SENTINEL\n".into()),
        Ok("restrict ssh-ed25519 !!! herdr-ipad".into()),
    ] {
        let fake = Fake::new();
        *fake.result.lock().unwrap_or_else(|e| e.into_inner()) = result;
        let mut owner = Owner::new(fake.clone());
        owner.read_or_initialize();
        settle(&mut owner);
        assert!(owner.line().is_none());
        assert!(owner.confirmation().is_none());
        assert_eq!(fake.replacements.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn f16_success_also_requires_explicit_read_and_f18_reads_share_owner() {
    let fake = Fake::new();
    let mut owner = ready(&fake);
    let t = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
    owner.confirm(t, true, false);
    settle(&mut owner);
    assert_eq!(owner.state, State::Unknown);
    assert!(owner.line().is_none());
    assert!(owner.confirmation().is_none());
    owner.read_or_initialize();
    settle(&mut owner);
    assert_eq!(
        owner.line(),
        Some("restrict ssh-ed25519 RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM yebisu-injected")
    );
    assert_eq!(fake.reads.load(Ordering::SeqCst), 2);
}
#[test]
fn f26_unsupported_default_has_no_credential_capability() {
    let mut owner = Owner::default();
    assert!(!owner.supported());
    assert!(!owner.read_or_initialize());
    assert!(owner.confirmation().is_none());
}

#[test]
fn f05_f06_real_queue_admission_failures_consume_confirmation_without_mutation() {
    for full in [false, true] {
        let fake = Fake::new();
        let mut owner = ready(&fake);
        let _ticket = owner.confirmation();
        owner.ticket.take();
        let (send, receive) = mpsc::sync_channel(1);
        if full {
            send.try_send(Operation::Read)
                .unwrap_or_else(|_| panic!("fill"));
        } else {
            drop(receive);
        }
        assert!(!owner.enqueue(&send, Operation::Replace, 2));
        assert_eq!(owner.state, State::NotStarted);
        assert_eq!(owner.failure, Some(Failure::Queue));
        assert!(owner.ticket.is_none());
        assert_eq!(fake.replacements.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn f09_f12_lost_or_wrong_reply_is_unknown_after_join() {
    for wrong in [false, true] {
        let fake = Fake::new();
        let mut owner = ready(&fake);
        owner.unresolved = true;
        owner.key = 7;
        owner.display = KeyView::loading();
        let (send, receiver) = mpsc::sync_channel(1);
        let handle = std::thread::spawn(move || {
            if wrong {
                let _ = send.send(Reply {
                    op: 99,
                    key: 99,
                    value: Ok(RAW.into()),
                });
            }
        });
        owner.worker = Some(Worker {
            handle,
            receiver,
            op: 2,
            key: 7,
            kind: Operation::Replace,
            started: Instant::now(),
            admitted: true,
            view: owner.view,
        });
        settle(&mut owner);
        assert_eq!(owner.state, State::Unknown);
        assert!(owner.line().is_none());
        assert!(owner.confirmation().is_none());
    }
}
#[test]
fn f26_wrong_identity_refuses_even_supported_provider() {
    struct Wrong;
    impl Provider for Wrong {
        fn supported(&self) -> bool {
            true
        }
        fn identity(&self) -> &'static str {
            "another-app-tag"
        }
        fn read_or_initialize(&self) -> Result<String, Failure> {
            panic!("wrong provider invoked")
        }
        fn replace(&self) -> Result<String, Failure> {
            panic!("wrong provider invoked")
        }
    }
    let mut owner = Owner::new(Arc::new(Wrong));
    assert_eq!(owner.state, State::Unsupported);
    assert!(!owner.read_or_initialize());
    assert!(owner.confirmation().is_none());
}

#[test]
fn f08_old_view_read_finishes_owner_without_publishing_into_reopened_view() {
    struct SlowRead {
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl Provider for SlowRead {
        fn supported(&self) -> bool {
            true
        }
        fn read_or_initialize(&self) -> Result<String, Failure> {
            self.release
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .recv()
                .map_err(|_| Failure::Worker)?;
            Ok(RAW.into())
        }
        fn replace(&self) -> Result<String, Failure> {
            panic!("read must exclude replacement")
        }
    }
    let (release, wait) = mpsc::sync_channel(1);
    let mut owner = Owner::new(Arc::new(SlowRead {
        release: Mutex::new(wait),
    }));
    owner.read_or_initialize();
    owner.view_open();
    assert!(owner.confirmation().is_none());
    assert!(!owner.read_or_initialize());
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut owner);
    assert!(!owner.busy());
    assert!(owner.line().is_none());
    assert_eq!(owner.state, State::Unavailable);
}
#[test]
fn f12_panicked_worker_is_joined_but_never_retried_implicitly() {
    struct Panics;
    impl Provider for Panics {
        fn supported(&self) -> bool {
            true
        }
        fn read_or_initialize(&self) -> Result<String, Failure> {
            Ok(RAW.into())
        }
        fn replace(&self) -> Result<String, Failure> {
            panic!("injected worker termination")
        }
    }
    let mut owner = Owner::new(Arc::new(Panics));
    owner.read_or_initialize();
    settle(&mut owner);
    let ticket = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
    owner.confirm(ticket, true, false);
    settle(&mut owner);
    assert_eq!(owner.state, State::Unknown);
    assert_eq!(owner.failure, Some(Failure::Worker));
    assert!(owner.confirmation().is_none());
    assert!(owner.line().is_none());
}
#[test]
fn f15_f16_injected_slot_faults_reconcile_authoritative_key_only() {
    struct Slots {
        active: Mutex<Option<&'static str>>,
        retired: Mutex<Option<&'static str>>,
        fail_promote: bool,
        calls: AtomicUsize,
    }
    const OLD: &str = "restrict ssh-ed25519 YWJjZA== herdr-ipad-old";
    const NEW: &str = "restrict ssh-ed25519 ZWZnaA== herdr-ipad-new";
    impl Provider for Slots {
        fn supported(&self) -> bool {
            true
        }
        fn read_or_initialize(&self) -> Result<String, Failure> {
            let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
            if active.is_none() {
                *active = self
                    .retired
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
            }
            active.map(str::to_owned).ok_or(Failure::Missing)
        }
        fn replace(&self) -> Result<String, Failure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
            *self.retired.lock().unwrap_or_else(|e| e.into_inner()) = active.take();
            if self.fail_promote {
                return Err(Failure::Provider);
            }
            *active = Some(NEW); // Deliberately leave retired behind: injected cleanup failure.
            Ok(NEW.into())
        }
    }
    for fail_promote in [false, true] {
        let provider = Arc::new(Slots {
            active: Mutex::new(Some(OLD)),
            retired: Mutex::new(None),
            fail_promote,
            calls: AtomicUsize::new(0),
        });
        let mut owner = Owner::new(provider.clone());
        owner.read_or_initialize();
        settle(&mut owner);
        let ticket = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
        owner.confirm(ticket, true, false);
        settle(&mut owner);
        assert_eq!(owner.state, State::Unknown);
        assert!(owner.line().is_none());
        assert!(owner.confirmation().is_none());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        owner.read_or_initialize();
        settle(&mut owner);
        let expected = crate::keys::authorized_line(if fail_promote { OLD } else { NEW })
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(owner.line(), Some(expected.as_str()));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }
}
