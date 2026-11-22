use super::*;

pub(super) fn parse(text: &str) -> Profile {
    Profile::from_file(Ok(text.as_bytes())).unwrap_or_else(|error| panic!("profile: {error:?}"))
}

fn key(text: &str) -> Keystroke {
    parse_key(text).unwrap_or_else(|| panic!("key {text}"))
}

#[test]
fn herdr_prefix_values_accept_string_list_and_other_key_names() {
    for (config, expected) in [
        ("prefix = 'ctrl+a'", vec!["ctrl+a"]),
        (
            "prefix = ['ctrl+space', 'ctrl+s', 'bogus']",
            vec!["ctrl+space", "ctrl+s"],
        ),
        ("prefix = 'f12'", vec!["f12"]),
        ("prefix = 'ctrl++'", vec!["ctrl++"]),
        ("prefix = 'esc'", vec!["escape"]),
    ] {
        let profile = parse(&format!("[keys]\n{config}"));
        assert_eq!(profile.fallback, None);
        for wanted in expected {
            assert!(
                profile.prefix(&key(wanted)),
                "{config} should accept {wanted}"
            );
        }
        assert!(!profile.prefix(&key(DEFAULT_PREFIX)), "{config}");
    }
}

#[test]
fn herdr_prefix_missing_and_unusable_config_fall_back_with_different_reasons() {
    for (source, reason) in [
        ("[keys]\nhelp = 'prefix+f1'", Fallback::MissingPrefix),
        ("[keys]\nprefix = []", Fallback::InvalidPrefix),
        ("[keys]\nprefix = 'hyper+a'", Fallback::InvalidPrefix),
        ("[keys]\nprefix = 'ctrl+'", Fallback::InvalidPrefix),
        ("[keys]\nprefix = ['ctrl+a', 4]", Fallback::InvalidPrefix),
    ] {
        let profile = parse(source);
        assert_eq!(profile.fallback, Some(reason));
        assert!(profile.prefix(&key(DEFAULT_PREFIX)));
    }
}

#[test]
fn herdr_prefix_file_errors_are_bounded_and_specific() {
    for (source, expected) in [
        (Ok(b"[keys" as &[u8]), Fallback::InvalidToml),
        (Ok(&[0xff]), Fallback::InvalidUtf8),
        (Err(Fallback::MissingFile), Fallback::MissingFile),
        (Err(Fallback::Timeout), Fallback::Timeout),
    ] {
        assert_eq!(Profile::from_file(source).err(), Some(expected));
    }
    assert_eq!(
        Profile::from_file(Ok(&vec![b' '; MAX_BYTES + 1])).err(),
        Some(Fallback::TooLarge)
    );
}

#[test]
fn herdr_prefix_arms_and_dispatches_the_server_actions() {
    let profile = parse(
        r#"
[keys]
prefix = "ctrl+a"
new_tab = ["prefix+c", "ctrl+alt+t"]
split_vertical = "prefix+g"
split_horizontal = "prefix+shift+-"
next_tab = "prefix+]"
previous_tab = "prefix+["
close_tab = "prefix+shift+w"
help = "prefix+f1"
"#,
    );
    let mut state = Prefix::default();
    let mut route = |spec| state.route(&profile, &profile, &key(spec), false);
    for (chord, command) in [
        ("c", Command::Tab),
        ("g", Command::SplitRight),
        ("_", Command::SplitDown),
        ("]", Command::NextTab),
        ("[", Command::PreviousTab),
        ("shift+w", Command::CloseTab),
        ("f1", Command::Keybinds),
    ] {
        assert_eq!(route("ctrl+a"), Route::Consume(None));
        assert_eq!(route(chord), Route::Consume(Some(command)), "{chord}");
        assert_eq!(route(chord), Route::Terminal, "the chord disarms");
    }
    assert_eq!(route("ctrl+alt+t"), Route::Consume(Some(Command::Tab)));
}

#[test]
fn herdr_prefix_replays_a_double_press_and_disarms_unknown_chords() {
    let profile = parse("[keys]\nprefix = ['ctrl+a', 'ctrl+b']\nclose_pane = []");
    let mut state = Prefix::default();
    let mut route = |spec| state.route(&profile, &profile, &key(spec), false);
    assert_eq!(route("ctrl+a"), Route::Consume(None));
    assert_eq!(route("ctrl+a"), Route::Replay);
    assert_eq!(route("c"), Route::Terminal);
    assert_eq!(route("ctrl+a"), Route::Consume(None));
    assert_eq!(route("x"), Route::Consume(None), "explicitly empty binding");
    assert_eq!(route("c"), Route::Terminal);
    assert_eq!(route("ctrl+a"), Route::Consume(None));
    assert_eq!(
        route("ctrl+b"),
        Route::Consume(None),
        "a different prefix is an unknown chord"
    );
    assert_eq!(route("c"), Route::Terminal);
}

#[test]
fn herdr_prefix_switch_off_keeps_the_servers_prefix_and_uses_default_chords() {
    let profile = parse("[keys]\nprefix = 'ctrl+a'\nnew_tab = 'prefix+t'");
    let mut state = Prefix::default();
    let mut route = |spec| state.route(&DEFAULTS, &profile, &key(spec), false);
    assert_eq!(route("ctrl+b"), Route::Terminal);
    assert_eq!(route("ctrl+a"), Route::Consume(None));
    assert_eq!(route("t"), Route::Consume(None));
    assert_eq!(route("ctrl+a"), Route::Consume(None));
    assert_eq!(route("c"), Route::Consume(Some(Command::Tab)));
}

#[test]
fn herdr_prefix_held_key_does_not_arm_or_retrigger() {
    let profile = parse("[keys]\nprefix = 'ctrl+a'");
    let mut state = Prefix::default();
    assert_eq!(
        state.route(&profile, &profile, &key("ctrl+a"), true),
        Route::Terminal
    );
    assert_eq!(
        state.route(&profile, &profile, &key("c"), false),
        Route::Terminal
    );
    assert_eq!(
        state.route(&profile, &profile, &key("ctrl+a"), false),
        Route::Consume(None)
    );
    assert_eq!(
        state.route(&profile, &profile, &key("ctrl+a"), true),
        Route::Terminal
    );
    assert_eq!(
        state.route(&profile, &profile, &key("c"), false),
        Route::Consume(Some(Command::Tab))
    );
}

#[test]
fn hint_command_aliases_do_not_become_host_actions() {
    let profile = parse(
        "[keys]\nprefix = 'ctrl+a'\nnew_tab = ['cmd+t', 'command+y', 'super+u', 'prefix+cmd+c', 'prefix+command+v', 'prefix+super+x']",
    );
    let mut prefix = Prefix::default();
    for chord in ["cmd+t", "command+y", "super+u"] {
        assert_eq!(
            prefix.route(&profile, &profile, &key(chord), false),
            Route::Terminal
        );
    }
    for chord in ["cmd+c", "command+v", "super+x"] {
        assert_eq!(
            prefix.route(&profile, &profile, &key("ctrl+a"), false),
            Route::Consume(None)
        );
        assert_eq!(
            prefix.route(&profile, &profile, &key(chord), false),
            Route::Terminal
        );
    }
}
