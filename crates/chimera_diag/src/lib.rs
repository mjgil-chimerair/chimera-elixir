//! Diagnostic types and rendering for the Rust/Zig Elixir compiler.
//!
//! Provides `Diagnostic`, severity levels, labels, notes, hints,
//! renderers, JSON output, and warning-as-error mode.

#[cfg(test)]
use chimera_allocator as _;

use chimera_source::{SourceFileId, SourceLocation, SourceSpan};

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Information,
    Hint,
}

/// A diagnostic label with a span and optional message.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    #[allow(dead_code)]
    pub span: chimera_source::SourceSpan,
    pub message: String,
    pub style: LabelStyle,
}

/// Label styling hint.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LabelStyle {
    Primary,
    #[default]
    Secondary,
}

/// A note providing additional context.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub message: String,
    pub location: Option<SourceLocation>,
}

/// A hint suggesting a fix.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub message: String,
}

/// An actionable suggestion with replacement text and location.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub message: String,
    pub span: Option<SourceSpan>,
    pub replacement: Option<String>,
    pub insert_position: Option<InsertPosition>,
}

impl Suggestion {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            replacement: None,
            insert_position: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }

    pub fn with_insert_at_end(mut self, text: impl Into<String>) -> Self {
        self.insert_position = Some(InsertPosition::End(text.into()));
        self
    }

    pub fn with_insert_at_start(mut self, text: impl Into<String>) -> Self {
        self.insert_position = Some(InsertPosition::Start(text.into()));
        self
    }
}

/// Position for inserting new text.
#[derive(Debug, Clone, PartialEq)]
pub enum InsertPosition {
    Start(String),
    End(String),
}

/// Code or snippet for context.
#[derive(Debug, Clone, PartialEq)]
pub struct Code {
    pub lang: String,
    pub text: String,
}

/// The main diagnostic type.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<DiagnosticCode>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<Note>,
    pub hints: Vec<Hint>,
    pub suggestions: Vec<Suggestion>,
    pub code_snippet: Option<Code>,
}

impl Diagnostic {
    /// Create a new error diagnostic.
    pub fn error(message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            hints: Vec::new(),
            suggestions: Vec::new(),
            code_snippet: None,
        }
    }

    /// Create a new warning diagnostic.
    pub fn warning(message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            hints: Vec::new(),
            suggestions: Vec::new(),
            code_snippet: None,
        }
    }

    /// Add a label to this diagnostic.
    pub fn with_label(
        mut self,
        span: chimera_source::SourceSpan,
        message: impl Into<String>,
    ) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
            style: LabelStyle::Primary,
        });
        self
    }

    /// Add a secondary label to this diagnostic.
    pub fn with_secondary_label(
        mut self,
        span: chimera_source::SourceSpan,
        message: impl Into<String>,
    ) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
            style: LabelStyle::Secondary,
        });
        self
    }

    /// Add a note to this diagnostic.
    pub fn with_note(mut self, message: impl Into<String>) -> Self {
        self.notes.push(Note {
            message: message.into(),
            location: None,
        });
        self
    }

    /// Add a hint to this diagnostic.
    pub fn with_hint(mut self, message: impl Into<String>) -> Self {
        self.hints.push(Hint {
            message: message.into(),
        });
        self
    }

    /// Add an actionable suggestion to this diagnostic.
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Add a suggestion with just a message (no span/replacement).
    pub fn with_suggestion_msg(mut self, message: impl Into<String>) -> Self {
        self.suggestions.push(Suggestion::new(message));
        self
    }

    /// Set the diagnostic code.
    pub fn with_code(mut self, code: DiagnosticCode) -> Self {
        self.code = Some(code);
        self
    }

    /// Set the code snippet.
    pub fn with_snippet(mut self, lang: impl Into<String>, text: impl Into<String>) -> Self {
        self.code_snippet = Some(Code {
            lang: lang.into(),
            text: text.into(),
        });
        self
    }

    /// Set the severity.
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

/// A diagnostic code (e.g., "E001").
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticCode {
    pub code: String,
    pub explanation: String,
}

/// Render mode for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    /// Human-readable format with colors.
    Pretty,
    /// Plain text without colors.
    Plain,
    /// JSON format.
    Json,
    /// GNU-style format for editors.
    Gnu,
}

/// Renderer trait for diagnostics.
pub trait Renderer {
    /// Render a single diagnostic.
    fn render_diagnostic(&self, diag: &Diagnostic, mode: RenderMode) -> String;

