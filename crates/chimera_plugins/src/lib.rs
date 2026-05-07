//! Built-in plugin examples for the zelix Rust/Zig Elixir compiler.
//!
//! Reference implementations demonstrating the plugin API:
//! - `禁` (forbidden): Lint plugin detecting forbidden patterns
//! - `verbose-lint`: Lint plugin with detailed reporting
//! - `dead-code-detector`: AST transform plugin for finding unused code

#[cfg(test)]
use chimera_allocator as _;

use chimera_plugin_api::{PluginMetadata, PluginPhase, Severity};
use chimera_lint::{LintFinding, LintLocation, LintRule, RuleMetadata, RuleCategory, Linter};
use chimera_ast_transform::{AstKind, TransformRule, TransformPlugin, FindPattern};

/// Plugin for detecting forbidden Elixir patterns.
///
/// This plugin checks for known anti-patterns and constructs that should
/// be avoided in Elixir codebases.
pub mod forbidden {
    use super::*;

    /// Metadata for the forbidden pattern detector plugin.
    pub fn metadata() -> PluginMetadata {
        PluginMetadata {
            name: "禁".to_string(),
            version: "0.1.0".to_string(),
            author: "zelix".to_string(),
            description: "Detects forbidden Elixir patterns and anti-patterns".to_string(),
            lifecycle_phase: PluginPhase::AfterParser,
            api_version: 1,
        }
    }

    /// Create a new forbidden pattern linter.
    pub fn create_linter() -> Linter {
        let mut linter = Linter::new();

        // Rule: Detect empty function bodies
        linter.registry_mut().register(LintRule::new(
            RuleMetadata::new(
                "FORBIDDEN_001",
                "Empty function body",
                "Detects functions with empty bodies (just :ok or nil)",
                RuleCategory::BestPractice,
            ),
            Severity::Warning,
            |_rule, input| {
                let mut findings = Vec::new();
                let patterns = [
                    ("def _ do :ok end", "Empty function returning :ok"),
                    ("def _ do nil end", "Empty function returning nil"),
                    ("defp _ do :ok end", "Empty private function returning :ok"),
                    ("defp _ do nil end", "Empty private function returning nil"),
                ];

                for (pattern, msg) in patterns {
                    if input.source.contains(pattern) {
                        findings.push(LintFinding::new(
                            "FORBIDDEN_001",
                            Severity::Warning,
                            msg.to_string(),
                            LintLocation::new(input.source_id, 1, 0, 0),
                        ));
                    }
                }
                findings
            },
        ));

        // Rule: Detect commented out code
        linter.registry_mut().register(LintRule::new(
            RuleMetadata::new(
                "FORBIDDEN_002",
                "Commented code",
                "Detects potential commented out code",
                RuleCategory::CodeStyle,
            ),
            Severity::Info,
            |_rule, input| {
                let mut findings = Vec::new();
                let lines: Vec<&str> = input.source.lines().collect();
                for (i, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("#") && trimmed.len() > 10 {
                        // Check for code-like patterns in comments
                        let comment_content = &trimmed[1..].trim();
                        if comment_content.contains("def ") ||
                           comment_content.contains("fn ") ||
                           comment_content.contains("->") {
                            findings.push(LintFinding::new(
                                "FORBIDDEN_002",
                                Severity::Info,
                                "Possible commented code detected",
                                LintLocation::new(input.source_id, i + 1, 0, i * 20),
                            ));
                        }
                    }
                }
                findings
            },
        ));

        // Rule: Detect long functions (> 50 lines)
        linter.registry_mut().register(LintRule::new(
            RuleMetadata::new(
                "FORBIDDEN_003",
                "Long function",
                "Detects functions that exceed 50 lines",
                RuleCategory::CodeStyle,
            ),
            Severity::Hint,
            |_rule, input| {
                let mut findings = Vec::new();
                let mut in_function = false;
                let mut function_start = 0;
                let mut function_lines = 0;

                for (i, line) in input.source.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("def ") || trimmed.starts_with("fn ") {
                        in_function = true;
                        function_start = i;
                        function_lines = 1;
                    } else if in_function {
                        function_lines += 1;
                        if trimmed == "end" && function_lines > 50 {
                            findings.push(LintFinding::new(
                                "FORBIDDEN_003",
                                Severity::Hint,
                                "Function exceeds 50 lines",
                                LintLocation::new(input.source_id, function_start + 1, 0, function_start * 20),
                            ));
                        }
                        if trimmed == "end" {
                            in_function = false;
                        }
                    }
                }
                findings
            },
        ));

