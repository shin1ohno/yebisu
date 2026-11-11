use super::*;
#[test]
fn english_catalog_preserves_existing_command_labels() {
    assert!(ENGLISH.is_some(), "all P10 Fluent files must parse");
    for info in sheprd_commands::COMMANDS {
        let old = include_str!("commands.ftl")
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(" = ")?;
                (key == info.message).then_some(value)
            })
            .unwrap_or(info.name)
            .replace(
                "{ $number }",
                &info.command.number().unwrap_or(0).to_string(),
            );
        assert_eq!(command(info), old, "{}", info.message);
    }
}
