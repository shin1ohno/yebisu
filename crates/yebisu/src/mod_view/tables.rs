//! The receiving side's reflection tables. `ValidationTables` is empty by
//! default, so the renderer rejects every style and callback name that is not
//! listed here. The real tables come from the negotiated gpui-kit version
//! (E-1 work); this prototype lists only what `scene` can draw.
use std::collections::{BTreeMap, BTreeSet};
use yebisu_mod_wire as wire;

/// Nullary styles by wire index.
pub(crate) const STYLES: [(u16, &str); 4] = [
    (0, "flex_1"),
    (1, "size_full"),
    (2, "items_center"),
    (3, "w_full"),
];
/// Styles with one argument: four dimensions in points and two RGB colours.
pub(crate) const PARAM_STYLES: [&str; 6] = ["bg", "gap", "h", "p", "text_color", "w"];
pub(crate) const ON_CLICK: &str = "on_click";
pub(crate) const ON_KEY_DOWN: &str = "on_key_down";

pub(crate) fn tables() -> wire::ValidationTables {
    wire::ValidationTables {
        styles: STYLES
            .iter()
            .map(|(index, name)| (*index, (*name).to_string()))
            .collect::<BTreeMap<_, _>>(),
        param_styles: PARAM_STYLES.iter().map(|s| (*s).to_string()).collect(),
        methods: BTreeSet::new(),
        callbacks: [ON_CLICK, ON_KEY_DOWN]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        actions: BTreeSet::new(),
        slots: BTreeSet::new(),
    }
}
