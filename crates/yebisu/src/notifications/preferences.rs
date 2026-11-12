use super::{Policy, Route};
use crate::{Error, Result};
#[derive(Default)]
pub(crate) struct Edits {
    route: bool,
    sound: bool,
    volume: bool,
    muted: bool,
    quiet: bool,
}
impl Edits {
    pub fn track(&mut self, old: &Policy, new: &Policy) {
        self.route |= old.route != new.route;
        self.sound |= old.sound != new.sound;
        self.volume |= old.volume != new.volume;
        self.muted |= old.muted != new.muted;
        self.quiet |= old.quiet != new.quiet;
    }
    pub fn merge(&self, draft: &mut Policy, loaded: Policy) {
        if !self.route {
            draft.route = loaded.route;
        }
        if !self.sound {
            draft.sound = loaded.sound;
        }
        if !self.volume {
            draft.volume = loaded.volume;
        }
        if !self.muted {
            draft.muted = loaded.muted;
        }
        if !self.quiet {
            draft.quiet = loaded.quiet;
        }
    }
}
impl Policy {
    pub fn encode(&self) -> serde_json::Value {
        serde_json::json!({"route": match self.route { Route::Off => "off", Route::Toast => "toast", Route::System => "system" }, "sound":self.sound,"volume":self.volume,"muted":self.muted,"quiet":self.quiet})
    }
    pub fn decode(value: Option<&serde_json::Value>) -> Result<Self> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let invalid = || Error::Settings("invalid notification preferences".into());
        let route = match value["route"].as_str() {
            Some("off") => Route::Off,
            Some("toast") => Route::Toast,
            Some("system") => Route::System,
            _ => return Err(invalid()),
        };
        let sound = value["sound"].as_bool().ok_or_else(invalid)?;
        let muted = value["muted"].as_bool().ok_or_else(invalid)?;
        let volume = value["volume"]
            .as_u64()
            .filter(|v| *v <= 100)
            .ok_or_else(invalid)? as u8;
        let quiet = match value.get("quiet") {
            Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Array(pair)) if pair.len() == 2 => {
                let start = pair[0].as_u64().filter(|v| *v < 1440).ok_or_else(invalid)? as u16;
                let end = pair[1].as_u64().filter(|v| *v < 1440).ok_or_else(invalid)? as u16;
                Some((start, end))
            }
            _ => return Err(invalid()),
        };
        Ok(Self {
            route,
            sound,
            volume,
            muted,
            quiet,
        })
    }
}
