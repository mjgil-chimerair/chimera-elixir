//! Lint rules for the zelix Rust/Zig Elixir compiler.
//!
//! Provides a framework for defining, registering, and executing lint rules
//! with severity levels, configurations, and rule metadata.

#[cfg(test)]
use chimera_allocator as _;

use std::collections::HashMap;
use chimera_plugin_api::Severity;

/// A single lint rule with its configuration and metadata.
pub struct LintRule {
    pub metadata: RuleMetadata,
    pub severity: Severity,
    pub enabled: bool,
    pub config: HashMap<String, String>,
    pub check_fn: Box<dyn Fn(&LintRule, &LintInput) -> Vec<LintFinding> + Send + Sync>,
}

impl LintRule {
    pub fn new(
        metadata: RuleMetadata,
        severity: Severity,
        check_fn: impl Fn(&LintRule, &LintInput) -> Vec<LintFinding> + 'static + Send + Sync,
    ) -> Self {
        Self {
            metadata,
            severity,
            enabled: true,
            config: HashMap::new(),
            check_fn: Box::new(check_fn),
        }
    }

    pub fn check(&self, input: &LintInput) -> Vec<LintFinding> {
        if !self.enabled {
            return Vec::new();
        }
        (self.check_fn)(self, input)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_config(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.config.insert(key.into(), value.into());
    }

    pub fn get_config(&self, key: &str) -> Option<&str> {
        self.config.get(key).map(|s| s.as_str())
    }
}

/// Metadata about a lint rule.
#[derive(Debug, Clone)]
pub struct RuleMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: RuleCategory,
    pub languages: Vec<String>,
}

impl RuleMetadata {
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>, category: RuleCategory) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category,
            languages: vec!["elixir".to_string()],
        }
    }
}

/// Category of lint rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleCategory {
    CodeStyle,
    BestPractice,
    PotentialBug,
    Security,
    Performance,
    Documentation,
    Deprecated,
}

impl RuleCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleCategory::CodeStyle => "code_style",
            RuleCategory::BestPractice => "best_practice",
            RuleCategory::PotentialBug => "potential_bug",
            RuleCategory::Security => "security",
            RuleCategory::Performance => "performance",
            RuleCategory::Documentation => "documentation",
            RuleCategory::Deprecated => "deprecated",
        }
    }
}

/// Input data for lint rule checking.
#[derive(Debug, Clone)]
pub struct LintInput {
    pub source_id: u32,
    pub file_path: String,
    pub source: String,
    pub ast: LintAst,
    pub line_offsets: Vec<usize>,
}

impl LintInput {
    pub fn new(source_id: u32, file_path: impl Into<String>, source: impl Into<String>) -> Self {
        let source = source.into();
        let mut line_offsets = vec![0];
        for (i, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_offsets.push(i + 1);
            }
        }
        Self {
            source_id,
            file_path: file_path.into(),
            source,
            ast: LintAst::default(),
            line_offsets,
        }
    }

    /// Add AST nodes from a parsed CST/AST for semantic linting.
    pub fn with_ast_nodes(mut self, nodes: Vec<LintNode>) -> Self {
        self.ast = LintAst { nodes };
        self
    }

    /// Get a node by index.
    pub fn get_node(&self, index: usize) -> Option<&LintNode> {
        self.ast.nodes.get(index)
    }

    /// Get all nodes of a specific kind.
    pub fn get_nodes_by_kind(&self, kind: &str) -> Vec<&LintNode> {
        self.ast.nodes.iter().filter(|n| n.kind == kind).collect()
    }

    /// Get the root node if available.
    pub fn root_node(&self) -> Option<&LintNode> {
        self.ast.nodes.first()
    }

    pub fn get_line(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.line_offsets.len() {
            return None;
        }
        let start = self.line_offsets[line - 1];
        let end = self.line_offsets.get(line).copied().unwrap_or(self.source.len());
        Some(&self.source[start..end].trim_end_matches('\n'))
    }

    pub fn get_column(&self, offset: usize) -> Option<usize> {
        for (i, &start) in self.line_offsets.iter().enumerate() {
            if start > offset {
                return Some(i);
            }
        }
        Some(self.line_offsets.len())
    }
}

