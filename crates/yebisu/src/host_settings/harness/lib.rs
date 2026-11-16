//! Compile the exact isolated production modules before P11 integration.
#![allow(dead_code)]
#[path = "../mod.rs"]
mod host_settings;
#[path = "../../theme_picker/mod.rs"]
mod theme_picker;