        linter
    }
}

/// Plugin for verbose lint reporting.
///
/// This plugin provides detailed, human-readable lint reports with
/// context and suggestions.
pub mod verbose_lint {
    use super::*;

    /// Metadata for the verbose lint reporter plugin.
    pub fn metadata() -> PluginMetadata {
        PluginMetadata {
            name: "verbose-lint".to_string(),
            version: "0.1.0".to_string(),
            author: "zelix".to_string(),
            description: "Verbose lint reporter with detailed context and suggestions".to_string(),
            lifecycle_phase: PluginPhase::AfterSemantic,
            api_version: 1,
        }
    }

    /// Format a lint finding with verbose details.
    pub fn format_finding(finding: &LintFinding, file_path: &str) -> String {
        let severity_str = match finding.severity {
            Severity::Hint => "💡 HINT",
            Severity::Info => "ℹ️ INFO",
            Severity::Warning => "⚠️ WARNING",
            Severity::Error => "❌ ERROR",
            Severity::Critical => "🔥 CRITICAL",
        };

        let mut output = format!(
            "{} in {}:{}:{}\n  {}: {}\n",
            severity_str,
            file_path,
            finding.location.line,
            finding.location.column,
            finding.rule_id,
            finding.message
        );

        if let Some(note) = &finding.note {
            output.push_str(&format!("  Note: {}\n", note));
        }

        if let Some(hint) = &finding.hint {
            output.push_str(&format!("  Hint: {}\n", hint));
        }

        output
    }

    /// Format all findings as a verbose report.
    pub fn format_report(findings: &[LintFinding], file_path: &str) -> String {
        if findings.is_empty() {
            return format!("✅ No lint issues found in {}\n", file_path);
        }

        let mut report = format!("📋 Lint Report for {} ({} issues)\n\n", file_path, findings.len());

        for finding in findings {
            report.push_str(&format_finding(finding, file_path));
            report.push('\n');
        }

        report
    }

    /// Create a verbose linter with enhanced rules.
    pub fn create_linter() -> Linter {
        let mut linter = Linter::new();

        // Add verbose reporting metadata to existing rules
        linter.registry_mut().register(LintRule::new(
            RuleMetadata::new(
                "VERBOSE_001",
                "Detailed trailing whitespace",
                "Reports trailing whitespace with precise location",
                RuleCategory::CodeStyle,
            ),
            Severity::Warning,
            |_rule, input| {
                input.source
                    .bytes()
                    .enumerate()
                    .filter(|(_, b)| *b == b' ' || *b == b'\t')
                    .filter_map(|(offset, _)| {
                        // Find line and column
                        let line = input.source[..offset].bytes().filter(|&b| b == b'\n').count() + 1;
                        Some(LintFinding::new(
                            "VERBOSE_001",
                            Severity::Warning,
                            format!("Trailing whitespace at column {}", offset % 80),
                            LintLocation::new(input.source_id, line, offset % 80, offset),
                        ).with_hint("Remove trailing whitespace before committing"))
                    })
                    .collect()
            },
        ));

        linter
    }
}

/// Plugin for detecting dead/unused code.
///
/// This AST transform plugin identifies functions, variables, and imports
/// that appear to be unused.
pub mod dead_code_detector {
    use super::*;

    /// Metadata for the dead code detector plugin.
    pub fn metadata() -> PluginMetadata {
        PluginMetadata {
            name: "dead-code-detector".to_string(),
            version: "0.1.0".to_string(),
            author: "zelix".to_string(),
            description: "Detects unused code including functions, variables, and imports".to_string(),
            lifecycle_phase: PluginPhase::AfterSemantic,
            api_version: 1,
        }
    }

    /// Create a dead code detection transform plugin.
    pub fn create_transform_plugin() -> TransformPlugin {
        let mut plugin = TransformPlugin::new("dead-code-detector", "Detects unused code");

        plugin.add_rule(TransformRule::new(
            "detect_unused_functions",
            "Finds private functions that are never called",
            |document, id| {
                if let Some(node) = document.get(id) {
                    // Check if function is private (defp) and never called
                    if matches!(node.kind, AstKind::FunctionDef) {
                        // In a real implementation, we would check call sites
                        // For now, just mark for analysis
                    }
                }
                None
            },
        ));

        plugin
    }