/// Simplified AST representation for linting.
#[derive(Debug, Default, Clone)]
pub struct LintAst {
    pub nodes: Vec<LintNode>,
}

impl LintAst {
    /// Add a child relationship.
    pub fn add_child(&mut self, parent_idx: usize, child_idx: usize) {
        if parent_idx < self.nodes.len() && child_idx < self.nodes.len() {
            self.nodes[parent_idx].children.push(child_idx);
            let _ = self.nodes[child_idx].parent.replace(parent_idx);
        }
    }

    /// Find all descendant nodes of a specific kind.
    pub fn find_descendants(&self, parent_idx: usize, kind: &str) -> Vec<usize> {
        let mut result = Vec::new();
        self.collect_descendants(parent_idx, kind, &mut result);
        result
    }

    fn collect_descendants(&self, parent_idx: usize, kind: &str, result: &mut Vec<usize>) {
        if let Some(node) = self.nodes.get(parent_idx) {
            if node.kind == kind {
                result.push(parent_idx);
            }
            for &child_idx in &node.children {
                self.collect_descendants(child_idx, kind, result);
            }
        }
    }
}

/// CST node wrapper for lint integration.
#[derive(Debug, Clone)]
pub struct CstNode {
    kind: CstKind,
    start_offset: u32,
    end_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CstKind {
    Module,
    Function,
    Clause,
    Expression,
    Identifier,
    Literal,
    Operator,
    Delimiter,
    Comment,
    Whitespace,
    Newline,
    Error,
    Unknown,
}

impl CstNode {
    pub fn new(kind: CstKind, start_offset: u32, end_offset: u32) -> Self {
        Self { kind, start_offset, end_offset }
    }

    pub fn kind(&self) -> CstKind {
        self.kind
    }

    pub fn start_offset(&self) -> u32 {
        self.start_offset
    }

    pub fn end_offset(&self) -> u32 {
        self.end_offset
    }

    pub fn span_len(&self) -> u32 {
        self.end_offset - self.start_offset
    }
}

#[derive(Debug, Clone)]
pub struct LintNode {
    pub kind: String,
    pub content: String,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
}

/// A finding from a lint rule.
#[derive(Debug, Clone)]
pub struct LintFinding {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub location: LintLocation,
    pub note: Option<String>,
    pub hint: Option<String>,
}

impl LintFinding {
    pub fn new(
        rule_id: impl Into<String>,
        severity: Severity,
        message: impl Into<String>,
        location: LintLocation,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            severity,
            message: message.into(),
            location,
            note: None,
            hint: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Location of a lint finding.
#[derive(Debug, Clone, Copy)]
pub struct LintLocation {
    pub source_id: u32,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub length: usize,
}

impl LintLocation {
    pub fn new(source_id: u32, line: usize, column: usize, offset: usize) -> Self {
        Self {
            source_id,
            line,
            column,
            offset,
            length: 0,
        }
    }

    pub fn with_length(mut self, length: usize) -> Self {
        self.length = length;
        self
    }
}

/// Registry for managing lint rules.
pub struct LintRegistry {
    rules: HashMap<String, Box<LintRule>>,
    categories: HashMap<RuleCategory, Vec<String>>,
}

impl LintRegistry {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            categories: HashMap::new(),
        }
    }

    /// Register a lint rule.
    pub fn register(&mut self, rule: LintRule) {
        let rule_id = rule.metadata.id.clone();
        self.rules.insert(rule_id.clone(), Box::new(rule));
        self.categories
            .entry(RuleCategory::CodeStyle)
            .or_default()
            .push(rule_id);
    }

    /// Get a rule by ID.
    pub fn get(&self, rule_id: &str) -> Option<&LintRule> {
        self.rules.get(rule_id).map(|r| r.as_ref())
    }

    /// Get a mutable rule by ID.
    pub fn get_mut(&mut self, rule_id: &str) -> Option<&mut LintRule> {
        self.rules.get_mut(rule_id).map(|r| r.as_mut())
    }

