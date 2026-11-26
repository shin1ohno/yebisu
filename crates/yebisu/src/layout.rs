//! Window geometry for the SH1-157 layout (L0): a pure function from the
//! window size, the bottom keyboard, the safe area, the window controls and
//! the mod region settings to every rectangle the window shows. No GPUI type,
//! no I/O, no state.
//!
//! Values come from the SH1-157 design text (D1, D2, artboards 10 and 11) and
//! the owner decisions it cites: the terminal never goes below 280×206 (Q1),
//! width shortage takes M2 then the sidebar (Q2), withdrawn mods are listed
//! (Q3), height shortage takes M4 then M3 (q3), a mod with a mismatched version
//! is absent (q1), the find bar sits at the top of the terminal column directly
//! under the tab row with M4 below it (q8, 2026-11-17), and the keyboard and
//! the safe area bottom take the larger only (D1).
//!
//! L0 only computes. No render path calls it until L2a wires it in.
#![cfg_attr(not(test), allow(dead_code))]

mod controls;
mod height;
mod width;

#[cfg(test)]
pub use controls::STATUS_ICONS_REACH;
pub use controls::{ControlsClearance, ControlsReach, top_band};

/// Expanded sidebar width.
pub const SIDEBAR_WIDTH: f32 = 300.0;
/// Collapsed sidebar (rail) width.
pub const RAIL_WIDTH: f32 = 72.0;
/// Hairline drawn between the sidebar and the terminal column, under the tab
/// row and over the status bar (the canvas counts it: 301, 37 and 45).
pub const RULE: f32 = 1.0;
pub const TAB_ROW_HEIGHT: f32 = 36.0;
pub const STATUS_BAR_HEIGHT: f32 = 44.0;
pub const FIND_BAR_HEIGHT: f32 = 44.0;
/// Sidebar header above the list (SplitHalf.dc.html).
pub const SIDEBAR_HEADER_HEIGHT: f32 = 44.0;
/// Leading and trailing padding of the full sidebar's header.
pub const SIDEBAR_HEADER_PADDING: f32 = 12.0;
/// Sidebar footer under the list and M1: rule 1 and one 44 pt Hosts and
/// Settings row (SplitHalf.dc.html:125–127, without its 4 + 4 padding), the
/// status bar's own rule and height, so the two top rules line up (owner
/// decision 2026-11-25).
pub const SIDEBAR_FOOTER_HEIGHT: f32 = RULE + STATUS_BAR_HEIGHT;

/// One pane's minimum, about 28 columns × 8 rows (Q1).
pub const TERMINAL_MIN_WIDTH: f32 = 280.0;
pub const TERMINAL_MIN_HEIGHT: f32 = 206.0;
/// Column width two panes need; narrower columns show one pane (design text
/// artboard 10: "2 ペインには 568 要る").
pub const TWO_PANE_MIN_WIDTH: f32 = 568.0;

pub const M1_MIN_HEIGHT: f32 = 120.0;
pub const M2_RANGE: Range = Range::new(280.0, 320.0, 480.0);
pub const M3_RANGE: Range = Range::new(44.0, 112.0, 320.0);
pub const M4_RANGE: Range = Range::new(28.0, 44.0, 160.0);
pub const M5_SECTION_WIDTH: f32 = 160.0;
pub const M5_MAX_SECTIONS: u8 = 3;
pub const OVERLAY_MARGIN: f32 = 32.0;
pub const DIALOG_WIDTH: f32 = 420.0;
pub const TOAST_SIZE: Size = Size::new(340.0, 72.0);

/// Minimum, default and maximum of one mod region dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

impl Range {
    const fn new(min: f32, default: f32, max: f32) -> Self {
        Self { min, default, max }
    }

