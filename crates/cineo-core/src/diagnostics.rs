//! Non-fatal findings produced while interpreting untrusted input.

use std::fmt;

/// A value together with the non-fatal warnings produced while building it.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed<T> {
    pub value: T,
    pub warnings: Vec<Warning>,
}

/// A non-fatal problem found in untrusted input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// JSON-pointer-like location of the problem, e.g. `catalogs[2].extra[0]`.
    pub location: String,
    pub kind: WarningKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WarningKind {
    IgnoredField { reason: String },
    SkippedItem { reason: String },
    Duplicate { key: String },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            WarningKind::IgnoredField { reason } => {
                write!(f, "{}: ignored ({reason})", self.location)
            }
            WarningKind::SkippedItem { reason } => {
                write!(f, "{}: skipped ({reason})", self.location)
            }
            WarningKind::Duplicate { key } => {
                write!(f, "{}: duplicate `{key}` dropped", self.location)
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Warnings(Vec<Warning>);

impl Warnings {
    pub(crate) fn ignored(&mut self, location: impl Into<String>, reason: impl Into<String>) {
        self.0.push(Warning {
            location: location.into(),
            kind: WarningKind::IgnoredField {
                reason: reason.into(),
            },
        });
    }

    pub(crate) fn skipped(&mut self, location: impl Into<String>, reason: impl Into<String>) {
        self.0.push(Warning {
            location: location.into(),
            kind: WarningKind::SkippedItem {
                reason: reason.into(),
            },
        });
    }

    pub(crate) fn duplicate(&mut self, location: impl Into<String>, key: impl Into<String>) {
        self.0.push(Warning {
            location: location.into(),
            kind: WarningKind::Duplicate { key: key.into() },
        });
    }

    pub(crate) fn finish<T>(self, value: T) -> Parsed<T> {
        Parsed {
            value,
            warnings: self.0,
        }
    }
}
