use super::SavedHosts;
use crate::setup::{Reply, Setup};

pub(crate) const STUDIO: &str = "dev@studio.local";
pub(crate) const MINI: &str = "dev@mini.local";

pub(crate) fn named_setup() -> Setup {
    let mut setup = Setup::default();
    setup.apply(Ok(Reply::Hosts(
        SavedHosts {
            targets: vec![STUDIO.into(), MINI.into()],
            selected: STUDIO.into(),
            names: [(STUDIO.into(), "Studio Mac".into())].into(),
        },
        String::new(),
    )));
    setup
}