    pub fn clamp(self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// True when the two rectangles share any area.
    pub fn overlaps(self, other: Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Insets the window keeps clear: the safe area on each edge, the software
/// keyboard docked at the bottom, and iPadOS 26's window controls.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub safe_top: f32,
    pub safe_left: f32,
    pub safe_right: f32,
    pub safe_bottom: f32,
    pub keyboard: f32,
    /// How far top-bar content must start from each side's safe-area edge to
    /// clear the window controls (herdr-uikit `window_control_insets`). All
    /// zero means no window controls; see `controls.rs`.
    pub window_controls: ControlsReach,
    /// How far top-row content must stay from the side opposite the window
    /// controls to clear the system status bar's icons, in an iPad window
    /// whose top row has moved up into the status bar's band (`top_band`); 0
    /// everywhere else.
    pub status_icons: f32,
}

impl Insets {
    /// The keyboard and the safe area bottom overlap; only the larger is
    /// taken (D1).
    pub fn bottom(self) -> f32 {
        self.keyboard.max(self.safe_bottom).max(0.0)
    }
}

/// A sized mod region (M2, M3, M4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ModSlot {
    /// No mod: size 0, reserves nothing.
    #[default]
    Empty,
    /// The mod runner and yebisu disagree on the version: treated as absent
    /// and not listed as withdrawn (q1).
    VersionMismatch,
    /// The size the mod asks for, clamped to the region's range.
    Wants(f32),
}

/// A mod region whose size the layout decides (M1 takes the rest of the
/// sidebar, M6 is the pane itself).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ModPresence {
    #[default]
    Empty,
    VersionMismatch,
    Present,
}

/// The mod region settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Regions {
    pub m1: ModPresence,
    pub m2: ModSlot,
    pub m3: ModSlot,
    pub m4: ModSlot,
    /// Status bar sections of version-matched mods.
    pub m5_sections: u8,
    pub m6: ModPresence,
}

impl Regions {
    /// M1 to M4 at their defaults (artboard 11).
    pub fn defaults() -> Self {
        Self {
            m1: ModPresence::Present,
            m2: ModSlot::Wants(M2_RANGE.default),
            m3: ModSlot::Wants(M3_RANGE.default),
            m4: ModSlot::Wants(M4_RANGE.default),
            m5_sections: 0,
            m6: ModPresence::Empty,
        }
    }

    /// Every region at its maximum (artboard 10).
    pub fn maximum() -> Self {
        Self {
            m1: ModPresence::Present,
            m2: ModSlot::Wants(M2_RANGE.max),
            m3: ModSlot::Wants(M3_RANGE.max),
            m4: ModSlot::Wants(M4_RANGE.max),
            m5_sections: M5_MAX_SECTIONS,
            m6: ModPresence::Present,
        }
    }
}

/// Everything the layout depends on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub window: Size,
    pub insets: Insets,
    pub regions: Regions,
    /// Height of the sidebar list's content (spaces and agents).
    pub sidebar_list_height: f32,
    /// Width the status bar keeps for its fixed items (connection name,
    /// Mods, Shortcuts); M5 sections share the rest.
    pub status_reserved_width: f32,
    pub find_bar_open: bool,
    /// Simulator QA strip: 0 in product builds, its measured height otherwise.
    pub qa_strip_height: f32,
    /// Panes in the current tab.
    pub pane_count: usize,
    /// The user collapsed the sidebar to the rail (Command-B, L4): the rail
    /// takes 72 pt at any width, and M2 stays when it fits beside it.
    pub sidebar_rail: bool,
}

/// A mod region that can be withdrawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Region {
    M1,
    M2,
    M3,
    M4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneMode {
    Single,
    Split,
}

/// Two panes need [`TWO_PANE_MIN_WIDTH`]; a narrower column shows one pane
/// even when the tab has more.
pub fn pane_mode(column_width: f32, pane_count: usize) -> PaneMode {
    if pane_count >= 2 && column_width >= TWO_PANE_MIN_WIDTH {
        PaneMode::Split
    } else {
        PaneMode::Single
    }
}

const RAIL_TERMINAL_MIN_WIDTH: f32 = RAIL_WIDTH + TERMINAL_MIN_WIDTH;

/// The rail has first claim on 72 pt beside the 280 pt terminal minimum.
pub(crate) fn compact_window(width: f32) -> bool {
    width < RAIL_TERMINAL_MIN_WIDTH
}

