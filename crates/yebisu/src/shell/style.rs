//! The canvas's visual values, in one place (design fidelity plan, Wave 0a).
//! Owner: the controller; Wave 1 streams read it and send a patch to change
//! it (docs/layout-contracts.md section 9).
//!
//! The reference is the canvas "yebisu iPad layout" (owner decision
//! 2026-11-20). Each value cites the artboard line it comes from (the files
//! under `yebisu-design-reference/project/`); `style_tests.rs` holds the
//! canvas text next to each constant. `{{accent}}` in the artboards is
//! [`color::ACCENT`].
//!
//! The UI typeface stays the iPadOS system font (SF, owner decision
//! 2026-11-20): only sizes and weights follow the canvas, whose Inter is not
//! bundled. The root draws text at [`text::BASE`] (`app.rs`), so a view that
//! sets no size gets the canvas body size instead of GPUI's 16.
//!
//! Colours are `0xRRGGBB` for `gpui::rgb`; translucent ones are [`Tint`].
//! Lengths are points. Region sizes (sidebar 300, rail 72, tab row 36, status
//! bar 44, sidebar header 44) stay in `crate::layout`, and sidebar row heights
//! (host 64, space 56, agent 44) in `shell::sidebar`; they are not repeated.
use gpui::{BoxShadow, Hsla, Rgba, point, px};

/// Colours, `0xRRGGBB`.
pub(crate) mod color {
    /// The page and the terminal; the text on the Needs you badge
    /// (Main.dc.html:22, :63).
    pub(crate) const BACKGROUND: u32 = 0x101419;
    /// The sidebar, rail, tab row, status bar and sheets (Main.dc.html:24,
    /// :130, :180; Settings.dc.html:24).
    pub(crate) const SURFACE: u32 = 0x1c1c22;
    /// The 1 pt lines between regions, and raised surfaces: menus, the edit
    /// bar, the Shortcuts popover, the current rail item and the selected
    /// segment (Main.dc.html:24; Herdr2.dc.html:123; Selection.dc.html:84;
    /// Rail.dc.html:30; Settings.dc.html:57).
    pub(crate) const RAISED: u32 = 0x2b2933;
    /// Lines inside a raised surface: menu separators and edit bar dividers
    /// (Herdr2.dc.html:127; Selection.dc.html:86).
    pub(crate) const SEPARATOR: u32 = 0x3b3945;
    /// Secondary text, section headings and icon strokes (Main.dc.html:36,
    /// :42, :27).
    pub(crate) const MUTED: u32 = 0xacb2bb;
    /// Primary text (Main.dc.html:22).
    pub(crate) const TEXT: u32 = 0xd8dee9;
    /// `{{accent}}`: the current space's line, the selected tab's underline,
    /// the focused pane's border and name, links; also the Idle dot
    /// (Main.dc.html:14, :49, :132, :161; SidebarSpec.dc.html:38).
    pub(crate) const ACCENT: u32 = 0x88c0d0;
    /// Done, and a connected host (SidebarSpec.dc.html:38; Main.dc.html:39).
    pub(crate) const GREEN: u32 = 0xa3be8c;
    /// Working, the Needs you badge, and a connecting host's ring
    /// (SidebarSpec.dc.html:38; Main.dc.html:63; SidebarMixed.dc.html:63).
    pub(crate) const YELLOW: u32 = 0xebcb8b;
    /// Needs you and Error dots, the Error tag and the Needs you status word
    /// (SidebarSpec.dc.html:38; ModsMax.dc.html:82).
    pub(crate) const RED: u32 = 0xbf616a;
    /// No agent and no status, an unfocused pane's border, the Mods count
    /// badge (SidebarSpec.dc.html:38; Herdr2.dc.html:96; ModsMax.dc.html:192).
    pub(crate) const SLATE: u32 = 0x4c566a;
    /// A disabled menu item's text (Herdr2.dc.html:129).
    pub(crate) const DISABLED: u32 = 0x6c7480;
}

/// A colour with the canvas's alpha, as its `rgba(r, g, b, a)` writes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tint {
    pub(crate) rgb: u32,
    pub(crate) alpha: f32,
}

impl From<Tint> for Rgba {
    fn from(tint: Tint) -> Self {
        let channel = |shift: u32| ((tint.rgb >> shift) & 0xff) as f32 / 255.;
        Rgba {
            r: channel(16),
            g: channel(8),
            b: channel(0),
            a: tint.alpha,
        }
    }
}

impl From<Tint> for Hsla {
    fn from(tint: Tint) -> Self {
        Rgba::from(tint).into()
    }
}

/// Translucent fills.
pub(crate) mod tint {
    use super::{Tint, color};

