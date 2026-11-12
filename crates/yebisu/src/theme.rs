//! Minimal terminal palette; settings and theme selection arrive in P15.
pub(crate) use crate::font_weight::bold as bold_font_weight;
#[derive(Clone, PartialEq)]
pub(crate) struct Theme {
    pub background: u32,
    pub bold: Option<u32>,
    pub cursor: u32,
    pub foreground: u32,
    pub muted: u32,
    pub primary: u32,
    pub palette: [u32; 256],
}
impl Theme {
    pub(crate) fn from_palette(palette: crate::theme_picker::Palette) -> Self {
        let mut theme = Self::default();
        if let Some(colors) = palette.colors() {
            theme.background = colors.background;
            theme.foreground = colors.foreground;
            theme.bold = Some(colors.foreground);
            theme.cursor = colors.foreground;
            theme.muted = colors.muted;
            theme.primary = colors.primary;
        }
        theme
    }

    pub fn primary(&self) -> u32 {
        self.primary
    }
}
impl Default for Theme {
    fn default() -> Self {
        let mut palette = [0; 256];
        palette[..16].copy_from_slice(&[
            0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x88c0d0, 0xe5e9f0,
            0x4c566a, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xeceff4,
        ]);
        let levels = [0, 95, 135, 175, 215, 255];
        for r in 0..6 {
            for g in 0..6 {
                for b in 0..6 {
                    palette[16 + r * 36 + g * 6 + b] =
                        (levels[r] << 16) | (levels[g] << 8) | levels[b];
                }
            }
        }
        for n in 0..24 {
            let v = 8 + n as u32 * 10;
            palette[232 + n] = v * 0x010101;
        }
        Self {
            background: 0x101419,
            foreground: 0xd8dee9,
            bold: Some(0xeceff4),
            cursor: 0xd8dee9,
            muted: 0x4c566a,
            primary: 0x88c0d0,
            palette,
        }
    }
}
