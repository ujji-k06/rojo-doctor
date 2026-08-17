use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Warning => formatter.write_str("warning"),
            Self::Error => formatter.write_str("error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub subject: String,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn warning(
        code: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            subject: subject.into(),
            message: message.into(),
            help: Some(help.into()),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "{}[{}]", self.severity, self.code)?;
        writeln!(formatter, "  {}", self.subject)?;
        write!(formatter, "  {}", self.message)?;

        if let Some(help) = &self.help {
            write!(formatter, "\n\n  help: {help}")?;
        }

        Ok(())
    }
}
