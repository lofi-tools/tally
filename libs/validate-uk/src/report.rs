//! Validation issue and report types.

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    Error,
    Warning,
}

impl IssueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            IssueKind::Error => "error",
            IssueKind::Warning => "warning",
        }
    }
}

/// A single validation finding, mirroring the message codes Arelle's
/// `validate/UK` plugin produces (JFCVC.3312, JFCVC.3314, HMRC.5.3, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub code: String,
    pub kind: IssueKind,
    pub message: String,
    /// The fact / context the issue relates to, when known (e.g. the context
    /// id or concept local name).
    pub location: Option<String>,
}

impl Issue {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Issue {
            code: code.into(),
            kind: IssueKind::Error,
            message: message.into(),
            location: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Issue {
            code: code.into(),
            kind: IssueKind::Warning,
            message: message.into(),
            location: None,
        }
    }

    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Render like Arelle's default log line:
    /// `[JFCVC.3312] message - file`.
    pub fn to_log_line(&self, file: &str) -> String {
        format!("[{}] {} - {}", self.code, self.message, file)
    }
}

/// The result of validating a single document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(|i| i.kind == IssueKind::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(|i| i.kind == IssueKind::Warning)
    }

    /// Exit code matching Arelle's `--validationExitCode`: 0 when clean,
    /// 1 on errors (warnings alone do not fail).
    pub fn exit_code(&self) -> i32 {
        if self.is_ok() {
            0
        } else {
            1
        }
    }
}
