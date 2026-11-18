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

/// Integration 2: L2b added `Command::Workspace` to the supported catalog, and
/// its OS menu row and palette row showed the raw name "new_workspace" (Find
/// showed "find" the same way).
/// Every command the app offers has an English label.
#[test]
fn every_supported_command_has_a_label() {
    for info in sheprd_commands::COMMANDS {
        if crate::actions::supported(info.command) {
            assert_ne!(command(info), info.name, "{} has no label", info.message);
        }
    }
    let workspace = sheprd_commands::COMMANDS
        .iter()
        .find(|info| info.command == sheprd_commands::Command::Workspace)
        .unwrap_or_else(|| panic!("Workspace is in the catalog"));
    assert_eq!(command(workspace), "New Space");
}
