//! L4: the rail's two badges (q3). Owner: L4.
//!
//! A rail button is 64 pt wide. The badges sit in it with an 8 pt inset on
//! each side: Error top left, Needs you top right, the status dot between
//! them (plan v2 7.9; `Rail.dc.html:41`). With three 16 pt wide parts the
//! gaps are exactly 2 pt (left badge x 8 to 24, dot x 26 to 38, right badge
//! x 40 to 56). A count that renders wider than 16 pt does not fit; the rail
//! then shows one combined number at the top right instead.
//!
//! Everything here is pure: the caller passes the text measure, so the rule is
//! tested without a window.

use super::style::metric;

/// Width of a rail button.
pub(crate) const BUTTON_WIDTH: f32 = 64.0;
/// Space between the button edge and a badge.
pub(crate) const EDGE_INSET: f32 = metric::SMALL_INSET;
/// The badge's `min-width` and its height in the canvas.
pub(crate) const BADGE_MIN_WIDTH: f32 = 16.0;
pub(crate) const BADGE_HEIGHT: f32 = 16.0;
/// The status dot between the badges.
pub(crate) const DOT_SIZE: f32 = 12.0;
/// The least space between a badge and the dot.
pub(crate) const MIN_GAP: f32 = metric::SEGMENT_INSET;
/// Badge digits are 11 pt bold (D10: nothing in the rail is smaller).
pub(crate) const BADGE_TEXT_PT: f32 = super::style::text::SMALL;
/// A count above this is written `99+`.
pub(crate) const COUNT_CAP: usize = 99;

/// How the two counts are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RailBadges {
    /// Error top left, Needs you top right.
    Two,
    /// One number, Needs you plus Error, top right.
    Combined,
}

/// The badge texts for one rail button; `None` draws nothing there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BadgeTexts {
    /// Top left: the Error count.
    pub(crate) left: Option<String>,
    /// Top right: the Needs you count, or the sum when combined.
    pub(crate) right: Option<String>,
}

/// The text of one badge: the count, or `99+` above the cap.
pub(crate) fn badge_label(count: usize) -> String {
    if count > COUNT_CAP {
        format!("{COUNT_CAP}+")
    } else {
        count.to_string()
    }
}

/// Where the status dot starts and ends inside the button.
pub(crate) fn dot_span() -> (f32, f32) {
    let left = (BUTTON_WIDTH - DOT_SIZE) / 2.0;
    (left, left + DOT_SIZE)
}

/// Rendered width of one badge: its text, never below the 16 pt minimum.
pub(crate) fn badge_width(label: &str, measure: &dyn Fn(&str) -> f32) -> f32 {
    measure(label).max(BADGE_MIN_WIDTH)
}

/// Decides how a button draws its counts. `measure` returns the rendered
/// width of a badge label in points (bold, 11 pt, at the user's text scale).
///
/// Two badges are used when both fit with at least [`MIN_GAP`] to the dot;
/// with at most one count there is nothing to combine.
pub(crate) fn rail_badges(needs: usize, error: usize, measure: &dyn Fn(&str) -> f32) -> RailBadges {
    if needs == 0 || error == 0 {
        return RailBadges::Two;
    }
    let (dot_left, dot_right) = dot_span();
    let left_right_edge = EDGE_INSET + badge_width(&badge_label(error), measure);
    let right_left_edge = BUTTON_WIDTH - EDGE_INSET - badge_width(&badge_label(needs), measure);
    let left_fits = left_right_edge + MIN_GAP <= dot_left;
    let right_fits = dot_right + MIN_GAP <= right_left_edge;
    if left_fits && right_fits {
        RailBadges::Two
    } else {
        RailBadges::Combined
    }
}

/// The texts to draw for the counts in the chosen mode. Error has no badge
/// without errors, and Needs you none without waiting agents.
pub(crate) fn badge_texts(needs: usize, error: usize, mode: RailBadges) -> BadgeTexts {
    match mode {
        RailBadges::Two => BadgeTexts {
            left: (error > 0).then(|| badge_label(error)),
            right: (needs > 0).then(|| badge_label(needs)),
        },
        RailBadges::Combined => BadgeTexts {
            left: None,
            right: (needs + error > 0).then(|| badge_label(needs + error)),
        },
    }
}

/// The horizontal extent (start, end) of each drawn badge inside the button.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BadgeSpans {
    pub(crate) left: Option<(f32, f32)>,
    pub(crate) right: Option<(f32, f32)>,
}

/// Where the badges of `texts` are drawn. The renderer places them from
/// these spans, so the fit check and the picture share one set of numbers.
pub(crate) fn badge_spans(texts: &BadgeTexts, measure: &dyn Fn(&str) -> f32) -> BadgeSpans {
    BadgeSpans {
        left: texts
            .left
            .as_deref()
            .map(|label| (EDGE_INSET, EDGE_INSET + badge_width(label, measure))),
        right: texts.right.as_deref().map(|label| {
            let end = BUTTON_WIDTH - EDGE_INSET;
            (end - badge_width(label, measure), end)
        }),
    }
}

/// The spoken part for the counts. It always carries the exact counts, one
/// phrase per kind, so a combined badge still tells Needs you from Error and
/// a count above the cap is not shortened.
pub(crate) fn badge_accessibility(needs: usize, error: usize) -> Option<String> {
    let mut parts = Vec::new();
    if needs > 0 {
        parts.push(match needs {
            1 => "1 agent waiting".to_owned(),
            n => format!("{n} agents waiting"),
        });
    }
    if error > 0 {
        parts.push(match error {
            1 => "1 agent in error".to_owned(),
            n => format!("{n} agents in error"),
        });
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

#[cfg(test)]
#[path = "rail_badges_tests.rs"]
mod tests;
