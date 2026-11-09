// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/config/font_weight.rs at 398c132a06723a0cc1494842a34666d173ec0200
use gpui::FontWeight;
const BOLD_STEP: f32 = 300.0;
pub fn bold(weight: FontWeight) -> FontWeight {
    FontWeight((weight.0 + BOLD_STEP).clamp(FontWeight::BOLD.0, FontWeight::BLACK.0))
}