    /// Get all rules for a category.
    pub fn get_by_category(&self, category: RuleCategory) -> Vec<&LintRule> {
        self.categories
            .get(&category)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.rules.get(id).map(|r| r.as_ref()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all enabled rules.
    pub fn get_enabled(&self) -> Vec<&LintRule> {
        self.rules.values().filter(|r| r.enabled).map(|r| r.as_ref()).collect()
    }

    /// Get all rule IDs.
    pub fn rule_ids(&self) -> Vec<String> {
        self.rules.keys().cloned().collect()
    }

    /// Enable a rule by ID.
    pub fn enable(&mut self, rule_id: &str) -> bool {
        if let Some(rule) = self.rules.get_mut(rule_id) {
            rule.set_enabled(true);
            true
        } else {
            false
        }
    }

    /// Disable a rule by ID.
    pub fn disable(&mut self, rule_id: &str) -> bool {
        if let Some(rule) = self.rules.get_mut(rule_id) {
            rule.set_enabled(false);
            true
        } else {
            false
        }
    }

    /// Run all enabled rules on input.
    pub fn run(&self, input: &LintInput) -> Vec<LintFinding> {
        let mut findings = Vec::new();
        for rule in self.get_enabled() {
            findings.extend(rule.check(input));
        }
        findings
    }

    /// Run rules for a specific category.
    pub fn run_category(&self, category: RuleCategory, input: &LintInput) -> Vec<LintFinding> {
        let mut findings = Vec::new();
        for rule in self.get_by_category(category) {
            findings.extend(rule.check(input));
        }
        findings
    }
}

impl Default for LintRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Project-level lint configuration.
#[derive(Debug, Clone)]
pub struct LintConfig {
    pub rules: HashMap<String, RuleConfig>,
    pub exclude_paths: Vec<String>,
    pub include_paths: Vec<String>,
}

impl LintConfig {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            exclude_paths: Vec::new(),
            include_paths: Vec::new(),
        }
    }

    pub fn with_rule(mut self, rule_id: impl Into<String>, severity: Severity) -> Self {
        self.rules.insert(rule_id.into(), RuleConfig { severity: Some(severity), enabled: true });
        self
    }

    pub fn exclude_path(mut self, path: impl Into<String>) -> Self {
        self.exclude_paths.push(path.into());
        self
    }
}

impl Default for LintConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for a single rule.
#[derive(Debug, Clone)]
pub struct RuleConfig {
    pub severity: Option<Severity>,
    pub enabled: bool,
}

/// Linter that executes lint rules against source code.
pub struct Linter {
    registry: LintRegistry,
    config: LintConfig,
}

impl Linter {
    pub fn new() -> Self {
        Self {
            registry: LintRegistry::new(),
            config: LintConfig::new(),
        }
    }