    /// Render multiple diagnostics.
    fn render(&self, diags: &[Diagnostic], mode: RenderMode) -> String {
        diags
            .iter()
            .map(|d| self.render_diagnostic(d, mode))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Default diagnostic renderer.
pub struct DefaultRenderer {
    source_map: chimera_source::SourceMap,
    colorize: bool,
}

impl DefaultRenderer {
    pub fn new(source_map: chimera_source::SourceMap) -> Self {
        DefaultRenderer {
            source_map,
            colorize: true,
        }
    }

    pub fn without_colors(mut self) -> Self {
        self.colorize = false;
        self
    }

    fn severity_prefix(&self, severity: Severity) -> &'static str {
        match severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Information => "info",
            Severity::Hint => "hint",
        }
    }

    fn color_for_severity(&self, severity: Severity) -> &'static str {
        match severity {
            Severity::Error if self.colorize => "\x1b[31m",
            Severity::Warning if self.colorize => "\x1b[33m",
            Severity::Information if self.colorize => "\x1b[34m",
            Severity::Hint if self.colorize => "\x1b[90m",
            _ => "",
        }
    }

    fn reset_color(&self) -> &'static str {
        if self.colorize {
            "\x1b[0m"
        } else {
            ""
        }
    }
}

impl Renderer for DefaultRenderer {
    fn render_diagnostic(&self, diag: &Diagnostic, mode: RenderMode) -> String {
        match mode {
            RenderMode::Json => self.render_json(diag),
            RenderMode::Gnu => self.render_gnu(diag),
            RenderMode::Pretty | RenderMode::Plain => self.render_pretty(diag),
        }
    }
}

impl DefaultRenderer {
    fn render_json(&self, diag: &Diagnostic) -> String {
        // Simple JSON-like output without serde dependency
        let labels_json: Vec<String> = diag
            .labels
            .iter()
            .map(|l| format!("{{\"message\": \"{}\"}}", l.message))
            .collect();
        format!(
            r#"{{"severity": "{:?}", "message": "{}", "labels": [{}]}}"#,
            diag.severity,
            diag.message,
            labels_json.join(", ")
        )
    }

    fn render_pretty(&self, diag: &Diagnostic) -> String {
        let mut output = String::new();

        let color = self.color_for_severity(diag.severity);
        let reset = self.reset_color();
        let prefix = self.severity_prefix(diag.severity);

        if let Some(ref code) = diag.code {
            output.push_str(&format!(
                "{}{}{}{}: {}{}{}\n",
                color,
                prefix,
                reset,
                if diag.severity == Severity::Error {
                    ""
                } else {
                    " "
                },
                code.code,
                reset,
                if diag.labels.is_empty() { "" } else { " -- " }
            ));
        } else {
            output.push_str(&format!(
                "{}{}{}{}:\n",
                color,
                prefix,
                reset,
                if diag.severity == Severity::Error {
                    ""
                } else {
                    " "
                }
            ));
        }

        output.push_str(&format!("{}{}\n", color, diag.message));

        for label in &diag.labels {
            if let Some(file) = self.source_map.get_file(SourceFileId::new(0)) {
                let (line, col) = file.offset_to_line_col(label.span.start);
                output.push_str(&format!(
                    "  {}:{}:{}: {}\n",
                    file.path,
                    line.0 + 1,
                    col.0,
                    label.message
                ));
            }
        }

        for note in &diag.notes {
            output.push_str(&format!("  = {}\n", note.message));
        }

        for hint in &diag.hints {
            output.push_str(&format!("  hint: {}\n", hint.message));
        }

        for suggestion in &diag.suggestions {
            output.push_str(&format!("  suggestion: {}\n", suggestion.message));
            if let Some(ref replacement) = suggestion.replacement {
                output.push_str(&format!("    -> {}\n", replacement));
            }
            if let Some(ref span) = suggestion.span {
                output.push_str(&format!(
                    "    at {}:{}\n",
                    span.start.as_usize(),
                    span.end.as_usize()
                ));
            }
        }

        output
    }

