//! Vertical allocation in the terminal column, top to bottom: the find bar
//! directly under the tab row (q8), M4, the terminal, M3, and the QA strip
//! above the status bar.
use super::{
    FIND_BAR_HEIGHT, Input, M3_RANGE, M4_RANGE, ModSlot, Range, Rect, Region, TERMINAL_MIN_HEIGHT,
};

pub(super) struct Bands {
    pub find_bar: Rect,
    pub m4: Option<Rect>,
    pub m3: Option<Rect>,
    pub qa_strip: Rect,
    /// What is left for the terminal; may be below the minimum.
    pub terminal: Rect,
}

#[derive(Clone, Copy)]
enum Band {
    /// M4, withdrawn whole.
    Top,
    /// M3, takes the rest down to its minimum, then withdrawn.
    Bottom,
}

/// A height shortage takes the bands in this order (q3).
const YIELD_ORDER: [Band; 2] = [Band::Top, Band::Bottom];

pub(super) fn fits_height(height: f32) -> bool {
    height >= TERMINAL_MIN_HEIGHT
}

fn requested(slot: ModSlot, range: Range) -> f32 {
    match slot {
        ModSlot::Wants(height) => range.clamp(height),
        ModSlot::Empty | ModSlot::VersionMismatch => 0.0,
    }
}

pub(super) fn allocate(column: Rect, input: &Input, withdrawn: &mut Vec<Region>) -> Bands {
    let find = if input.find_bar_open {
        FIND_BAR_HEIGHT.min(column.height)
    } else {
        0.0
    };
    // The QA strip comes off before the terminal and the bands share the rest.
    let qa = input.qa_strip_height.max(0.0).min(column.height - find);
    let room = column.height - find - qa;
    let mut top = requested(input.regions.m4, M4_RANGE);
    let mut bottom = requested(input.regions.m3, M3_RANGE);
    for band in YIELD_ORDER {
        if fits_height(room - top - bottom) {
            break;
        }
        match band {
            Band::Top if top > 0.0 => {
                top = 0.0;
                withdrawn.push(Region::M4);
            }
            Band::Bottom if bottom > 0.0 => {
                let rest = room - top - TERMINAL_MIN_HEIGHT;
                if rest >= M3_RANGE.min {
                    bottom = bottom.min(rest);
                } else {
                    bottom = 0.0;
                    withdrawn.push(Region::M3);
                }
            }
            Band::Top | Band::Bottom => {}
        }
    }
    let find_bar = Rect::new(column.x, column.y, column.width, find);
    let m4 = Rect::new(column.x, find_bar.bottom(), column.width, top);
    let terminal = Rect::new(column.x, m4.bottom(), column.width, room - top - bottom);
    let m3 = Rect::new(column.x, terminal.bottom(), column.width, bottom);
    Bands {
        find_bar,
        m4: (top > 0.0).then_some(m4),
        m3: (bottom > 0.0).then_some(m3),
        qa_strip: Rect::new(column.x, m3.bottom(), column.width, qa),
        terminal,
    }
}