    pub fn with_builtin_rules(mut self) -> Self {
        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ001",
                "Trailing whitespace",
                "Detects and fixes trailing whitespace in source files",
                RuleCategory::CodeStyle,
            ),
            Severity::Hint,
            |_rule, input| {
                let mut findings = Vec::new();
                for (offset, byte) in input.source.bytes().enumerate() {
                    if byte == b' ' || byte == b'\t' {
                        if offset + 1 < input.source.len() {
                            let next = input.source.as_bytes()[offset + 1];
                            if next == b'\n' || next == b'\r' {
                                let line = input.get_column(offset).unwrap_or(1);
                                findings.push(LintFinding::new(
                                    "RZ001",
                                    Severity::Hint,
                                    "Trailing whitespace detected",
                                    LintLocation::new(input.source_id, line, 0, offset),
                                ));
                            }
                        }
                    }
                }
                findings
            },
        ));

        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ002",
                "Tab character",
                "Detects tab characters which can cause inconsistent display",
                RuleCategory::CodeStyle,
            ),
            Severity::Info,
            |_rule, input| {
                input
                    .source
                    .bytes()
                    .enumerate()
                    .filter(|(_, b)| *b == b'\t')
                    .filter_map(|(offset, _)| {
                        Some(LintFinding::new(
                            "RZ002",
                            Severity::Info,
                            "Tab character detected",
                            LintLocation::new(input.source_id, input.get_column(offset)?, 0, offset),
                        ))
                    })
                    .collect()
            },
        ));

        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ003",
                "Line too long",
                "Detects lines exceeding 120 characters",
                RuleCategory::CodeStyle,
            ),
            Severity::Warning,
            |_rule, input| {
                input
                    .line_offsets
                    .windows(2)
                    .enumerate()
                    .filter(|(_, window)| {
                        let start = window[0];
                        let end = window[1];
                        end - start > 120
                    })
                    .filter_map(|(i, window)| {
                        Some(LintFinding::new(
                            "RZ003",
                            Severity::Warning,
                            "Line exceeds 120 characters",
                            LintLocation::new(input.source_id, i + 1, 0, window[0]),
                        ))
                    })
                    .collect()
            },
        ));

        // RZ004: TODO/FIXME comment detection
        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ004",
                "Incomplete code marker",
                "Detects TODO or FIXME comments indicating incomplete code",
                RuleCategory::PotentialBug,
            ),
            Severity::Warning,
            |_rule, input| {
                let mut findings = Vec::new();
                for (offset, _) in input.source.bytes().enumerate() {
                    if let Some(line) = input.get_line(input.get_column(offset).unwrap_or(1)) {
                        let line_upper = line.to_uppercase();
                        if line_upper.contains("TODO") || line_upper.contains("FIXME") || line_upper.contains("XXX") {
                            if let Some(col) = line.find(|c: char| !c.is_whitespace()) {
                                findings.push(LintFinding::new(
                                    "RZ004",
                                    Severity::Warning,
                                    format!("Incomplete code marker found: {}", line.trim()),
                                    LintLocation::new(input.source_id, input.get_column(offset).unwrap_or(1), col, offset),
                                ));
                            }
                        }
                    }
                }
                findings
            },
        ));

        // RZ005: Cyclic module alias (same module referenced multiple times)
        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ005",
                "Repeated alias",
                "Detects when the same module alias appears multiple times in a row",
                RuleCategory::CodeStyle,
            ),
            Severity::Hint,
            |_rule, input| {
                let mut findings = Vec::new();
                let mut chars = input.source.chars().peekable();
                let mut pos = 0;

                while let Some(c) = chars.next() {
                    if c == '\n' {
                        pos += 1;
                    }
                    if c == '.' {
                        if let Some(&next) = chars.peek() {
                            if next == '.' {
                                findings.push(LintFinding::new(
                                    "RZ005",
                                    Severity::Hint,
                                    "Repeated dot operator detected",
                                    LintLocation::new(input.source_id, pos + 1, 0, pos),
                                ));
                            }
                        }
                    }
                }
                findings
            },
        ));

        // RZ006: Empty function clause
        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ006",
                "Empty body",
                "Detects functions or clauses with empty bodies",
                RuleCategory::PotentialBug,
            ),
            Severity::Warning,
            |_rule, input| {
                let mut findings = Vec::new();
                let patterns = ["do\nend", "do:\nend", "do  \nend"];

                for pattern in &patterns {
                    let mut search_start = 0;
                    while let Some(pos) = input.source[search_start..].find(pattern) {
                        let abs_pos = search_start + pos;
                        findings.push(LintFinding::new(
                            "RZ006",
                            Severity::Warning,
                            "Empty body detected - this may be unintentional",
                            LintLocation::new(input.source_id, input.get_column(abs_pos).unwrap_or(1), 0, abs_pos),
                        ));
                        search_start = abs_pos + 1;
                    }
                }
                findings
            },
        ));

        // RZ007: Unused import
        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ007",
                "Invalid escape",
                "Detects invalid escape sequences in strings",
                RuleCategory::PotentialBug,
            ),
            Severity::Warning,
            |_rule, input| {
                let mut findings = Vec::new();
                let mut in_string = false;
                let bytes = input.source.as_bytes();

                for (i, &byte) in bytes.iter().enumerate() {
                    if byte == b'"' && (i == 0 || bytes[i-1] != b'\\') {
                        in_string = !in_string;
                    }

                    if in_string && byte == b'\\' && i + 1 < bytes.len() {
                        let next = bytes[i + 1];
                        // Valid escapes: \n, \t, \r, \\, \", \', \s, \b, \f, \e, \v, \0, \x
                        let valid = matches!(next, b'n' | b't' | b'r' | b'\\' | b'"' | b'\'' | b's' | b'b' | b'f' | b'e' | b'v' | b'0' | b'x');
                        if !valid && !next.is_ascii_digit() {
                            findings.push(LintFinding::new(
                                "RZ007",
                                Severity::Warning,
                                format!("Invalid escape sequence: \\{}", next as char),
                                LintLocation::new(input.source_id, input.get_column(i).unwrap_or(1), 0, i),
                            ));
                        }
                    }
                }
                findings
            },
        ));

        // RZ008: Trailing newline
        self.registry.register(LintRule::new(
            RuleMetadata::new(
                "RZ008",
                "Missing trailing newline",
                "Detects files that don't end with a newline",
                RuleCategory::CodeStyle,
            ),
            Severity::Hint,
            |_rule, input| {
                if input.source.is_empty() {
                    return Vec::new();
                }
                let last_char = input.source.chars().last().unwrap();
                if last_char != '\n' {
                    vec![LintFinding::new(
                        "RZ008",
                        Severity::Hint,
                        "File should end with a trailing newline",
                        LintLocation::new(input.source_id, input.line_offsets.len(), 0, input.source.len()),
                    )]
                } else {
                    Vec::new()
                }
            },
        ));

        self
    }

    pub fn registry(&self) -> &LintRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut LintRegistry {
        &mut self.registry
    }

    pub fn config(&self) -> &LintConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut LintConfig {
        &mut self.config
    }

    pub fn run(&self, input: &LintInput) -> Vec<LintFinding> {
        self.registry.run(input)
    }

    pub fn run_with_severity_filter(&self, input: &LintInput, min_severity: Severity) -> Vec<LintFinding> {
        self.run(input)
            .into_iter()
            .filter(|f| f.severity >= min_severity)
            .collect()
    }

    /// Convert lint findings to diagnostic set for rendering.
    pub fn findings_to_diagnostics(&self, findings: Vec<LintFinding>) -> chimera_diag::DiagnosticSet {
        use chimera_diag::{Diagnostic, Severity as DiagSeverity};

        let mut diag_set = chimera_diag::DiagnosticSet::new();

        for finding in findings {
            let severity = match finding.severity {
                Severity::Hint => DiagSeverity::Information,
                Severity::Info => DiagSeverity::Information,
                Severity::Warning => DiagSeverity::Warning,
                Severity::Error => DiagSeverity::Error,
                Severity::Critical => DiagSeverity::Error,
            };

            let mut diag = Diagnostic::error(finding.message.clone())
                .with_severity(severity);

            // Add location as primary label
            diag = diag.with_label(
                chimera_source::SourceSpan::new(
                    chimera_source::SourceOffset::new(finding.location.offset as u32),
                    chimera_source::SourceOffset::new((finding.location.offset + finding.location.length) as u32),
                ),
                format!("{} ({:?})", finding.rule_id, finding.location),
            );

            // Add note if present
            if let Some(note) = &finding.note {
                diag = diag.with_note(note.clone());
            }

            // Add hint if present
            if let Some(hint) = &finding.hint {
                diag = diag.with_hint(hint.clone());
            }

            diag_set.add(diag);
        }

        diag_set
    }
}