    fn render_gnu(&self, diag: &Diagnostic) -> String {
        let mut output = String::new();

        let prefix = self.severity_prefix(diag.severity);

        for label in &diag.labels {
            if let Some(file) = self.source_map.get_file(SourceFileId::new(0)) {
                let (line, col) = file.offset_to_line_col(label.span.start);
                let end_line = if label.span.start == label.span.end {
                    line
                } else {
                    let (el, _) = file.offset_to_line_col(label.span.end);
                    el
                };

                if line == end_line {
                    output.push_str(&format!(
                        "{}:{}:{}: {}{}\n",
                        file.path,
                        line.0 + 1,
                        col.0,
                        prefix,
                        diag.message
                    ));
                    output.push_str(&format!(
                        "  {}: {}: {}\n",
                        if label.style == LabelStyle::Primary {
                            "error"
                        } else {
                            "note"
                        },
                        label.message,
                        diag.message
                    ));
                }
            }
        }

        if diag.labels.is_empty() {
            output.push_str(&format!("{}: {}\n", prefix, diag.message));
        }

        output
    }
}

/// Warning-as-error mode.
#[derive(Debug, Clone, Default)]
pub struct WarningAsError(pub bool);

impl WarningAsError {
    pub fn enabled(self) -> bool {
        self.0
    }

    /// Convert warnings to errors if warning-as-error is enabled.
    pub fn apply(&self, diag: &Diagnostic) -> Severity {
        if self.0 && diag.severity == Severity::Warning {
            Severity::Error
        } else {
            diag.severity
        }
    }
}

/// Collect and organize diagnostics.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticSet {
    diags: Vec<Diagnostic>,
    warning_as_error: WarningAsError,
}

impl DiagnosticSet {
    pub fn new() -> Self {
        DiagnosticSet::default()
    }

    pub fn with_warning_as_error(mut self) -> Self {
        self.warning_as_error = WarningAsError(true);
        self
    }

    /// Add a diagnostic.
    pub fn add(&mut self, diag: Diagnostic) {
        let severity = self.warning_as_error.apply(&diag);
        let mut diag = diag;
        diag.severity = severity;
        self.diags.push(diag);
    }

    /// Extend with multiple diagnostics.
    pub fn extend(&mut self, diags: impl IntoIterator<Item = Diagnostic>) {
        for diag in diags {
            self.add(diag);
        }
    }

    /// Get all diagnostics.
    pub fn diags(&self) -> &[Diagnostic] {
        &self.diags
    }

    /// Get diagnostics of a specific severity.
    pub fn with_severity(&self, sev: Severity) -> Vec<&Diagnostic> {
        self.diags.iter().filter(|d| d.severity == sev).collect()
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(|d| d.severity == Severity::Error)
    }

    /// Get the number of errors.
    pub fn error_count(&self) -> usize {
        self.with_severity(Severity::Error).len()
    }

    /// Get the number of warnings.
    pub fn warning_count(&self) -> usize {
        self.with_severity(Severity::Warning).len()
    }

    /// Sort diagnostics by file and location.
    pub fn sort(&mut self) {
        self.diags.sort_by(|a, b| {
            let a_start = a
                .labels
                .first()
                .map(|l| l.span.start)
                .unwrap_or_else(|| chimera_source::SourceOffset::new(0));
            let b_start = b
                .labels
                .first()
                .map(|l| l.span.start)
                .unwrap_or_else(|| chimera_source::SourceOffset::new(0));
            a_start.cmp(&b_start)
        });
    }

    /// Clear all diagnostics.
    pub fn clear(&mut self) {
        self.diags.clear();
    }

    /// Get all suggestions from diagnostics that have them.
    pub fn suggestions(&self) -> Vec<&Suggestion> {
        self.diags
            .iter()
            .flat_map(|d| d.suggestions.iter())
            .collect()
    }

