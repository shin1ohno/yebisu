//! Compile-spike strings use the same engine and original English catalog.
use sheprd_i18n::{Catalog, File};
use std::sync::LazyLock;
static ENGLISH: LazyLock<Option<Catalog>> = LazyLock::new(|| {
    Catalog::english(&[
        File {
            language: "en-US",
            feature: "settings",
            source: include_str!("settings.ftl"),
        },
        File {
            language: "en-US",
            feature: "fonts",
            source: include_str!("../locales/settings-fonts.ftl"),
        },
        File {
            language: "en-US",
            feature: "ipad",
            source: include_str!("../locales/settings-ipad.ftl"),
        },
    ])
    .ok()
});
pub(super) fn text(id: &'static str) -> String {
    ENGLISH
        .as_ref()
        .map_or_else(|| id.to_owned(), |catalog| catalog.text(id))
}
macro_rules! fl {
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = sheprd_i18n::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::settings_spike::strings::with_args($id, &args)
    }};
    ($id:literal) => {
        $crate::settings_spike::strings::text($id)
    };
}
pub(super) use fl;
#[cfg(test)]
pub(super) fn valid() -> bool {
    ENGLISH.is_some()
}

pub(super) fn with_args(id: &str, args: &sheprd_i18n::FluentArgs) -> String {
    ENGLISH
        .as_ref()
        .map_or_else(|| id.to_owned(), |catalog| catalog.text_with(id, args))
}
