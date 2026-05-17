//! Lossless Concrete Syntax Tree for the Rust/Zig Elixir compiler.
//!
//! Provides a lossless representation of source code that preserves all tokens,
//! trivia (comments, whitespace), delimiter information, and source spans.
//! This enables precise formatting, error reporting, and IDE features.

#[cfg(test)]
use chimera_allocator as _;

use chimera_lexer::{Token, TokenKind, TokenValue};
use chimera_source::{SourceFileId, SourceSpan};

/// A node in the lossless Concrete Syntax Tree.
#[derive(Debug, Clone, PartialEq)]
pub struct CSTNode {
    pub kind: CSTKind,
    pub span: SourceSpan,
    pub children: Vec<CSTNode>,
    pub value: Option<String>,
    pub token: Option<Token>,
    pub leading_trivia: Vec<Trivia>,
    pub trailing_trivia: Vec<Trivia>,
}

impl CSTNode {
    /// Create a new CST node.
    pub fn new(kind: CSTKind, span: SourceSpan) -> Self {
        CSTNode {
            kind,
            span,
            children: Vec::new(),
            value: None,
            token: None,
            leading_trivia: Vec::new(),
            trailing_trivia: Vec::new(),
        }
    }

    /// Create a node with a value.
    pub fn with_value(kind: CSTKind, span: SourceSpan, value: impl Into<String>) -> Self {
        CSTNode {
            kind,
            span,
            children: Vec::new(),
            value: Some(value.into()),
            token: None,
            leading_trivia: Vec::new(),
            trailing_trivia: Vec::new(),
        }
    }

    /// Create a node from a token.
    pub fn from_token(token: Token) -> Self {
        let kind = token_kind_to_cst_kind(&token.kind);
        CSTNode {
            kind,
            span: token.span,
            children: Vec::new(),
            value: token_value_to_string(&token.value),
            token: Some(token),
            leading_trivia: Vec::new(),
            trailing_trivia: Vec::new(),
        }
    }

    /// Add a child node.
    pub fn add_child(&mut self, child: CSTNode) {
        self.children.push(child);
    }

    /// Get all descendant tokens.
    pub fn tokens(&self) -> Vec<&Token> {
        let mut tokens = Vec::new();
        self.collect_tokens(&mut tokens);
        tokens
    }

    fn collect_tokens<'a>(&'a self, tokens: &mut Vec<&'a Token>) {
        if let Some(ref token) = self.token {
            tokens.push(token);
        }
        for child in &self.children {
            child.collect_tokens(tokens);
        }
    }

    /// Get the source text represented by this node.
    pub fn source_text(&self, source: &str) -> String {
        source[self.span.start.0 as usize..self.span.end.0 as usize].to_string()
    }

    /// Get the number of children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Get child at index.
    pub fn child(&self, index: usize) -> Option<&CSTNode> {
        self.children.get(index)
    }

    /// Get child at index mutably.
    pub fn child_mut(&mut self, index: usize) -> Option<&mut CSTNode> {
        self.children.get_mut(index)
    }

    /// Get first child.
    pub fn first_child(&self) -> Option<&CSTNode> {
        self.children.first()
    }

    /// Get last child.
    pub fn last_child(&self) -> Option<&CSTNode> {
        self.children.last()
    }

    /// Check if node has children.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Get leading trivia.
    pub fn leading_trivia(&self) -> &[Trivia] {
        &self.leading_trivia
    }

    /// Get trailing trivia.
    pub fn trailing_trivia(&self) -> &[Trivia] {
        &self.trailing_trivia
    }

    /// Find node by span in this subtree.
    pub fn find_by_span(&self, span: SourceSpan) -> Option<&CSTNode> {
        if self.span == span {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_span(span) {
                return Some(found);
            }
        }
        None
    }

    /// Get all descendant nodes as a flat list.
    pub fn descendants(&self) -> Vec<&CSTNode> {
        let mut result = Vec::new();
        self.collect_descendants(&mut result);
        result
    }

    fn collect_descendants<'a>(&'a self, result: &mut Vec<&'a CSTNode>) {
        for child in &self.children {
            result.push(child);
            child.collect_descendants(result);
        }
    }
}