    /// Get suggestions grouped by diagnostic.
    pub fn suggestions_by_diagnostic(&self) -> Vec<(usize, &Suggestion)> {
        self.diags
            .iter()
            .enumerate()
            .flat_map(|(i, d)| d.suggestions.iter().map(move |s| (i, s)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_source::{SourceMap, SourceOffset, SourceSpan};

    #[test]
    fn test_diagnostic_error() {
        let diag = Diagnostic::error("test error");
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "test error");
    }

    #[test]
    fn test_diagnostic_warning() {
        let diag = Diagnostic::warning("test warning");
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn test_diagnostic_with_label() {
        let diag = Diagnostic::error("test").with_label(
            SourceSpan::new(SourceOffset::new(0), SourceOffset::new(5)),
            "label message",
        );
        assert_eq!(diag.labels.len(), 1);
    }

    #[test]
    fn test_diagnostic_with_note() {
        let diag = Diagnostic::error("test").with_note("additional info");
        assert_eq!(diag.notes.len(), 1);
    }

    #[test]
    fn test_diagnostic_with_hint() {
        let diag = Diagnostic::error("test").with_hint("try this");
        assert_eq!(diag.hints.len(), 1);
    }

    #[test]
    fn test_diagnostic_with_code() {
        let diag = Diagnostic::error("test").with_code(DiagnosticCode {
            code: "E001".to_string(),
            explanation: "test error".to_string(),
        });
        assert!(diag.code.is_some());
    }

    #[test]
    fn test_warning_as_error() {
        let wae = WarningAsError(true);
        let diag = Diagnostic::warning("test");
        let applied = wae.apply(&diag);
        assert_eq!(applied, Severity::Error);
    }

    #[test]
    fn test_warning_as_error_disabled() {
        let wae = WarningAsError(false);
        let diag = Diagnostic::warning("test");
        let applied = wae.apply(&diag);
        assert_eq!(applied, Severity::Warning);
    }

    #[test]
    fn test_diagnostic_set_has_errors() {
        let mut set = DiagnosticSet::new();
        assert!(!set.has_errors());

        set.add(Diagnostic::warning("warning"));
        assert!(!set.has_errors());

        set.add(Diagnostic::error("error"));
        assert!(set.has_errors());
    }

    #[test]
    fn test_diagnostic_set_counts() {
        let mut set = DiagnosticSet::new();
        set.add(Diagnostic::error("error1"));
        set.add(Diagnostic::error("error2"));
        set.add(Diagnostic::warning("warning"));

        assert_eq!(set.error_count(), 2);
        assert_eq!(set.warning_count(), 1);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Error < Severity::Warning);
        assert!(Severity::Warning < Severity::Information);
        assert!(Severity::Information < Severity::Hint);
    }

    #[test]
    fn test_suggestion_new() {
        let suggestion = Suggestion::new("Add missing semicolon");
        assert_eq!(suggestion.message, "Add missing semicolon");
        assert!(suggestion.span.is_none());
        assert!(suggestion.replacement.is_none());
    }

    #[test]
    fn test_suggestion_with_span() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(10),
        );
        let suggestion = Suggestion::new("Replace with correct syntax").with_span(span.clone());
        assert!(suggestion.span.is_some());
    }

    #[test]
    fn test_suggestion_with_replacement() {
        let suggestion = Suggestion::new("Fix typo").with_replacement("correct");
        assert_eq!(suggestion.replacement.as_deref(), Some("correct"));
    }

    #[test]
    fn test_suggestion_insert_position() {
        let suggestion = Suggestion::new("Add import").with_insert_at_end("import MyModule;");
        match suggestion.insert_position {
            Some(InsertPosition::End(text)) => assert_eq!(text, "import MyModule;"),
            _ => panic!("Expected InsertPosition::End"),
        }

        let suggestion2 =
            Suggestion::new("Add import at start").with_insert_at_start("import MyModule;");
        match suggestion2.insert_position {
            Some(InsertPosition::Start(text)) => assert_eq!(text, "import MyModule;"),
            _ => panic!("Expected InsertPosition::Start"),
        }
    }

    #[test]
    fn test_diagnostic_with_suggestion() {
        let diag = Diagnostic::error("syntax error")
            .with_suggestion(Suggestion::new("Did you mean 'def'?"))
            .with_suggestion_msg("Try using 'do' instead");
        assert_eq!(diag.suggestions.len(), 2);
    }

    #[test]
    fn test_diagnostic_suggestions_empty_by_default() {
        let diag = Diagnostic::error("test error");
        assert!(diag.suggestions.is_empty());
    }

    #[test]
    fn test_diagnostic_set_suggestions() {
        let mut set = DiagnosticSet::new();
        set.add(Diagnostic::error("error 1").with_suggestion(Suggestion::new("fix 1")));
        set.add(
            Diagnostic::error("error 2")
                .with_suggestion(Suggestion::new("fix 2"))
                .with_suggestion(Suggestion::new("fix 3")),
        );

        let suggestions = set.suggestions();
        assert_eq!(suggestions.len(), 3);
    }

    #[test]
    fn test_diagnostic_set_suggestions_by_diagnostic() {
        let mut set = DiagnosticSet::new();
        set.add(Diagnostic::error("error 1").with_suggestion(Suggestion::new("fix 1")));
        set.add(Diagnostic::error("error 2").with_suggestion(Suggestion::new("fix 2")));

        let grouped = set.suggestions_by_diagnostic();
        assert_eq!(grouped.len(), 2);
    }
}