    /// The selected tab, the selected Settings item and a pressed Shortcuts
    /// row: `rgba(136,192,208,0.22)` (Main.dc.html:132; Settings.dc.html:38).
    pub(crate) const SELECTED: Tint = Tint {
        rgb: color::ACCENT,
        alpha: 0.22,
    };
    /// The current space row: `rgba(136,192,208,0.15)` (Main.dc.html:49).
    pub(crate) const CURRENT_ROW: Tint = Tint {
        rgb: color::ACCENT,
        alpha: 0.15,
    };
    /// Selected terminal text: `rgba(136,192,208,0.38)` (Selection.dc.html:79).
    pub(crate) const SELECTION: Tint = Tint {
        rgb: color::ACCENT,
        alpha: 0.38,
    };
    /// Behind a sheet: `rgba(16,20,25,0.66)` (Settings.dc.html:22;
    /// AddHost.dc.html:27).
    pub(crate) const SHEET_SCRIM: Tint = Tint {
        rgb: color::BACKGROUND,
        alpha: 0.66,
    };
    /// Behind the Shortcuts popover, above the status bar:
    /// `rgba(16,20,25,0.45)` (Shortcuts.dc.html:20).
    pub(crate) const POPOVER_SCRIM: Tint = Tint {
        rgb: color::BACKGROUND,
        alpha: 0.45,
    };
}

/// A black drop shadow straight down: `0 {y}px {blur}px rgba(0,0,0,{alpha})`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shadow {
    pub(crate) y: f32,
    pub(crate) blur: f32,
    pub(crate) alpha: f32,
}

impl Shadow {
    /// The GPUI shadow: no horizontal offset and no spread, as the canvas.
    pub(crate) fn box_shadow(self) -> BoxShadow {
        BoxShadow {
            color: Rgba {
                r: 0.,
                g: 0.,
                b: 0.,
                a: self.alpha,
            }
            .into(),
            offset: point(px(0.), px(self.y)),
            blur_radius: px(self.blur),
            spread_radius: px(0.),
            inset: false,
        }
    }
}

/// Drop shadows.
pub(crate) mod shadow {
    use super::Shadow;

    /// Sheets (Settings, Add host): `0 20px 60px rgba(0,0,0,0.55)`
    /// (Settings.dc.html:24; AddHost.dc.html:29).
    pub(crate) const SHEET: Shadow = Shadow {
        y: 20.,
        blur: 60.,
        alpha: 0.55,
    };
    /// The ... menu and the Shortcuts popover: `0 12px 36px rgba(0,0,0,0.5)`
    /// (Herdr2.dc.html:123; Shortcuts.dc.html:22).
    pub(crate) const POPOVER: Shadow = Shadow {
        y: 12.,
        blur: 36.,
        alpha: 0.5,
    };
    /// The edit bar: `0 6px 18px rgba(0,0,0,0.45)` (Selection.dc.html:84).
    pub(crate) const EDIT_BAR: Shadow = Shadow {
        y: 6.,
        blur: 18.,
        alpha: 0.45,
    };
}

/// Corner radii.
pub(crate) mod radius {
    /// Sheets (Settings.dc.html:24; AddHost.dc.html:29).
    pub(crate) const SHEET: f32 = 18.;
    /// The ... menu and the Shortcuts popover (Herdr2.dc.html:123;
    /// Shortcuts.dc.html:22).
    pub(crate) const POPOVER: f32 = 14.;
    /// A box inside a sheet (AddHost.dc.html:64).
    pub(crate) const CARD: f32 = 12.;
    /// The edit bar (Selection.dc.html:84).
    pub(crate) const EDIT_BAR: f32 = 10.;
    /// Buttons, Settings items, segmented controls and Shortcuts rows
    /// (Main.dc.html:26, :115; Settings.dc.html:34, :56; Shortcuts.dc.html:39).
    pub(crate) const CONTROL: f32 = 8.;
    /// Rail items and a segment's selected option (Rail.dc.html:26;
    /// Settings.dc.html:57).
    pub(crate) const RAIL_ITEM: f32 = 6.;
    /// The Needs you badge and the Error tag (Main.dc.html:63;
    /// ModsMax.dc.html:82).
    pub(crate) const BADGE: f32 = 4.;
}