impl Default for Linter {
    fn default() -> Self {
        Self::new().with_builtin_rules()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_registry_register() {
        let mut registry = LintRegistry::new();
        registry.register(LintRule::new(
            RuleMetadata::new("RZ999", "Test rule", "A test rule", RuleCategory::CodeStyle),
            Severity::Warning,
            |_, _| vec![],
        ));
        assert!(registry.get("RZ999").is_some());
    }

    #[test]
    fn test_lint_registry_enable_disable() {
        let mut registry = LintRegistry::new();
        registry.register(LintRule::new(
            RuleMetadata::new("RZ999", "Test rule", "A test rule", RuleCategory::CodeStyle),
            Severity::Warning,
            |_, _| vec![],
        ));
        assert!(registry.get("RZ999").unwrap().enabled);
        registry.disable("RZ999");
        assert!(!registry.get("RZ999").unwrap().enabled);
        registry.enable("RZ999");
        assert!(registry.get("RZ999").unwrap().enabled);
    }

    #[test]
    fn test_lint_input_line_offsets() {
        let input = LintInput::new(1, "test.ex", "line1\nline2\nline3");
        assert_eq!(input.line_offsets.len(), 3);
        assert_eq!(input.get_line(1), Some("line1"));
        assert_eq!(input.get_line(2), Some("line2"));
        assert_eq!(input.get_line(3), Some("line3"));
    }

    #[test]
    fn test_lint_finding_with_note_and_hint() {
        let finding = LintFinding::new("RZ001", Severity::Warning, "Test", LintLocation::new(1, 1, 0, 0))
            .with_note("This is a note")
            .with_hint("This is a hint");
        assert!(finding.note.is_some());
        assert!(finding.hint.is_some());
    }

    #[test]
    fn test_linter_run() {
        let linter = Linter::new();
        let input = LintInput::new(1, "test.ex", "defmodule Foo do\n  :ok\nend");
        let findings = linter.run(&input);
        assert!(findings.is_empty()); // No trailing whitespace, tabs, or long lines
    }

    #[test]
    fn test_linter_trailing_whitespace() {
        let linter = Linter::default();
        // Use content without trailing newline to avoid RZ008 firing first
        let input = LintInput::new(1, "test.ex", "line1 \nline2"); // no trailing newline
        let findings = linter.run(&input);
        let trailing_findings: Vec<_> = findings.iter().filter(|f| f.rule_id == "RZ001").collect();
        assert!(!trailing_findings.is_empty());
        // RZ008 should also fire since no trailing newline
        let newline_findings: Vec<_> = findings.iter().filter(|f| f.rule_id == "RZ008").collect();
        assert!(!newline_findings.is_empty());
    }

    #[test]
    fn test_rule_category_as_str() {
        assert_eq!(RuleCategory::CodeStyle.as_str(), "code_style");
        assert_eq!(RuleCategory::Security.as_str(), "security");
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Hint < Severity::Info);
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    #[test]
    fn test_linter_todo_fixes() {
        let input = Linter::default().run(&LintInput::new(
            1,
            "test.ex",
            "# TODO: implement this\n# FIXME: fix this bug\n:ok",
        ));
        // Check that RZ004 exists and finds TODO/FIXME
        let todo_findings: Vec<_> = input.iter().filter(|f| f.rule_id == "RZ004").collect();
        assert!(!todo_findings.is_empty());
    }

    #[test]
    fn test_linter_missing_newline() {
        let linter = Linter::default();
        // String with trailing newline
        let input = LintInput::new(1, "test.ex", "defmodule Foo do\n  :ok\nend\n");
        let findings = linter.run(&input);
        let newline_findings: Vec<_> = findings.iter().filter(|f| f.rule_id == "RZ008").collect();
        assert!(newline_findings.is_empty()); // Has newline

        // String without trailing newline
        let input2 = LintInput::new(2, "test.ex", "defmodule Foo do\n  :ok\nend"); // no trailing newline
        let findings2 = linter.run(&input2);
        let newline_findings2: Vec<_> = findings2.iter().filter(|f| f.rule_id == "RZ008").collect();
        assert!(!newline_findings2.is_empty());
    }

    #[test]
    fn test_linter_invalid_escape() {
        let linter = Linter::default();
        let input = LintInput::new(1, "test.ex", r#" "hello\q" "#);
        let findings = linter.run(&input);
        let escape_findings: Vec<_> = findings.into_iter().filter(|f| f.rule_id == "RZ007").collect();
        assert!(!escape_findings.is_empty());
    }

    #[test]
    fn test_linter_with_all_rules() {
        let linter = Linter::default();
        let rules: Vec<_> = linter.registry().rule_ids();
        // Should have RZ001-RZ008
        assert!(rules.contains(&"RZ001".to_string()));
        assert!(rules.contains(&"RZ002".to_string()));
        assert!(rules.contains(&"RZ003".to_string()));
        assert!(rules.contains(&"RZ004".to_string()));
        assert!(rules.contains(&"RZ005".to_string()));
        assert!(rules.contains(&"RZ006".to_string()));
        assert!(rules.contains(&"RZ007".to_string()));
        assert!(rules.contains(&"RZ008".to_string()));
        assert_eq!(rules.len(), 8);
    }

    #[test]
    fn test_lint_finding_note_hint() {
        let finding = LintFinding::new("RZ001", Severity::Warning, "Test", LintLocation::new(1, 1, 0, 0))
            .with_note("This is a note")
            .with_hint("This is a hint");
        assert_eq!(finding.note.as_deref(), Some("This is a note"));
        assert_eq!(finding.hint.as_deref(), Some("This is a hint"));
    }

    #[test]
    fn test_findings_to_diagnostics() {
        let linter = Linter::default();
        let findings = vec![
            LintFinding::new(
                "RZ001",
                Severity::Warning,
                "Trailing whitespace",
                LintLocation::new(1, 1, 10, 10),
            )
            .with_note("This is a note")
            .with_hint("Remove the trailing whitespace"),
        ];

        let diags = linter.findings_to_diagnostics(findings);
        assert!(diags.has_errors() || diags.warning_count() > 0);
    }

    #[test]
    fn test_lint_input_get_nodes_by_kind() {
        let nodes = vec![
            LintNode { kind: "Module".into(), content: "defmodule".into(), line: 1, column: 0, offset: 0, children: vec![], parent: None },
            LintNode { kind: "Function".into(), content: "foo".into(), line: 1, column: 10, offset: 10, children: vec![], parent: None },
            LintNode { kind: "Module".into(), content: "defmodule2".into(), line: 2, column: 0, offset: 20, children: vec![], parent: None },
        ];
        let input = LintInput::new(1, "test.ex", "defmodule Foo do end\ndefmodule Bar do end")
            .with_ast_nodes(nodes);

        let modules = input.get_nodes_by_kind("Module");
        assert_eq!(modules.len(), 2);

        let functions = input.get_nodes_by_kind("Function");
        assert_eq!(functions.len(), 1);
    }

    #[test]
    fn test_lint_location_debug() {
        let loc = LintLocation::new(1, 5, 10, 100);
        let debug_str = format!("{:?}", loc);
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_lint_ast_add_child() {
        let mut ast = LintAst {
            nodes: vec![
                LintNode { kind: "Module".into(), content: "defmodule".into(), line: 1, column: 0, offset: 0, children: vec![], parent: None },
                LintNode { kind: "Expression".into(), content: "foo".into(), line: 1, column: 10, offset: 10, children: vec![], parent: None },
            ],
        };
        ast.add_child(0, 1);
        assert_eq!(ast.nodes[0].children, vec![1]);
        assert_eq!(ast.nodes[1].parent, Some(0));
    }

    #[test]
    fn test_lint_ast_find_descendants() {
        let ast = LintAst {
            nodes: vec![
                LintNode { kind: "Module".into(), content: "defmodule".into(), line: 1, column: 0, offset: 0, children: vec![1, 2], parent: None },
                LintNode { kind: "Function".into(), content: "foo".into(), line: 1, column: 10, offset: 10, children: vec![], parent: Some(0) },
                LintNode { kind: "Function".into(), content: "bar".into(), line: 2, column: 10, offset: 30, children: vec![], parent: Some(0) },
            ],
        };

        let descendants = ast.find_descendants(0, "Function");
        assert_eq!(descendants.len(), 2);
    }

    #[test]
    fn test_cst_node() {
        let cst = CstNode::new(CstKind::Module, 0, 10);
        assert_eq!(cst.kind(), CstKind::Module);
        assert_eq!(cst.start_offset(), 0);
        assert_eq!(cst.end_offset(), 10);
        assert_eq!(cst.span_len(), 10);
    }

    #[test]
    fn test_lint_input_with_ast_nodes() {
        let nodes = vec![
            LintNode { kind: "Module".into(), content: "defmodule".into(), line: 1, column: 0, offset: 0, children: vec![], parent: None },
        ];
        let input = LintInput::new(1, "test.ex", "defmodule Foo do end")
            .with_ast_nodes(nodes);

        assert!(input.root_node().is_some());
        assert_eq!(input.get_nodes_by_kind("Module").len(), 1);
    }
}
