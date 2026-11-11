//! P10 compile spike of the sheet's five prerequisite groups. Never installed
//! by app::run: host tests alone mount it. Its render paths compile on both iOS
//! targets, so unused UI dependencies cannot be hidden behind cfg(test).
#![allow(dead_code, reason = "P10 unmounted settings sheet compile spike")]
mod appearance;
mod draft;
mod editor;
mod marks;
mod metrics;
mod motion;
mod number;
mod sheet;
mod strings;
#[cfg(test)]
mod tests;
mod ui;
