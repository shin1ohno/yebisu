//! English-only strings. The sheet can reuse the pinned Fluent engine in P11.
use sheprd_i18n::{Catalog, File, FluentArgs};
use std::sync::LazyLock;
const FILES: &[File] = &[
    File {
        language: "en-US",
        feature: "commands",
        source: include_str!("commands.ftl"),
    },
    File {
        language: "en-US",
        feature: "settings-fonts",
        source: include_str!("locales/settings-fonts.ftl"),
    },
    File {
        language: "en-US",
        feature: "settings-ipad",
        source: include_str!("locales/settings-ipad.ftl"),
    },
    File {
        language: "en-US",
        feature: "ipad-settings-sheet",
        source: include_str!("locales/ipad-settings-sheet.ftl"),
    },
];
static ENGLISH: LazyLock<Option<Catalog>> = LazyLock::new(|| match Catalog::english(FILES) {
    Ok(catalog) => Some(catalog),
    Err(error) => {
        tracing::error!(%error, "English catalog failed");
        None
    }
});
pub(crate) fn command(info: &sheprd_commands::CommandInfo) -> String {
    let Some(catalog) = ENGLISH.as_ref() else {
        return info.name.to_owned();
    };
    if !catalog.has(info.message) {
        return info.name.to_owned();
    }
    if let Some(number) = info.command.number() {
        let mut args = FluentArgs::new();
        args.set("number", number);
        catalog.text_with(info.message, &args)
    } else {
        catalog.text(info.message)
    }
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