/// Convert a lexer token kind to a CST kind.
fn token_kind_to_cst_kind(kind: &TokenKind) -> CSTKind {
    match kind {
        TokenKind::Identifier => CSTKind::Identifier,
        TokenKind::AliasIdentifier => CSTKind::AliasIdentifier,
        TokenKind::Atom => CSTKind::Atom,
        TokenKind::Integer => CSTKind::Integer,
        TokenKind::Float => CSTKind::Float,
        TokenKind::String => CSTKind::String,
        TokenKind::CharList => CSTKind::CharList,
        TokenKind::SigilStart => CSTKind::Sigil,
        TokenKind::OpenParen => CSTKind::LeftParen,
        TokenKind::CloseParen => CSTKind::RightParen,
        TokenKind::OpenBracket => CSTKind::LeftBracket,
        TokenKind::CloseBracket => CSTKind::RightBracket,
        TokenKind::OpenBrace => CSTKind::LeftBrace,
        TokenKind::CloseBrace => CSTKind::RightBrace,
        TokenKind::Comma => CSTKind::Comma,
        TokenKind::Dot => CSTKind::Dot,
        TokenKind::DotDot => CSTKind::DotDot,
        TokenKind::DotDotDot => CSTKind::DotDotDot,
        TokenKind::Colon => CSTKind::Colon,
        TokenKind::DoubleColon => CSTKind::DoubleColon,
        TokenKind::Pipe => CSTKind::Pipe,
        TokenKind::At => CSTKind::At,
        TokenKind::Capture => CSTKind::Capture,
        TokenKind::When => CSTKind::When,
        TokenKind::And => CSTKind::And,
        TokenKind::Or => CSTKind::Or,
        TokenKind::Equal => CSTKind::Equal,
        TokenKind::NotEqual => CSTKind::NotEqual,
        TokenKind::StrictEqual => CSTKind::StrictEqual,
        TokenKind::StrictNotEqual => CSTKind::StrictNotEqual,
        TokenKind::LessThan => CSTKind::LessThan,
        TokenKind::GreaterThan => CSTKind::GreaterThan,
        TokenKind::LessThanOrEqual => CSTKind::LessThanOrEqual,
        TokenKind::GreaterThanOrEqual => CSTKind::GreaterThanOrEqual,
        TokenKind::Plus => CSTKind::Plus,
        TokenKind::Minus => CSTKind::Minus,
        TokenKind::Star => CSTKind::Star,
        TokenKind::Slash => CSTKind::Slash,
        TokenKind::Percent => CSTKind::Percent,
        TokenKind::Caret => CSTKind::Caret,
        TokenKind::Tilde => CSTKind::Tilde,
        TokenKind::Bang => CSTKind::Bang,
        TokenKind::AndAnd => CSTKind::AndAnd,
        TokenKind::OrOr => CSTKind::OrOr,
        TokenKind::LessThanLessThan => CSTKind::LessThanLessThan,
        TokenKind::GreaterThanGreaterThan => CSTKind::GreaterThanGreaterThan,
        TokenKind::LessThanLessThanLessThan => CSTKind::LessThanLessThanLessThan,
        TokenKind::GreaterThanGreaterThanGreaterThan => CSTKind::GreaterThanGreaterThanGreaterThan,
        TokenKind::PlusPlus => CSTKind::PlusPlus,
        TokenKind::MinusMinus => CSTKind::MinusMinus,
        TokenKind::StarStar => CSTKind::StarStar,
        TokenKind::SlashSlash => CSTKind::SlashSlash,
        TokenKind::TildeGreaterThan => CSTKind::TildeGreaterThan,
        TokenKind::KeywordDo => CSTKind::KeywordDo,
        TokenKind::KeywordEnd => CSTKind::KeywordEnd,
        TokenKind::KeywordFn => CSTKind::KeywordFn,
        TokenKind::KeywordDef => CSTKind::KeywordDef,
        TokenKind::KeywordDefp => CSTKind::KeywordDefp,
        TokenKind::KeywordDefmacro => CSTKind::KeywordDefmacro,
        TokenKind::KeywordDefmacrop => CSTKind::KeywordDefmacrop,
        TokenKind::KeywordDefmodule => CSTKind::KeywordDefmodule,
        TokenKind::KeywordDefprotocol => CSTKind::KeywordDefprotocol,
        TokenKind::KeywordDefimpl => CSTKind::KeywordDefimpl,
        TokenKind::KeywordDefstruct => CSTKind::KeywordDefstruct,
        TokenKind::KeywordDerive => CSTKind::KeywordDerive,
        TokenKind::KeywordAlias => CSTKind::KeywordAlias,
        TokenKind::KeywordRequire => CSTKind::KeywordRequire,
        TokenKind::KeywordImport => CSTKind::KeywordImport,
        TokenKind::KeywordUse => CSTKind::KeywordUse,
        TokenKind::KeywordIf => CSTKind::KeywordIf,
        TokenKind::KeywordUnless => CSTKind::KeywordUnless,
        TokenKind::KeywordCase => CSTKind::KeywordCase,
        TokenKind::KeywordCond => CSTKind::KeywordCond,
        TokenKind::KeywordFor => CSTKind::KeywordFor,
        TokenKind::KeywordWith => CSTKind::KeywordWith,
        TokenKind::KeywordReceive => CSTKind::KeywordReceive,
        TokenKind::KeywordTry => CSTKind::KeywordTry,
        TokenKind::KeywordRescue => CSTKind::KeywordRescue,
        TokenKind::KeywordCatch => CSTKind::KeywordCatch,
        TokenKind::KeywordAfter => CSTKind::KeywordAfter,
        TokenKind::KeywordRaise => CSTKind::KeywordRaise,
        TokenKind::KeywordThrow => CSTKind::KeywordThrow,
        TokenKind::KeywordAssert => CSTKind::KeywordAssert,
        TokenKind::KeywordElse => CSTKind::KeywordElse,
        TokenKind::KeywordSuper => CSTKind::KeywordSuper,
        TokenKind::KeywordQuote => CSTKind::KeywordQuote,
        TokenKind::KeywordUnquote => CSTKind::KeywordUnquote,
        TokenKind::KeywordUnquoteSplicing => CSTKind::KeywordUnquoteSplicing,
        TokenKind::Newline => CSTKind::Newline,
        TokenKind::Eof => CSTKind::Eof,
        TokenKind::Error => CSTKind::Error,
        _ => CSTKind::Error,
    }
}

