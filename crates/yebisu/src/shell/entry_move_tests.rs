//! L2b tests: the legacy bands are retired and nothing lives there.
use super::*;
use crate::shell::{ChromeState, HerdrKind, Place, entry_points};

const LEGACY_PLACES: [Place; 5] = [
    Place::Toolbar,
    Place::InputBand,
    Place::ReturnBand,
    Place::HandoffBand,
    Place::ActionsBand,
];

/// Plan v2 7.6: no toolbar rows, Input options, Return, handoff or tab
/// band, and the QA bands moved to the QA strip.
#[test]
fn legacy_bands_are_retired() {
    let bands = LegacyBands::current();
    assert!(!bands.toolbar && !bands.input_options && !bands.return_policy);
    assert!(!bands.handoff && !bands.actions && !bands.qa_controls);
    for kind in [HerdrKind::Herdr1, HerdrKind::Herdr2] {
        for connected in [true, false] {
            let state = ChromeState {
                kind,
                connected,
                ..ChromeState::default()
            };
            let points = entry_points::chrome_entry_points(&state, bands);
            assert!(
                !points.iter().any(|p| LEGACY_PLACES.contains(&p.place)),
                "{kind:?} connected={connected}: a legacy place is left"
            );
            let before = entry_points::chrome_entry_points(&state, LegacyBands::before_l2b());
            assert!(before.iter().any(|p| p.place == Place::Toolbar));
        }
    }
}
