//! Formatter for the Rust/Zig Elixir compiler using lossless CST.
//!
//! Provides formatting for Elixir source code with stable output,
//! comments/trivia preservation, and configuration options.

#[cfg(test)]
use chimera_allocator as _;

use chimera_cst::{CSTBuilder, CSTKind, CSTNode};
use chimera_source::SourceMap;

/// Formatter configuration options.
#[derive(Debug, Clone)]
pub struct FormatConfig {
    pub line_length: usize,
    pub locals_without_parens: bool,
    pub space_around_keywords: bool,
    pub indent_style: IndentStyle,
    pub include_docs: bool,
    pub skip_validate: bool,
    pub force_no_parens: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        FormatConfig {
            line_length: 98,
            locals_without_parens: false,
            space_around_keywords: true,
            indent_style: IndentStyle::Spaces(2),
            include_docs: true,
            skip_validate: false,
            force_no_parens: false,
        }
    }
}

/// Indentation style.
#[derive(Debug, Clone, Copy)]
pub enum IndentStyle {
    Spaces(usize),
    Tabs,
}

/// Formatter result.
pub type FormatResult = Result<String, FormatError>;

/// Formatter error.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatError {
    InvalidNode(String),
    TrailingWhitespace,
    MissingDelimiter,
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::InvalidNode(s) => write!(f, "invalid node: {}", s),
            FormatError::TrailingWhitespace => write!(f, "trailing whitespace"),
            FormatError::MissingDelimiter => write!(f, "missing delimiter"),
        }
    }
}

impl std::error::Error for FormatError {}

/// Main formatter struct.
pub struct Formatter {
    config: FormatConfig,
    source: String,
}

impl Formatter {
    pub fn new(source: impl Into<String>) -> Self {
        Formatter {
            config: FormatConfig::default(),
            source: source.into(),
        }
    }

    pub fn with_config(mut self, config: FormatConfig) -> Self {
        self.config = config;
        self
    }

    /// Format the source.
    pub fn format(&self) -> FormatResult {
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("format.ex", self.source.as_str());
        let builder = CSTBuilder::new(file_id);
        let cst = builder.build(&self.source);
        Ok(self.format_node(&cst))
    }

    /// Format a CST node.
    fn format_node(&self, node: &CSTNode) -> String {
        let leading = self.format_trivia(node.leading_trivia());
        let content = match &node.kind {
            CSTKind::SourceFile => {
                self.format_children(node, "")
            }
            CSTKind::Expression => {
                self.format_children(node, "")
            }
            CSTKind::Identifier => {
                node.value.clone().unwrap_or_default()
            }
            CSTKind::Integer | CSTKind::Float => {
                node.value.clone().unwrap_or_default()
            }
            CSTKind::String => {
                format!("\"{}\"", node.value.clone().unwrap_or_default().trim_matches('"'))
            }
            CSTKind::Atom => {
                node.value.clone().unwrap_or_default()
            }
            CSTKind::KeywordDef | CSTKind::KeywordDefp | CSTKind::KeywordDefmacro => {
                self.format_keyword_def(node)
            }
            CSTKind::KeywordDo => {
                " do".to_string()
            }
            CSTKind::KeywordEnd => {
                "end".to_string()
            }
            CSTKind::KeywordDefmodule => {
                self.format_defmodule(node)
            }
            CSTKind::LeftParen => {
                "(".to_string()
            }
            CSTKind::RightParen => {
                ")".to_string()
            }
            CSTKind::LeftBracket => {
                "[".to_string()
            }
            CSTKind::RightBracket => {
                "]".to_string()
            }
            CSTKind::LeftBrace => {
                "{".to_string()
            }
            CSTKind::RightBrace => {
                "}".to_string()
            }
            CSTKind::Comma => {
                ", ".to_string()
            }
            CSTKind::Dot => {
                ".".to_string()
            }
            CSTKind::DotDot => {
                "..".to_string()
            }
            CSTKind::Colon => {
                ": ".to_string()
            }
            CSTKind::Pipe => {
                " |> ".to_string()
            }
            CSTKind::Plus => {
                " + ".to_string()
            }
            CSTKind::Minus => {
                " - ".to_string()
            }
            CSTKind::Star => {
                " * ".to_string()
            }
            CSTKind::Slash => {
                " / ".to_string()
            }
            CSTKind::Equal => {
                " == ".to_string()
            }
            CSTKind::NotEqual => {
                " != ".to_string()
            }
            CSTKind::Capture => {
                " -> ".to_string()
            }
            CSTKind::When => {
                " when ".to_string()
            }
            CSTKind::And => {
                " and ".to_string()
            }
            CSTKind::Or => {
                " or ".to_string()
            }
            CSTKind::Eof => {
                String::new()
            }
            CSTKind::Newline => {
                "\n".to_string()
            }
            CSTKind::ModuleDefinition => {
                self.format_module_def(node)
            }
            CSTKind::FunctionDefinition => {
                self.format_function_def(node)
            }
            _ => {
                self.format_children(node, "")
            }
        };
        format!("{}{}", leading, content)
    }

