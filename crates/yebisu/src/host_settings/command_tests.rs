use super::{ConfigPath, command};

#[test]
fn fixed_literal_and_hex_are_the_entire_command() {
    let path = ConfigPath::new("/fixture/a b.toml").unwrap_or_else(|e| panic!("{e:?}"));
    let result = command::command(&path).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(result.starts_with("exec /usr/bin/python3 -I -S -B -c '"));
    assert!(result.ends_with(" '2f666978747572652f6120622e746f6d6c'"));
    assert!(result.len() <= command::MAX_COMMAND_BYTES);
    assert!(!result.contains(path.as_str()));
}

#[test]
fn maximum_valid_path_fits_and_shell_metacharacters_fail_before_encoding() {
    let path =
        ConfigPath::new(&format!("/{}", "a".repeat(4095))).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(command::command(&path).is_ok());
    for value in [
        "/tmp/$(id)",
        "/tmp/`id`",
        "/tmp/a'b",
        "/tmp/a\nb",
        "/tmp/../a",
    ] {
        assert!(ConfigPath::new(value).is_err());
    }
}