/// The server owns Herdr 1 splits. Herdr 2 can give the focused pane the
/// whole client viewport while the keyboard is shown or in compact mode.
pub(crate) fn herdr2_pane_mode(column_width: f32, pane_count: usize, focus_only: bool) -> PaneMode {
    if focus_only {
        PaneMode::Single
    } else {
        pane_mode(column_width, pane_count)
    }
}

/// The window cannot hold one pane at its minimum: the app says so instead of
/// shrinking the terminal (D2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooSmall {
    /// Space left for the terminal after every region yielded.
    pub available: Size,
    pub minimum: Size,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TerminalFit {
    Fits(Rect),
    TooSmall(TooSmall),
}

impl TerminalFit {
    pub fn rect(self) -> Option<Rect> {
        match self {
            Self::Fits(rect) => Some(rect),
            Self::TooSmall(_) => None,
        }
    }
}

/// M5 sections that fit the status bar; the rest move to the details.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StatusSections {
    pub shown: u8,
    pub moved_to_details: u8,
    pub width: f32,
}

/// Overlay (O) limits: the widths shrink to the window width − 32 and the
/// content scrolls when the height runs out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overlay {
    pub dialog_width: f32,
    pub toast: Size,
    pub max_height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub sidebar: Rect,
    pub sidebar_collapsed: bool,
    /// What the sidebar keeps clear of iPadOS 26's window controls.
    pub controls: ControlsClearance,
    pub tab_row: Rect,
    /// Top of the terminal column under the tab row; height 0 when closed.
    pub find_bar: Rect,
    pub status_bar: Rect,
    /// Full-width key band between the status bar and the docked keyboard.
    pub(crate) key_band: Option<Rect>,
    pub m1: Option<Rect>,
    pub m2: Option<Rect>,
    pub m3: Option<Rect>,
    pub m4: Option<Rect>,
    pub m5: StatusSections,
    pub m6: Option<Rect>,
    pub overlay: Overlay,
    /// Bottom of the terminal column, above the status bar, outside the
    /// terminal rect.
    pub qa_strip: Rect,
    pub terminal: TerminalFit,
    /// Requested, version-matched regions that did not fit (Q3), in region
    /// order.
    pub withdrawn: Vec<Region>,
    pub pane_mode: PaneMode,
}

impl Layout {
    /// Size of a mod region; an absent or withdrawn one is 0×0.
    pub fn region_size(&self, region: Region) -> Size {
        let rect = match region {
            Region::M1 => self.m1,
            Region::M2 => self.m2,
            Region::M3 => self.m3,
            Region::M4 => self.m4,
        };
        rect.map(Rect::size).unwrap_or_default()
    }
}

/// Lays out the window.
pub fn compute(input: &Input) -> Layout {
    compute_with_key_band(input, 0.0)
}

pub(crate) fn usable_width(width: f32, safe_left: f32, safe_right: f32) -> f32 {
    (width - safe_left.max(0.0) - safe_right.max(0.0)).max(0.0)
}