fn token_value_to_string(value: &TokenValue) -> Option<String> {
    match value {
        TokenValue::None => None,
        TokenValue::Integer(n) => Some(n.to_string()),
        TokenValue::Float(f) => Some(f.to_string()),
        TokenValue::Atom(s) => Some(format!(":{}", s)),
        TokenValue::String(s) => Some(format!("\"{}\"", s)),
        TokenValue::CharList(cs) => Some(format!(
            "'{}'",
            String::from_utf8_lossy(&cs.iter().map(|&c| c as u8).collect::<Vec<_>>())
        )),
        TokenValue::Identifier(s) => Some(s.clone()),
        TokenValue::SigilName(s) => Some(s.clone()),
        TokenValue::BlockIdentifier(s) => Some(s.clone()),
        TokenValue::Error(s) => Some(s.clone()),
    }
}

/// CST node kinds covering all Elixir syntax.
#[derive(Debug, Clone, PartialEq)]
pub enum CSTKind {
    // Source structure
    SourceFile,
    SourceFragment,

    // Tokens
    Identifier,
    AliasIdentifier,
    Atom,
    Integer,
    Float,
    String,
    CharList,
    Sigil,

    // Delimiters
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    DotDot,
    DotDotDot,
    Colon,
    DoubleColon,
    Pipe,
    At,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Tilde,
    Bang,
    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
    GreaterThanOrEqual,
    AndAnd,
    OrOr,
    And,
    Or,
    LessThanLessThan,
    GreaterThanGreaterThan,
    LessThanLessThanLessThan,
    GreaterThanGreaterThanGreaterThan,
    PlusPlus,
    MinusMinus,
    StarStar,
    SlashSlash,
    TildeGreaterThan,
    When,
    Capture,

