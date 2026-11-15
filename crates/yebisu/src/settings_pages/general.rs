//! The General page shows the application's actual bundled legal texts.

pub(crate) const LICENSE: &str = include_str!("../../../../LICENSE");
pub(crate) const NOTICE: &str = include_str!("../../../../NOTICE");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Document {
    License,
    Notice,
}

impl Document {
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::License => LICENSE,
            Self::Notice => NOTICE,
        }
    }
}