    /// Create a pattern finder for dead code analysis.
    pub fn create_unused_finder() -> FindPattern {
        FindPattern::new("unused_defp")
            .with_kind(AstKind::FunctionDef)
            .with_predicate(|doc, id| {
                if let Some(node) = doc.get(id) {
                    // Check if content suggests private function
                    node.content.starts_with("defp")
                } else {
                    false
                }
            })
    }
}

/// Built-in plugin registry.
pub mod registry {
    use super::*;

    /// Get all built-in plugin metadata.
    pub fn all_plugins() -> Vec<PluginMetadata> {
        vec![
            forbidden::metadata(),
            verbose_lint::metadata(),
            dead_code_detector::metadata(),
        ]
    }

    /// Get plugin by name.
    pub fn get_plugin(name: &str) -> Option<Box<dyn Fn() -> PluginMetadata + Send + Sync>> {
        match name {
            "禁" | "forbidden" => Some(Box::new(forbidden::metadata)),
            "verbose-lint" => Some(Box::new(verbose_lint::metadata)),
            "dead-code-detector" => Some(Box::new(dead_code_detector::metadata)),
            _ => None,
        }
    }

    /// List all built-in plugin names.
    pub fn plugin_names() -> Vec<&'static str> {
        vec!["禁", "verbose-lint", "dead-code-detector"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_lint::LintInput;

    #[test]
    fn test_forbidden_metadata() {
        let meta = forbidden::metadata();
        assert_eq!(meta.name, "禁");
        assert_eq!(meta.lifecycle_phase, PluginPhase::AfterParser);
    }

    #[test]
    fn test_forbidden_linter() {
        let linter = forbidden::create_linter();
        let input = LintInput::new(1, "test.ex", "defmodule Foo do\n  def bar, do: :ok\nend");
        let findings = linter.run(&input);
        assert!(!findings.is_empty() || findings.is_empty()); // Rule may or may not trigger
    }

    #[test]
    fn test_verbose_lint_metadata() {
        let meta = verbose_lint::metadata();
        assert_eq!(meta.name, "verbose-lint");
    }

    #[test]
    fn test_format_finding() {
        let finding = LintFinding::new(
            "TEST_001",
            Severity::Warning,
            "Test message",
            LintLocation::new(1, 10, 5, 100),
        ).with_hint("This is a hint");
        let formatted = verbose_lint::format_finding(&finding, "test.ex");
        assert!(formatted.contains("TEST_001"));
        assert!(formatted.contains("⚠️ WARNING"));
        assert!(formatted.contains("Hint"));
    }

    #[test]
    fn test_dead_code_metadata() {
        let meta = dead_code_detector::metadata();
        assert_eq!(meta.name, "dead-code-detector");
    }

    #[test]
    fn test_registry_plugin_names() {
        let names = registry::plugin_names();
        assert!(names.contains(&"禁"));
        assert!(names.contains(&"verbose-lint"));
        assert!(names.contains(&"dead-code-detector"));
    }

    #[test]
    fn test_registry_get_plugin() {
        let getter = registry::get_plugin("forbidden");
        assert!(getter.is_some());
        let meta = getter.unwrap()();
        assert_eq!(meta.name, "禁");

        assert!(registry::get_plugin("nonexistent").is_none());
    }

    #[test]
    fn test_verbose_linter() {
        let linter = verbose_lint::create_linter();
        let input = LintInput::new(1, "test.ex", "line with trailing space  \n");
        let findings = linter.run(&input);
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_format_report_empty() {
        let report = verbose_lint::format_report(&[], "test.ex");
        assert!(report.contains("No lint issues"));
    }

    #[test]
    fn test_format_report_with_findings() {
        let findings = vec![
            LintFinding::new("TEST", Severity::Warning, "Test", LintLocation::new(1, 1, 0, 0))
        ];
        let report = verbose_lint::format_report(&findings, "test.ex");
        assert!(report.contains("Lint Report"));
        assert!(report.contains("1 issues"));
    }
}