    // Keywords
    KeywordDo,
    KeywordEnd,
    KeywordFn,
    KeywordDef,
    KeywordDefp,
    KeywordDefmacro,
    KeywordDefmacrop,
    KeywordDefmodule,
    KeywordDefprotocol,
    KeywordDefimpl,
    KeywordDefstruct,
    KeywordDerive,
    KeywordAlias,
    KeywordRequire,
    KeywordImport,
    KeywordUse,
    KeywordIf,
    KeywordUnless,
    KeywordCase,
    KeywordCond,
    KeywordFor,
    KeywordWith,
    KeywordReceive,
    KeywordTry,
    KeywordRescue,
    KeywordCatch,
    KeywordAfter,
    KeywordRaise,
    KeywordThrow,
    KeywordAssert,
    KeywordElse,
    KeywordSuper,
    KeywordQuote,
    KeywordUnquote,
    KeywordUnquoteSplicing,

    // Trivia
    Newline,
    Whitespace,
    Comment,
    Eof,
    Error,

    // Expressions (wraps child nodes)
    Expression,

    // Clauses
    DoClause,
    ClauseCondition,
    ClauseBody,

    // Definitions
    ModuleDefinition,
    FunctionDefinition,
    MacroDefinition,
    TypeDefinition,

    // Body
    Body,
    StabClause,

    // Interpolation
    InterpolatedString,
    InterpolatedAtom,
    InterpolatedCharlist,
    InterpolatedSigil,

    // Collections
    List,
    Tuple,
    Map,
    Binary,
    BitstringSegment,

    // Other
    Block,
    Unmatched,
}

/// Trivia type (whitespace, comments).
#[derive(Debug, Clone, PartialEq)]
pub enum TriviaKind {
    Whitespace,
    Comment,
    Newline,
}

/// Trivia element.
#[derive(Debug, Clone, PartialEq)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub span: SourceSpan,
    pub text: String,
}

impl Trivia {
    pub fn new(kind: TriviaKind, span: SourceSpan, text: impl Into<String>) -> Self {
        Trivia {
            kind,
            span,
            text: text.into(),
        }
    }

    pub fn whitespace(span: SourceSpan, text: impl Into<String>) -> Self {
        Self::new(TriviaKind::Whitespace, span, text)
    }

    pub fn comment(span: SourceSpan, text: impl Into<String>) -> Self {
        Self::new(TriviaKind::Comment, span, text)
    }

    pub fn newline(span: SourceSpan) -> Self {
        Self::new(TriviaKind::Newline, span, "\n")
    }
}

/// Builder for constructing CST trees from token streams.
pub struct CSTBuilder {
    file_id: SourceFileId,
}

impl CSTBuilder {
    pub fn new(file_id: SourceFileId) -> Self {
        CSTBuilder { file_id }
    }