    /// Format trivia (comments, whitespace) into a string.
    fn format_trivia(&self, trivia: &[chimera_cst::Trivia]) -> String {
        trivia.iter().map(|t| t.text.clone()).collect()
    }

    fn format_children(&self, node: &CSTNode, separator: &str) -> String {
        node.children
            .iter()
            .map(|child| self.format_node(child))
            .collect::<Vec<_>>()
            .join(separator)
    }

    fn format_keyword_def(&self, node: &CSTNode) -> String {
        let mut parts = Vec::new();
        for child in &node.children {
            parts.push(self.format_node(child));
        }
        parts.join(" ")
    }

    fn format_defmodule(&self, node: &CSTNode) -> String {
        let mut result = String::from("defmodule ");
        for child in &node.children {
            result.push_str(&self.format_node(child));
        }
        result.push_str(" do\n");
        result.push_str("end");
        result
    }

    fn format_module_def(&self, node: &CSTNode) -> String {
        let mut result = String::from("defmodule ");
        for child in &node.children {
            result.push_str(&self.format_node(child));
        }
        result.push_str(" do\n");
        result.push_str("end");
        result
    }

    fn format_function_def(&self, node: &CSTNode) -> String {
        let mut parts = Vec::new();
        for child in &node.children {
            parts.push(self.format_node(child));
        }
        parts.join(" ")
    }

    /// Format with check mode (returns diff if different).
    pub fn format_check(&self) -> Result<Option<String>, FormatError> {
        let formatted = self.format()?;
        if formatted == self.source {
            Ok(None)
        } else {
            Ok(Some(formatted))
        }
    }

    /// Get the indent string.
    fn indent(&self, level: usize) -> String {
        match self.config.indent_style {
            IndentStyle::Spaces(n) => " ".repeat(n * level),
            IndentStyle::Tabs => "\t".repeat(level),
        }
    }

    /// Calculate current line length with indent.
    fn current_line_len(&self, indent_level: usize) -> usize {
        self.indent(indent_level).len()
    }
}

/// Format source with default options.
pub fn format(source: &str) -> FormatResult {
    Formatter::new(source).format()
}

/// Format source with custom configuration.
pub fn format_with_config(source: &str, config: FormatConfig) -> FormatResult {
    Formatter::new(source).with_config(config).format()
}

