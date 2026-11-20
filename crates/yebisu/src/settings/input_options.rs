//! SH1-157 L1 (Q7, q7): the Input options and the Return policy in the
//! settings file, and since the design fidelity plan's Wave 0a the Sidebar
//! density (`sidebar_density.rs`). Each is a two-way choice stored by name.
//! A missing key (a file written before L1) or a name this build does not
//! know loads the default, never an error, so an old or newer file still
//! opens. A default value is not written, which keeps the file of an owner
//! who never changed these options identical to the one older builds wrote.
use super::Values;
use serde_json::{Map, Value};

/// One stored two-way choice: `off` is the name for `false`, `on` for `true`.
pub(crate) struct Field {
    pub key: &'static str,
    pub off: &'static str,
    pub on: &'static str,
    pub default: bool,
    pub get: fn(&Values) -> bool,
    pub set: fn(&mut Values, bool),
}

impl Field {
    /// The stored value, or the default for a missing or unknown name.
    pub(crate) fn read(&self, document: &Value) -> bool {
        let Some(stored) = document.get(self.key) else {
            // A file written before L1.
            return self.default;
        };
        match stored.as_str() {
            Some(name) if name == self.on => true,
            Some(name) if name == self.off => false,
            // A name (or type) this build does not know.
            _ => self.default,
        }
    }

    fn write(&self, document: &mut Map<String, Value>, value: bool) {
        if value == self.default {
            document.remove(self.key);
        } else {
            let name = if value { self.on } else { self.off };
            document.insert(self.key.to_owned(), Value::from(name));
        }
    }
}

pub(crate) const CANDIDATES: Field = Field {
    key: "japanese_candidates",
    off: "clause",
    on: "caret",
    default: false,
    get: |v| v.caret_anchor,
    set: |v, on| v.caret_anchor = on,
};
pub(crate) const COMMAND_KEY: Field = Field {
    key: "command_key",
    off: "local",
    on: "ctrl",
    default: false,
    get: |v| v.command_control,
    set: |v, on| v.command_control = on,
};
pub(crate) const OPTION_KEY: Field = Field {
    key: "option_key",
    off: "native",
    on: "alt",
    default: false,
    get: |v| v.option_alt,
    set: |v, on| v.option_alt = on,
};
pub(crate) const RETURN_POLICY: Field = Field {
    key: "return_policy",
    off: "tap",
    on: "automatic",
    default: true,
    get: |v| v.reconnect_automatic,
    set: |v, on| v.reconnect_automatic = on,
};

/// Every two-way choice the settings file stores, in edit-tracking order
/// (`Edited::choices` has one flag per entry).
pub(crate) const FIELDS: &[&Field] = &[
    &OPTION_KEY,
    &COMMAND_KEY,
    &CANDIDATES,
    &RETURN_POLICY,
    &super::sidebar_density::SIDEBAR_DENSITY,
];

/// Reads every field of `document` into `values`.
pub(super) fn load(document: &Value, values: &mut Values) {
    for field in FIELDS {
        (field.set)(values, field.read(document));
    }
}

/// Writes every field of `values` into `document`.
pub(super) fn save(document: &mut Map<String, Value>, values: &Values) {
    for field in FIELDS {
        field.write(document, (field.get)(values));
    }
}