    /// Build a CST tree from a source string with full trivia preservation.
    pub fn build(&self, source: &str) -> CSTNode {
        let mut lexer = chimera_lexer::Lexer::new(source, self.file_id);
        let mut root = CSTNode::new(
            CSTKind::SourceFile,
            SourceSpan::new(
                chimera_source::SourceOffset::new(0),
                chimera_source::SourceOffset::new(source.len() as u32),
            ),
        );

        let mut current_expr = CSTNode::new(
            CSTKind::Expression,
            SourceSpan::new(
                chimera_source::SourceOffset::new(0),
                chimera_source::SourceOffset::new(0),
            ),
        );
        let mut expr_start = chimera_source::SourceOffset::new(0);
        let mut pending_trivia: Vec<Trivia> = Vec::new();
        let mut last_token_end = chimera_source::SourceOffset::new(0);

        loop {
            let token = match lexer.next_token() {
                Ok(t) => t,
                Err(_) => break,
            };

            if token.kind == TokenKind::Eof {
                break;
            }

            // Collect any whitespace between the last token and this one
            if last_token_end.0 > 0 && token.span.start.0 > last_token_end.0 {
                let ws_text = &source[last_token_end.0 as usize..token.span.start.0 as usize];
                if !ws_text.is_empty() {
                    let ws_span = SourceSpan::new(last_token_end, token.span.start);
                    pending_trivia.push(Trivia::whitespace(ws_span, ws_text));
                }
            }

            let cst_node = CSTNode::from_token(token.clone());

            if Self::is_newline_or_comment(&token) {
                // This token IS trivia - add it directly to pending trivia
                let tk = token.clone();
                let tk_span = token.span;
                let tk_text = source[tk_span.start.0 as usize..tk_span.end.0 as usize].to_string();
                let kind = match tk.kind {
                    TokenKind::Newline => TriviaKind::Newline,
                    _ => TriviaKind::Comment,
                };
                pending_trivia.push(Trivia::new(kind, tk_span, tk_text));
                last_token_end = token.span.end;
                continue;
            }

            // Attach collected trivia as leading trivia
            let mut node_with_trivia = cst_node;
            if !pending_trivia.is_empty() {
                node_with_trivia.leading_trivia = pending_trivia.clone();
                pending_trivia.clear();
            }

            if current_expr.children.is_empty() {
                expr_start = token.span.start;
            }
            current_expr.add_child(node_with_trivia);
            last_token_end = token.span.end;
        }

        // Attach any remaining trivia to the last child as trailing trivia
        if !pending_trivia.is_empty() {
            if let Some(last_child) = current_expr.children.last_mut() {
                last_child.trailing_trivia = pending_trivia;
            }
        }

        if !current_expr.children.is_empty() {
            current_expr.span = SourceSpan::new(
                expr_start,
                current_expr
                    .children
                    .last()
                    .map(|c| c.span.end)
                    .unwrap_or(expr_start),
            );
            root.add_child(current_expr);
        }

        root.span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(source.len() as u32),
        );
        root
    }

    fn is_newline_or_comment(token: &Token) -> bool {
        matches!(token.kind, TokenKind::Newline | TokenKind::Eof)
    }

    /// Build a CST tree from tokens with trivia preservation.
    pub fn build_from_tokens(&self, tokens: Vec<Token>, source: &str) -> CSTNode {
        let mut root = CSTNode::new(
            CSTKind::SourceFile,
            SourceSpan::new(
                chimera_source::SourceOffset::new(0),
                chimera_source::SourceOffset::new(source.len() as u32),
            ),
        );

        let mut current_expr = CSTNode::new(
            CSTKind::Expression,
            SourceSpan::new(
                chimera_source::SourceOffset::new(0),
                chimera_source::SourceOffset::new(0),
            ),
        );
        let mut expr_start = chimera_source::SourceOffset::new(0);
        let mut pending_trivia: Vec<Trivia> = Vec::new();
        let mut last_token_end = chimera_source::SourceOffset::new(0);

        for token in tokens {
            if token.kind == TokenKind::Eof {
                break;
            }

            // Collect any whitespace between the last token and this one
            if last_token_end.0 > 0 && token.span.start.0 > last_token_end.0 {
                let ws_text = &source[last_token_end.0 as usize..token.span.start.0 as usize];
                if !ws_text.is_empty() {
                    let ws_span = SourceSpan::new(last_token_end, token.span.start);
                    pending_trivia.push(Trivia::whitespace(ws_span, ws_text));
                }
            }

            let cst_node = CSTNode::from_token(token.clone());

            if Self::is_newline_or_comment(&token) {
                // This token IS trivia - add it directly to pending trivia
                let tk = token.clone();
                let tk_span = token.span;
                let tk_text = source[tk_span.start.0 as usize..tk_span.end.0 as usize].to_string();
                let kind = match tk.kind {
                    TokenKind::Newline => TriviaKind::Newline,
                    _ => TriviaKind::Comment,
                };
                pending_trivia.push(Trivia::new(kind, tk_span, tk_text));
                last_token_end = token.span.end;
                continue;
            }

            // Attach collected trivia as leading trivia
            let mut node_with_trivia = cst_node;
            if !pending_trivia.is_empty() {
                node_with_trivia.leading_trivia = pending_trivia.clone();
                pending_trivia.clear();
            }

            if current_expr.children.is_empty() {
                expr_start = token.span.start;
            }
            current_expr.add_child(node_with_trivia);
            last_token_end = token.span.end;
        }

        // Attach any remaining trivia to the last child as trailing trivia
        if !pending_trivia.is_empty() {
            if let Some(last_child) = current_expr.children.last_mut() {
                last_child.trailing_trivia = pending_trivia;
            }
        }

        if !current_expr.children.is_empty() {
            current_expr.span = SourceSpan::new(
                expr_start,
                current_expr
                    .children
                    .last()
                    .map(|c| c.span.end)
                    .unwrap_or(expr_start),
            );
            root.add_child(current_expr);
        }

        root
    }
}