pub(crate) fn compute_with_key_band(input: &Input, band_height: f32) -> Layout {
    let left = input.insets.safe_left.max(0.0);
    let top = input.insets.safe_top.max(0.0);
    let keyboard_top = (input.window.height - input.insets.bottom()).max(0.0);
    let band_height = band_height.min((keyboard_top - top).max(0.0));
    let band_top = keyboard_top - band_height;
    let key_band =
        (band_height > 0.0).then(|| Rect::new(0.0, band_top, input.window.width, band_height));
    let usable = Size::new(
        usable_width(
            input.window.width,
            input.insets.safe_left,
            input.insets.safe_right,
        ),
        (band_top - top).max(0.0),
    );
    let mut withdrawn = Vec::new();
    let columns = width::allocate(
        usable.width,
        input.regions.m2,
        input.sidebar_rail,
        &mut withdrawn,
    );
    let column_x = left + columns.sidebar + RULE;
    let chrome_width = columns.column + columns.dock;

    let tab_row = Rect::new(column_x, top, chrome_width, TAB_ROW_HEIGHT);
    let status_height = STATUS_BAR_HEIGHT.min(usable.height);
    let status_bar = Rect::new(
        column_x,
        top + usable.height - status_height,
        chrome_width,
        status_height,
    );
    let content_top = top + TAB_ROW_HEIGHT + RULE;
    let content_height =
        (usable.height - TAB_ROW_HEIGHT - RULE - STATUS_BAR_HEIGHT - RULE).max(0.0);
    let column = Rect::new(column_x, content_top, columns.column, content_height);
    let bands = height::allocate(column, input, &mut withdrawn);

    let sidebar = Rect::new(left, top, columns.sidebar, usable.height);
    let m1 = sidebar_bottom(sidebar, columns.collapsed, input, &mut withdrawn);
    let m2 = (columns.dock > 0.0).then(|| {
        Rect::new(
            column_x + columns.column,
            content_top,
            columns.dock,
            content_height,
        )
    });
    let terminal = if !columns.too_narrow && height::fits_height(bands.terminal.height) {
        TerminalFit::Fits(bands.terminal)
    } else {
        TerminalFit::TooSmall(TooSmall {
            available: Size::new(columns.column, bands.terminal.height.max(0.0)),
            minimum: Size::new(TERMINAL_MIN_WIDTH, TERMINAL_MIN_HEIGHT),
        })
    };
    let m6 = match input.regions.m6 {
        ModPresence::Present => terminal.rect(),
        ModPresence::Empty | ModPresence::VersionMismatch => None,
    };
    withdrawn.sort();
    Layout {
        sidebar,
        sidebar_collapsed: columns.collapsed,
        controls: controls::clearance(&input.insets),
        tab_row,
        find_bar: bands.find_bar,
        status_bar,
        key_band,
        m1,
        m2,
        m3: bands.m3,
        m4: bands.m4,
        m5: status_sections(chrome_width, input),
        m6,
        overlay: overlay(usable),
        qa_strip: bands.qa_strip,
        terminal,
        withdrawn,
        pane_mode: pane_mode(columns.column, input.pane_count),
    }
}

/// M1 takes what the list leaves in the expanded sidebar, and needs 120.
fn sidebar_bottom(
    sidebar: Rect,
    collapsed: bool,
    input: &Input,
    withdrawn: &mut Vec<Region>,
) -> Option<Rect> {
    if input.regions.m1 != ModPresence::Present {
        return None;
    }
    let list_top = sidebar.y + SIDEBAR_HEADER_HEIGHT;
    let list = input.sidebar_list_height.max(0.0);
    let rest = sidebar.height - SIDEBAR_HEADER_HEIGHT - SIDEBAR_FOOTER_HEIGHT - list;
    if collapsed || rest < M1_MIN_HEIGHT {
        withdrawn.push(Region::M1);
        return None;
    }
    Some(Rect::new(sidebar.x, list_top + list, sidebar.width, rest))
}

fn status_sections(status_width: f32, input: &Input) -> StatusSections {
    let requested = input.regions.m5_sections.min(M5_MAX_SECTIONS);
    let room = (status_width - input.status_reserved_width).max(0.0);
    // Bounded by M5_MAX_SECTIONS before the cast, so it cannot truncate.
    let fitting = (room / M5_SECTION_WIDTH)
        .floor()
        .min(f32::from(M5_MAX_SECTIONS)) as u8;
    let shown = requested.min(fitting);
    StatusSections {
        shown,
        moved_to_details: requested - shown,
        width: f32::from(shown) * M5_SECTION_WIDTH,
    }
}

fn overlay(usable: Size) -> Overlay {
    let widest = (usable.width - OVERLAY_MARGIN).max(0.0);
    Overlay {
        dialog_width: DIALOG_WIDTH.min(widest),
        toast: Size::new(TOAST_SIZE.width.min(widest), TOAST_SIZE.height),
        max_height: (usable.height - OVERLAY_MARGIN).max(0.0),
    }
}

#[cfg(test)]
mod tests;
