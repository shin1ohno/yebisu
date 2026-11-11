//! Fixed consumer appearance replacing Config/Theme/font-picker dependencies.
use gpui::{Font, font};

// Source: S/config.rs:170-171; S/config/fonts.rs:15.
pub(super) mod corners {
    pub const CONTROL: f32 = 8.;
    pub const SMALL: f32 = 4.;
}
pub(super) const FONT_SIZE_RANGE: std::ops::RangeInclusive<f32> = 8.0..=48.0;
#[derive(Clone)]
pub(super) struct Theme {
    pub background: u32,
    pub foreground: u32,
    pub active: u32,
    pub surface: u32,
    pub muted: u32,
    primary: u32,
}
impl Default for Theme {
    fn default() -> Self {
        let palette = crate::theme::Theme::default();
        Self {
            background: palette.background,
            foreground: palette.foreground,
            active: palette.muted,
            surface: palette.palette[0],
            muted: palette.muted,
            primary: palette.primary,
        }
    }
}
impl Theme {
    pub fn primary(&self) -> u32 {
        self.primary
    }
    pub fn subtext(&self) -> u32 {
        self.muted
    }
    // Fixed palette: user-selected contrast remains outside the P10 closure.
    pub fn ink(&self, color: u32) -> u32 {
        color
    }
}

pub(super) struct Appearance {
    pub theme: Theme,
    pub font: Font,
    pub size: f32,
    pub line_height: f32,
}
impl Appearance {
    pub fn for_density(compact: bool) -> Self {
        // Same source-defined presets as terminal_view::terminal_paint_callback.
        let (size, line_height) = if compact { (12., 17.) } else { (14., 20.) };
        Self {
            theme: Theme::default(),
            font: font("SauceCodePro Nerd Font Mono"),
            size,
            line_height,
        }
    }
    pub fn specimen(&self) -> gpui::Div {
        use gpui::{prelude::*, *};
        div()
            .debug_selector(|| "font-specimen".into())
            .font(self.font.clone())
            .text_size(px(self.size))
            .line_height(px(self.line_height))
            .bg(rgb(self.theme.background))
            .text_color(rgb(self.theme.foreground))
            .child("Aa 0123 日本語 ┌─┐")
    }
}