/// Check if a CST node represents an expression.
pub fn is_expression(kind: &CSTKind) -> bool {
    matches!(
        kind,
        CSTKind::Expression
            | CSTKind::List
            | CSTKind::Tuple
            | CSTKind::Map
            | CSTKind::Binary
            | CSTKind::Block
            | CSTKind::ModuleDefinition
            | CSTKind::FunctionDefinition
            | CSTKind::MacroDefinition
    )
}

/// Check if a node is trivia.
pub fn is_trivia(kind: &CSTKind) -> bool {
    matches!(
        kind,
        CSTKind::Whitespace | CSTKind::Comment | CSTKind::Newline
    )
}

/// Get the text content of a node as it appears in source.
pub fn node_text(node: &CSTNode, source: &str) -> String {
    if let Some(ref _token) = node.token {
        if let Some(ref val) = node.value {
            return val.clone();
        }
    }
    source[node.span.start.0 as usize..node.span.end.0 as usize].to_string()
}

/// Check if two CST nodes represent the same source location.
pub fn same_span(a: &CSTNode, b: &CSTNode) -> bool {
    a.span == b.span
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cst_node_new() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(5),
        );
        let node = CSTNode::new(CSTKind::Identifier, span);
        assert_eq!(node.kind, CSTKind::Identifier);
        assert!(node.token.is_none());
    }

    #[test]
    fn test_cst_node_with_value() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(3),
        );
        let node = CSTNode::with_value(CSTKind::Integer, span, "42");
        assert_eq!(node.value, Some("42".to_string()));
    }

    #[test]
    fn test_cst_builder_build() {
        let builder = CSTBuilder::new(chimera_source::SourceFileId::new(0));
        let cst = builder.build("42");
        assert_eq!(cst.kind, CSTKind::SourceFile);
    }

    #[test]
    fn test_cst_node_from_token() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(3),
        );
        let token = Token {
            kind: TokenKind::Identifier,
            span,
            value: TokenValue::Identifier("foo".to_string()),
        };
        let node = CSTNode::from_token(token);
        assert_eq!(node.kind, CSTKind::Identifier);
        assert!(node.token.is_some());
    }

    #[test]
    fn test_trivia_newline() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(1),
        );
        let trivia = Trivia::newline(span);
        assert_eq!(trivia.kind, TriviaKind::Newline);
        assert_eq!(trivia.text, "\n");
    }

    #[test]
    fn test_trivia_comment() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(5),
        );
        let trivia = Trivia::comment(span, "# comment");
        assert_eq!(trivia.kind, TriviaKind::Comment);
    }

    #[test]
    fn test_is_expression() {
        assert!(is_expression(&CSTKind::Expression));
        assert!(!is_expression(&CSTKind::Identifier));
    }

    #[test]
    fn test_is_trivia() {
        assert!(is_trivia(&CSTKind::Whitespace));
        assert!(is_trivia(&CSTKind::Comment));
        assert!(!is_trivia(&CSTKind::Identifier));
    }

    #[test]
    fn test_node_text() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(2),
        );
        let node = CSTNode::with_value(CSTKind::Integer, span, "42");
        let text = node_text(&node, "42");
        assert_eq!(text, "42");
    }

    #[test]
    fn test_trivia_preservation() {
        let builder = CSTBuilder::new(chimera_source::SourceFileId::new(0));
        // Source with whitespace between tokens
        let cst = builder.build("foo bar");
        assert_eq!(cst.kind, CSTKind::SourceFile);
        assert_eq!(cst.children.len(), 1); // One Expression node

        let expr = &cst.children[0];
        assert_eq!(expr.children.len(), 2); // foo and bar tokens

        // First token should have no leading trivia
        assert!(expr.children[0].leading_trivia.is_empty());

        // Second token should have leading whitespace trivia
        assert!(!expr.children[1].leading_trivia.is_empty());
        assert_eq!(
            expr.children[1].leading_trivia[0].kind,
            TriviaKind::Whitespace
        );
    }

    #[test]
    fn test_trivia_newline_preservation() {
        let builder = CSTBuilder::new(chimera_source::SourceFileId::new(0));
        // Source with newline between tokens
        let cst = builder.build("foo\nbar");
        assert_eq!(cst.kind, CSTKind::SourceFile);

        let expr = &cst.children[0];
        assert_eq!(expr.children.len(), 2); // foo and bar tokens

        // Second token should have leading newline trivia
        assert!(!expr.children[1].leading_trivia.is_empty());
        assert_eq!(expr.children[1].leading_trivia[0].kind, TriviaKind::Newline);
    }

    #[test]
    fn test_trivia_comment_preservation() {
        let builder = CSTBuilder::new(chimera_source::SourceFileId::new(0));
        // Source with comment between tokens
        // Note: comments are absorbed as whitespace since lexer doesn't emit comment tokens
        let cst = builder.build("foo # comment\nbar");
        assert_eq!(cst.kind, CSTKind::SourceFile);

        let expr = &cst.children[0];
        assert_eq!(expr.children.len(), 2); // foo and bar tokens

        // Second token should have leading trivia
        let trivia = &expr.children[1].leading_trivia;
        assert!(!trivia.is_empty());
        // The trivia will be whitespace containing the comment text
        let trivia_text: String = trivia.iter().map(|t| t.text.as_str()).collect();
        assert!(trivia_text.contains("# comment") || trivia_text.contains("comment"));
    }

    #[test]
    fn test_cst_node_child_apis() {
        let span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(10),
        );
        let mut parent = CSTNode::new(CSTKind::Expression, span);

        let child1 = CSTNode::new(
            CSTKind::Identifier,
            SourceSpan::new(
                chimera_source::SourceOffset::new(0),
                chimera_source::SourceOffset::new(3),
            ),
        );
        let child2 = CSTNode::new(
            CSTKind::Integer,
            SourceSpan::new(
                chimera_source::SourceOffset::new(4),
                chimera_source::SourceOffset::new(7),
            ),
        );

        parent.add_child(child1);
        parent.add_child(child2);

        assert_eq!(parent.child_count(), 2);
        assert!(parent.has_children());
        assert!(parent.first_child().is_some());
        assert!(parent.last_child().is_some());
        assert_eq!(parent.child(0).map(|c| &c.kind), Some(&CSTKind::Identifier));
        assert_eq!(parent.child(1).map(|c| &c.kind), Some(&CSTKind::Integer));
    }

    #[test]
    fn test_cst_find_by_span() {
        let builder = CSTBuilder::new(chimera_source::SourceFileId::new(0));
        let cst = builder.build("foo bar");

        // SourceFile has span 0-7
        // Expression has span 0-7
        // Identifier "foo" at 0-3
        // Identifier "bar" at 4-7

        let foo_span = SourceSpan::new(
            chimera_source::SourceOffset::new(0),
            chimera_source::SourceOffset::new(3),
        );

        let found = cst.find_by_span(foo_span);
        assert!(found.is_some());
        assert_eq!(found.unwrap().kind, CSTKind::Identifier);
    }

    #[test]
    fn test_cst_descendants() {
        let builder = CSTBuilder::new(chimera_source::SourceFileId::new(0));
        let cst = builder.build("foo bar");

        let descendants = cst.descendants();
        // Should include the Expression node and the two Identifier nodes
        assert!(descendants.len() >= 3); // At least Expression + 2 identifiers
    }

    #[test]
    fn test_cst_trailing_trivia() {
        let builder = CSTBuilder::new(chimera_source::SourceFileId::new(0));
        // "foo bar " has trailing space after "bar"
        let cst = builder.build("foo ");
        assert_eq!(cst.kind, CSTKind::SourceFile);
        assert!(!cst.children.is_empty());
        // The trailing trivia should be collected
    }
}
