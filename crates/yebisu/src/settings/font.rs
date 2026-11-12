//! Terminal appearance persisted by SettingsHost; defaults retain owner metrics.
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalFont {
    pub size: Option<f32>,
    pub family: String,
}
impl Default for TerminalFont {
    fn default() -> Self {
        Self {
            size: None,
            family: "SauceCodePro Nerd Font Mono".into(),
        }
    }
}
impl TerminalFont {
    pub fn metrics(&self, compact: bool) -> (f32, f32) {
        let (size, height) = if compact { (12., 17.) } else { (14., 20.) };
        self.size
            .map_or((size, height), |value| (value, value * height / size))
    }
    pub fn validate(&self) -> Result<()> {
        if self
            .size
            .is_some_and(|value| !value.is_finite() || !(8.0..=48.0).contains(&value))
        {
            return Err(Error::Settings("invalid terminal font size".into()));
        }
        if self.family.trim().is_empty()
            || self.family.len() > 128
            || self.family.chars().any(char::is_control)
        {
            return Err(Error::Settings("invalid terminal font family".into()));
        }
        Ok(())
    }
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let mut font = Self::default();
        if let Some(size) = value.get("terminal_font_size")
            && !size.is_null()
        {
            font.size = Some(
                size.as_f64()
                    .ok_or_else(|| Error::Settings("invalid terminal font size".into()))?
                    as f32,
            );
        }
        if let Some(family) = value.get("terminal_font_family") {
            font.family = family
                .as_str()
                .ok_or_else(|| Error::Settings("invalid terminal font family".into()))?
                .into();
        }
        font.validate()?;
        Ok(font)
    }
}