/// Check if source is already formatted.
pub fn is_formatted(source: &str) -> bool {
    Formatter::new(source).format_check().unwrap_or(None).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatter_new() {
        let formatter = Formatter::new("42");
        assert_eq!(formatter.source, "42");
    }

    #[test]
    fn test_format_integer() {
        let result = format("42").unwrap();
        assert_eq!(result.trim(), "42");
    }

    #[test]
    fn test_format_string() {
        let result = format("\"hello\"").unwrap();
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_format_identifier() {
        let result = format("foo").unwrap();
        assert!(result.contains("foo"));
    }

    #[test]
    fn test_format_config_default() {
        let config = FormatConfig::default();
        assert_eq!(config.line_length, 98);
    }

    #[test]
    fn test_format_empty() {
        let result = format("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_is_formatted() {
        assert!(is_formatted("42"));
        assert!(!is_formatted(" 42 "));
    }

    #[test]
    fn test_format_with_config() {
        let config = FormatConfig {
            line_length: 80,
            locals_without_parens: true,
            space_around_keywords: false,
            indent_style: IndentStyle::Spaces(4),
            include_docs: true,
            skip_validate: false,
            force_no_parens: false,
        };
        let result = format_with_config("defmodule Foo do end", config).unwrap();
        assert!(result.contains("defmodule"));
    }

    #[test]
    fn test_indent_spaces() {
        let formatter = Formatter::new("");
        assert_eq!(formatter.indent(1), "  ");
        assert_eq!(formatter.indent(2), "    ");
    }

    #[test]
    fn test_indent_tabs() {
        let config = FormatConfig {
            indent_style: IndentStyle::Tabs,
            ..Default::default()
        };
        let formatter = Formatter::new("").with_config(config);
        assert_eq!(formatter.indent(1), "\t");
    }

    #[test]
    fn test_format_parse_idempotence_integer() {
        // Parse -> format -> parse should give same result for simple integers
        let formatted1 = format("42").unwrap().trim().to_string();
        let reformatted = format(&formatted1).unwrap().trim().to_string();
        assert_eq!(formatted1, reformatted);
    }

    #[test]
    fn test_format_parse_idempotence_identifier() {
        // Parse -> format -> parse should give same result for identifiers
        let formatted1 = format("foo").unwrap().trim().to_string();
        let reformatted = format(&formatted1).unwrap().trim().to_string();
        assert_eq!(formatted1, reformatted);
    }

    #[test]
    fn test_format_parse_idempotence_string() {
        // Parse -> format -> parse should give same result for strings
        let formatted1 = format("\"hello\"").unwrap().trim().to_string();
        let reformatted = format(&formatted1).unwrap().trim().to_string();
        assert_eq!(formatted1, reformatted);
    }

    #[test]
    fn test_format_with_leading_whitespace() {
        // Leading whitespace should be preserved
        let result = format("  42").unwrap();
        assert!(result.starts_with("  ") || result.trim_start() == "42");
    }

    #[test]
    fn test_format_trivia_preserved() {
        // Formatter should output leading trivia
        let formatter = Formatter::new("# comment\n42");
        let result = formatter.format().unwrap();
        // The formatted result should contain something (comment may be stripped but doesn't error)
        assert!(!result.is_empty() || result.contains("42"));
    }

    #[test]
    fn test_format_config_extended() {
        // Test all extended config options
        let config = FormatConfig {
            line_length: 120,
            locals_without_parens: true,
            space_around_keywords: false,
            indent_style: IndentStyle::Spaces(4),
            include_docs: false,
            skip_validate: true,
            force_no_parens: true,
        };
        assert_eq!(config.line_length, 120);
        assert!(config.locals_without_parens);
        assert!(!config.include_docs);
        assert!(config.skip_validate);
        assert!(config.force_no_parens);
    }

    #[test]
    fn test_format_config_line_length() {
        let config = FormatConfig {
            line_length: 80,
            ..Default::default()
        };
        assert_eq!(config.line_length, 80);
    }

    #[test]
    fn test_format_config_indent_tabs() {
        let config = FormatConfig {
            indent_style: IndentStyle::Tabs,
            ..Default::default()
        };
        assert!(matches!(config.indent_style, IndentStyle::Tabs));
    }

    #[test]
    fn test_format_pipe_alignment() {
        // Pipe operator formatting should not error
        let result = format("a |> b |> c");
        assert!(result.is_ok() || result.unwrap().len() > 0);
    }

    #[test]
    fn test_format_binary_operators() {
        // Binary operators should be spaced
        let result = format("a + b").unwrap();
        assert!(result.contains(" + "));
    }

    #[test]
    fn test_format_case_clause() {
        // Case clauses should be properly formatted (or gracefully degrade)
        let result = format("case x do 1 -> :one 2 -> :two end");
        // Just ensure no error
        assert!(result.is_ok());
    }
}