/// Text sizes in points. The canvas body is 13; GPUI's default is 16.
pub(crate) mod text {
    /// Body text and the status bar; the root size (Main.dc.html:22, :180).
    pub(crate) const BASE: f32 = 13.;
    /// Host, space and app names (Main.dc.html:29, :41, :52).
    pub(crate) const NAME: f32 = 14.;
    /// Second lines, status words and the sheet's save state
    /// (Main.dc.html:42, :55, :94; Settings.dc.html:26).
    pub(crate) const SECONDARY: f32 = 12.;
    /// Section headings (HOSTS, AGENTS, Settings groups), tab counts, badges
    /// and rail labels (Main.dc.html:36, :53, :63; Rail.dc.html:28).
    pub(crate) const SMALL: f32 = 11.;
    /// Sheet and popover titles (Settings.dc.html:27; AddHost.dc.html:32;
    /// Shortcuts.dc.html:24).
    pub(crate) const SHEET_TITLE: f32 = 15.;
    /// Page titles in a sheet, on a [`PAGE_TITLE_LINE`] line
    /// (Settings.dc.html:48).
    pub(crate) const PAGE_TITLE: f32 = 18.;
    pub(crate) const PAGE_TITLE_LINE: f32 = 26.;
    /// The two lines of an AGENTS row (Main.dc.html:93).
    pub(crate) const AGENT_LINE: f32 = 16.;
    /// Section heading tracking in em (Main.dc.html:36). GPUI 0.3.12's text
    /// style has no letter spacing, so headings are drawn without it.
    pub(crate) const HEADING_TRACKING_EM: f32 = 0.06;
}

/// Text weights.
pub(crate) mod weight {
    use gpui::FontWeight;

    /// Body text; what unset text draws, for a view that resets bold text.
    pub(crate) const REGULAR: FontWeight = FontWeight::NORMAL;
    /// "Needs you first" and the edit bar's buttons (Main.dc.html:89;
    /// Selection.dc.html:85).
    pub(crate) const MEDIUM: FontWeight = FontWeight::MEDIUM;
    /// Section headings, the app name, the selected tab, sheet and page
    /// titles, and Done (Main.dc.html:29, :36, :135; Settings.dc.html:27,
    /// :28, :48).
    pub(crate) const SEMIBOLD: FontWeight = FontWeight::SEMIBOLD;
    /// Host names, the current space's name, the Needs you badge and the
    /// Needs you status word (Main.dc.html:41, :52, :63, :94).
    pub(crate) const BOLD: FontWeight = FontWeight::BOLD;
}

/// Lengths the artboards repeat.
pub(crate) mod metric {
    /// Every tappable control's side (Main.dc.html:26, :44, :115, :122).
    pub(crate) const TOUCH: f32 = 44.;
    /// Status dots: sidebar rows (Main.dc.html:39, :92).
    pub(crate) const DOT: f32 = 10.;
    /// Status dots: tabs and the status bar (Main.dc.html:134, :181).
    pub(crate) const SMALL_DOT: f32 = 8.;
    /// A connecting host's ring width (SidebarMixed.dc.html:63).
    pub(crate) const RING: f32 = 2.;
    /// Section heading rows (Main.dc.html:36, :88).
    pub(crate) const HEADING_ROW: f32 = 32.;
    /// Space above the AGENTS heading (Main.dc.html:88).
    pub(crate) const HEADING_GAP: f32 = 12.;
    /// Side inset of headings and AGENTS rows (Main.dc.html:36, :91).
    pub(crate) const SIDE: f32 = 16.;
    /// Gap between a dot and its text in host and AGENTS rows
    /// (Main.dc.html:38, :91).
    pub(crate) const ROW_GAP: f32 = 10.;
    /// Gap between items on one line: space rows, tabs, the pane gap
    /// (Main.dc.html:50, :132, :160).
    pub(crate) const INLINE_GAP: f32 = 8.;
    /// Left inset of a space row; a current row is [`CURRENT_LINE`] less and
    /// draws an accent line that wide (Main.dc.html:49, :58).
    pub(crate) const SPACE_INDENT: f32 = 40.;
    pub(crate) const CURRENT_LINE: f32 = 2.;
    /// A space row's second line, under the name (Main.dc.html:55).
    pub(crate) const DETAIL_INDENT: f32 = 18.;
    /// Pane borders and every hairline (Main.dc.html:24, :161).
    pub(crate) const LINE: f32 = 1.;
    /// The pane name sits this far above the border's top and in from its
    /// left (Main.dc.html:161: `top: -10px; left: 10px`).
    pub(crate) const PANE_NAME_INSET: f32 = 10.;
    /// The ... menu's width, item height and top and bottom padding
    /// (Herdr2.dc.html:123, :129).
    pub(crate) const MENU_WIDTH: f32 = 260.;
    pub(crate) const MENU_ITEM: f32 = 48.;
    pub(crate) const MENU_PADDING: f32 = 6.;
    /// A sheet's header (Settings.dc.html:25).
    pub(crate) const SHEET_HEADER: f32 = 60.;
    /// Settings: the sections list's width and the height of its items and
    /// of the rows on a page (Settings.dc.html:32, :34, :54).
    pub(crate) const SETTINGS_LIST: f32 = 240.;
    pub(crate) const SETTINGS_ROW: f32 = 56.;
    /// A segmented control's option height and the inset around them
    /// (Settings.dc.html:56-57).
    pub(crate) const SEGMENT: f32 = 40.;
    pub(crate) const SEGMENT_INSET: f32 = 2.;
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
