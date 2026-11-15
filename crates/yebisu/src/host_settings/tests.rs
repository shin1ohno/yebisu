use super::*;

fn path(value: &str) -> ConfigPath {
    ConfigPath::new(value).unwrap_or_else(|e| panic!("{e:?}"))
}
fn host(value: &str) -> HostId {
    HostId::new(value).unwrap_or_else(|e| panic!("{e:?}"))
}
fn begin(model: &mut Model, name: &str, generation: u64, now: Instant) -> Identity {
    let Request::Read { identity } = model
        .begin(
            host(name),
            generation,
            path("/home/test/.config/herdr/config.toml"),
            now,
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
    identity
}
fn reply(identity: Identity) -> Snapshot {
    Snapshot {
        identity,
        file: parse(Some(b"[experimental]\npane_history = true")),
    }
}

#[test]
fn missing_empty_false_and_unknown_fields_are_distinct() {
    assert_eq!(parse(None), FileState::Missing);
    assert_eq!(
        parse(Some(b"")),
        FileState::Parsed(KnownSettings { pane_history: None })
    );
    assert_eq!(
        parse(Some(b"[experimental]\npane_history=false\nfuture='secret'")),
        FileState::Parsed(KnownSettings {
            pane_history: Some(false)
        })
    );
    assert_eq!(
        parse(Some(b"[experimental]\npane_history=true")),
        FileState::Parsed(KnownSettings {
            pane_history: Some(true)
        })
    );
}

#[test]
fn malformed_wrong_type_duplicate_and_invalid_utf8_fail_closed() {
    for input in [
        b"broken = [".as_slice(),
        b"[experimental]\npane_history=true\npane_history=false",
    ] {
        assert_eq!(parse(Some(input)), FileState::Failed(Error::InvalidToml));
    }
    for input in [
        b"experimental = 1".as_slice(),
        b"[experimental]\npane_history='true'",
    ] {
        assert_eq!(parse(Some(input)), FileState::Failed(Error::InvalidField));
    }
    assert_eq!(parse(Some(&[0xff])), FileState::Failed(Error::InvalidUtf8));
}

#[test]
fn bytes_limit_includes_all_unknown_content() {
    assert!(matches!(
        parse(Some(&vec![b' '; MAX_BYTES])),
        FileState::Parsed(_)
    ));
    assert_eq!(
        parse(Some(&vec![b' '; MAX_BYTES + 1])),
        FileState::Failed(Error::TooLarge)
    );
}

#[test]
fn paths_are_data_with_no_guess_or_shell_syntax() {
    assert_eq!(ConfigPath::new(""), Err(Error::UnknownPath));
    for input in [
        "relative",
        "/",
        "/tmp/../private",
        "/tmp/./config",
        "/tmp//config",
        "/tmp/config/",
        "/tmp/$(id)",
        "/tmp/`id`",
        "/tmp/x;id",
        "/tmp/x\nfile",
        "/tmp/\0config",
        "/tmp/${HOME}",
        "/tmp/'config'",
    ] {
        assert_eq!(ConfigPath::new(input), Err(Error::UnsafePath), "{input:?}");
    }
    assert_eq!(
        path("/home/test user/config.toml").as_str(),
        "/home/test user/config.toml"
    );
    assert_eq!(
        ConfigPath::new(&format!("/{}", "a".repeat(4096))),
        Err(Error::UnsafePath)
    );
}

#[test]
fn exact_host_generation_request_and_path_are_required() {
    let now = Instant::now();
    let mut model = Model::default();
    let a = begin(&mut model, "a", 1, now);
    let b = begin(&mut model, "b", 1, now);
    assert!(!model.complete(reply(a), now));
    let mut wrong_host = b.clone();
    wrong_host.host = host("a");
    let mut wrong_generation = b.clone();
    wrong_generation.connection_generation = 2;
    let mut wrong_request = b.clone();
    wrong_request.request_id += 1;
    let mut wrong_path = b.clone();
    wrong_path.path = path("/other/config.toml");
    for wrong in [wrong_host, wrong_generation, wrong_request, wrong_path] {
        assert!(!model.complete(reply(wrong), now));
        assert_eq!(model.state(), &State::Loading(b.clone()));
    }
    assert!(model.complete(reply(b.clone()), now));
    assert!(!model.complete(reply(b), now));
}

#[test]
fn same_host_same_generation_reissue_cannot_collide() {
    let now = Instant::now();
    let mut model = Model::default();
    let old = begin(&mut model, "a", 1, now);
    let new = begin(&mut model, "a", 1, now);
    assert_ne!(old.request_id, new.request_id);
    assert!(!model.complete(reply(old), now));
    assert!(model.complete(reply(new), now));
}

#[test]
fn cancel_disconnect_and_deadline_reject_late_replies() {
    let now = Instant::now();
    let mut model = Model::default();
    let id = begin(&mut model, "a", 1, now);
    model.cancel();
    assert_eq!(
        model.state(),
        &State::Finished(Snapshot {
            identity: id.clone(),
            file: FileState::Failed(Error::Cancelled),
        })
    );
    assert!(!model.complete(reply(id), now));
    let id = begin(&mut model, "a", 2, now);
    model.disconnect();
    assert!(!model.complete(reply(id), now));
    assert_eq!(model.state(), &State::Disconnected);
    let id = begin(&mut model, "a", 3, now);
    assert!(!model.complete(reply(id.clone()), now + TIMEOUT));
    assert_eq!(
        model.state(),
        &State::Finished(Snapshot {
            identity: id,
            file: FileState::Failed(Error::Timeout),
        })
    );
    assert!(!model.expire(now + TIMEOUT));
}

#[test]
fn unsafe_file_and_transport_errors_do_not_become_missing() {
    let now = Instant::now();
    let mut model = Model::default();
    for error in [
        Error::Symlink,
        Error::NotRegular,
        Error::PermissionDenied,
        Error::Transport,
    ] {
        let id = begin(&mut model, "a", 1, now);
        let snapshot = Snapshot {
            identity: id,
            file: FileState::Failed(error),
        };
        assert!(model.complete(snapshot.clone(), now));
        assert_eq!(model.state(), &State::Finished(snapshot));
    }
}

#[test]
fn request_counter_exhaustion_clears_old_state() {
    let now = Instant::now();
    let mut model = Model::default();
    begin(&mut model, "a", 1, now);
    model.next_request = u64::MAX;
    assert_eq!(
        model.begin(host("a"), 1, path("/config"), now),
        Err(Error::Exhausted)
    );
    assert_eq!(model.state(), &State::Disconnected);
}
