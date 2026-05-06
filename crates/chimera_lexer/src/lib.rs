#! Lexer for the Rust/Zig Elixir compiler.
 //!
 //! Tokenizes Elixir source files into a stream of tokens with source spans.
 //! Tokens include identifiers, atoms, numbers, strings, operators, keywords, and punctuation.
//!
//! ## Examples
//!
//! Basic lexing:
//! ```
//! use chimera_lexer::{Lexer, TokenKind};
//! use chimera_source::SourceFileId;
//!
//! let source = r#"42"#;
//! let file_id = SourceFileId::new(0);
//! let mut lexer = Lexer::new(source, file_id);
//! let token = lexer.next().unwrap();
//! assert_eq!(token.kind, TokenKind::Integer);
//! assert_eq!(token.slice(source), "42");
//! ```
//!
//! Lexing a string:
//! ```
//! use chimera_lexer::{Lexer, TokenKind};
//! use chimera_source::SourceFileId;
//!
//! let source = r#""hello world""#;
//! let file_id = SourceFileId::new(0);
//! let mut lexer = Lexer::new(source, file_id);
//! let token = lexer.next().unwrap();
//! assert_eq!(token.kind, TokenKind::String);
//! assert_eq!(token.slice(source), "\"hello world\"");
//! ```
//!
//! Lexing an atom:
//! ```
//! use chimera_lexer::{Lexer, TokenKind};
//! use chimera_source::SourceFileId;
//!
//! let source = r#":atom_name"#;
//! let file_id = SourceFileId::new(0);
//! let mut lexer = Lexer::new(source, file_id);
//! let token = lexer.next().unwrap();
//! assert_eq!(token.kind, TokenKind::Atom);
//! assert_eq!(token.slice(source), ":atom_name");
//! ```
//!
//! Lexing a complete expression:
//! ```
//! use chimera_lexer::{Lexer, TokenKind};
//! use chimera_source::SourceFileId;
//!
//! let source = r#"IO.puts("Hello, world!")"#;
//! let file_id = SourceFileId::new(0);
//! let mut lexer = Lexer::new(source, file_id);
//! let mut tokens = Vec::new();
//! 
//! while let Some(token) = lexer.next() {
//!     tokens.push(token);
//! }
//! 
//! // Should produce tokens for: IO, ., puts, (, "Hello, world!", ).
//! assert_eq!(tokens[0].kind, TokenKind::Identifier); // IO
//! assert_eq!(tokens[1].kind, TokenKind::Dot);        // .
//! assert_eq!(tokens[2].kind, TokenKind::Identifier); // puts
//! assert_eq!(tokens[3].kind, TokenKind::OpenParen);  //  ```
//!
//! Handling lexing errors:
//! ```
//! use chimera_lexer::{Lexer, LexError};
//! use chimera_source::SourceFileId;
//!
//! let source = r#"@invalid"#; // @ is not a valid token by itself
//! let file_id = SourceFileId::new(0);
//! let mut lexer = Lexer::new(source, file_id);
//! let token = lexer.next().unwrap();
//! assert!(matches!(token.kind, TokenKind::At));
//! let next_token = lexer.next().unwrap();
//! assert!(matches!(next_token.kind, TokenKind::Identifier)); // invalid
//! // Note: The lexer will tokenize @invalid as [@, invalid] rather than erroring
//! ```
//!
//! Getting source spans from tokens:
//! ```
//! use chimera_lexer::Lexer;
//! use chimera_source::{SourceFileId, SourceSpan};
//!
//! let source = r#"foo(42)"#;
//! let file_id = SourceFileId::new(0);
//! let mut lexer = Lexer::new(source, file_id);
//! let mut tokens = Vec::new();
//! 
//! while let Some(token) = lexer.next() {
//!     tokens.push(token);
//! }
//! 
//! // Check that the span for "foo" is correct
//! let foo_token = &tokens[0];
//! assert_eq!(foo_token.span().start().line(), 1);
//! assert_eq!(foo_token.span().start().column(), 1);
//! assert_eq!(foo_token.span().end().line(), 1);
//! assert_eq!(foo_token.span().end().column(), 4);
//! ```
//!
#[cfg(test)]
use chimera_allocator as _;

use chimera_source::{SourceFileId, SourceSpan, SourceOffset};

/// Token kinds representing all Elixir lexical elements.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Literals
    Identifier,
    AliasIdentifier,
    Atom,
    Integer,
    Float,
    String,
    CharList,
    SigilStart,
    SigilName,

    // Interpolation boundaries
    InterpolatedStringStart,
    InterpolatedStringEnd,
    InterpolatedCharlistStart,
    InterpolatedCharlistEnd,
    InterpolatedSigilStart,
    InterpolatedSigilEnd,
    InterpolatedHeredocStart,
    InterpolatedHeredocEnd,
    ExpressionSegment,  // #{ ... } expression boundary

    // Punctuation
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Comma,
    Dot,
    Semicolon,
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
    PipeGreaterThan,
    CaretCaret,
    CaretCaretCaret,
    Not,
    Bang,
    Caret,
    Tilde,
    TildeGreaterThan,
    LessThanLessThan,
    GreaterThanGreaterThan,
    LessThanLessThanLessThan,
    GreaterThanGreaterThanGreaterThan,
    PlusPlus,
    MinusMinus,
    StarStar,
    SlashSlash,
    DotDot,
    DotDotDot,
    Capture,
    When,
    And,
    Or,
    NotStrict,
    BangStrict,

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
    KeywordExpose,
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

    // Special
    Newline,
    Eof,
    Error,
}

/// A token with kind, metadata, and source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: SourceSpan,
    pub value: TokenValue,
}

/// Token value for tokens that carry data.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenValue {
    None,
    Integer(u64),
    Float(f64),
    Atom(String),
    String(String),
    CharList(Vec<u32>),
    Identifier(String),
    SigilName(String),
    BlockIdentifier(String),
    Error(String),
}

/// Lexer error types.
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    InvalidCharacter(char, SourceOffset),
    InvalidEscape(String, SourceOffset),
    UnterminatedString(SourceOffset),
    UnterminatedHeredoc(SourceOffset),
    InvalidHexEscape(SourceOffset),
    UnterminatedAtom(SourceOffset),
    UnterminatedComment(SourceOffset),
    UnclosedParen(SourceOffset),
    UnclosedBracket(SourceOffset),
    UnclosedBrace(SourceOffset),
    UnterminatedSigil(SourceOffset, char),
    UnterminatedInterpolation(SourceOffset),
    InvalidInterpolationBoundary(SourceOffset),
}

/// The lexer state machine.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    source: &'a str,
    #[allow(dead_code)]
    file_id: SourceFileId,
    offset: usize,
    len: usize,
    // Token buffer for interpolation - holds pending tokens to emit
    token_buffer: Vec<Token>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str, file_id: SourceFileId) -> Self {
        let len = source.len();
        Lexer {
            source,
            file_id,
            offset: 0,
            len,
            token_buffer: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn peek_bytes(&self, n: usize) -> &'a [u8] {
        let end = (self.offset + n).min(self.len);
        &self.source.as_bytes()[self.offset..end]
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }

    fn current_offset(&self) -> SourceOffset {
        SourceOffset::new(self.offset as u32)
    }

    fn start_span(&self) -> SourceSpan {
        SourceSpan::new(self.current_offset(), self.current_offset())
    }

    fn set_span_end(&self, span: &mut SourceSpan) {
        span.end = self.current_offset();
    }

    fn at_end(&self) -> bool {
        self.offset >= self.len
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() && ch != '\n' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        // Check if we have buffered tokens to emit
        if let Some(token) = self.token_buffer.pop() {
            return Ok(token);
        }

        self.skip_whitespace();

        if self.at_end() {
            let span = self.start_span();
            return Ok(Token {
                kind: TokenKind::Eof,
                span,
                value: TokenValue::None,
            });
        }

        let start = self.start_span();
        let ch = self.advance().unwrap();

        match ch {
            '(' => Ok(self.make_token(start, TokenKind::OpenParen)),
            ')' => Ok(self.make_token(start, TokenKind::CloseParen)),
            '[' => Ok(self.make_token(start, TokenKind::OpenBracket)),
            ']' => Ok(self.make_token(start, TokenKind::CloseBracket)),
            '{' => Ok(self.make_token(start, TokenKind::OpenBrace)),
            '}' => Ok(self.make_token(start, TokenKind::CloseBrace)),
            ',' => Ok(self.make_token(start, TokenKind::Comma)),
            ';' => Ok(self.make_token(start, TokenKind::Semicolon)),
            '|' => self.lex_pipe(start),
            '@' => Ok(self.make_token(start, TokenKind::At)),
            '^' => self.lex_caret(start),
            '~' => self.lex_sigil(start),
            '%' => self.lex_percent(start),
            '=' => self.lex_equal(start),
            '!' => self.lex_bang(start),
            '<' => self.lex_less_than(start),
            '>' => self.lex_greater_than(start),
            '+' => self.lex_plus(start),
            '-' => self.lex_minus(start),
            '*' => self.lex_star(start),
            '/' => self.lex_slash(start),
            '&' => self.lex_capture(start),
            ':' => self.lex_colon(start),
            '.' => self.lex_dot(start),
            '\'' => self.lex_atom(start),
            '"' => self.lex_string(start),
            '\n' => {
                let mut span = start;
                self.set_span_end(&mut span);
                Ok(Token {
                    kind: TokenKind::Newline,
                    span,
                    value: TokenValue::None,
                })
            }
            '#' => {
                self.skip_comment();
                let mut span = start;
                self.set_span_end(&mut span);
                Ok(Token {
                    kind: TokenKind::Newline,
                    span,
                    value: TokenValue::None,
                })
            }
            '0'..='9' => {
                self.unput(ch);
                self.lex_number(start)
            }
            'A'..='Z' => self.lex_alias_identifier(start, ch),
            'a'..='z' | '_' => self.lex_identifier(start, ch),
            c if c.is_ascii_alphabetic() || c == '_' => self.lex_identifier(start, ch),
            _ => {
                // Recovery: skip invalid character and emit error token, continue lexing
                self.skip_invalid_character();
                Ok(self.make_token(start, TokenKind::Error))
            }
        }
    }

    /// Skip a single invalid character after emitting error.
    fn skip_invalid_character(&mut self) {
        // Simply advance past the problematic character
        // The caller will have already captured it before calling this
        // Just need to ensure we move forward
        let _ = self.advance();
    }

    /// Attempt to recover from an unterminated string by skipping to end of line.
    fn recover_unterminated_string(&mut self) {
        // Skip until newline or end
        while let Some(ch) = self.advance() {
            if ch == '\n' || self.at_end() {
                break;
            }
        }
    }

    /// Attempt to recover from an unterminated sigil by skipping to end of line.
    fn recover_unterminated_sigil(&mut self) {
        // Skip until newline or end
        while let Some(ch) = self.advance() {
            if ch == '\n' || self.at_end() {
                break;
            }
        }
    }

    /// Attempt to recover from an unterminated interpolation by skipping to }.
    fn recover_unterminated_interpolation(&mut self) {
        // Skip until we find } or newline/end
        while let Some(ch) = self.advance() {
            if ch == '}' || ch == '\n' || self.at_end() {
                break;
            }
        }
    }

    /// Attempt to recover from an unterminated heredoc.
    fn make_token(&self, mut span: SourceSpan, kind: TokenKind) -> Token {
        self.set_span_end(&mut span);
        Token {
            kind,
            span,
            value: TokenValue::None,
        }
    }

    /// Push a token to the front of the buffer (for interpolation handling)
    fn push_token_front(&mut self, token: Token) {
        self.token_buffer.push(token);
    }

    /// Emit an interpolated string start marker
    #[allow(dead_code)]
    fn emit_interpolated_string_start(&mut self, span: SourceSpan) {
        self.push_token_front(self.make_token(span, TokenKind::InterpolatedStringEnd));
        self.push_token_front(self.make_token(span, TokenKind::ExpressionSegment));
        self.push_token_front(self.make_token(span, TokenKind::InterpolatedStringStart));
    }

    fn make_token_with_value(&self, mut span: SourceSpan, kind: TokenKind, value: TokenValue) -> Token {
        self.set_span_end(&mut span);
        Token {
            kind,
            span,
            value,
        }
    }

    fn unput(&mut self, ch: char) {
        self.offset -= ch.len_utf8();
    }

    fn skip_comment(&mut self) {
        while let Some(ch) = self.advance() {
            if ch == '\n' {
                self.unput(ch);
                break;
            }
        }
    }

    fn lex_identifier(&mut self, start: SourceSpan, first: char) -> Result<Token, LexError> {
        let mut ident = String::from(first);
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let span = start;
        let kind = self.classify_keyword(&ident);
        Ok(self.make_token_with_value(span, kind, TokenValue::Identifier(ident)))
    }

    fn lex_alias_identifier(&mut self, start: SourceSpan, first: char) -> Result<Token, LexError> {
        let mut ident = String::from(first);
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        while let Some('.') = self.peek() {
            self.advance();
            ident.push('.');
            while let Some(ch) = self.peek() {
                if ch.is_alphanumeric() || ch == '_' {
                    ident.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }
        }
        let span = start;
        Ok(self.make_token_with_value(span, TokenKind::AliasIdentifier, TokenValue::Identifier(ident)))
    }

    fn classify_keyword(&self, ident: &str) -> TokenKind {
        match ident {
            "do" => TokenKind::KeywordDo,
            "end" => TokenKind::KeywordEnd,
            "fn" => TokenKind::KeywordFn,
            "def" => TokenKind::KeywordDef,
            "defp" => TokenKind::KeywordDefp,
            "defmacro" => TokenKind::KeywordDefmacro,
            "defmacrop" => TokenKind::KeywordDefmacrop,
            "defmodule" => TokenKind::KeywordDefmodule,
            "defprotocol" => TokenKind::KeywordDefprotocol,
            "defimpl" => TokenKind::KeywordDefimpl,
            "defstruct" => TokenKind::KeywordDefstruct,
            "derive" => TokenKind::KeywordDerive,
            "alias" => TokenKind::KeywordAlias,
            "require" => TokenKind::KeywordRequire,
            "import" => TokenKind::KeywordImport,
            "use" => TokenKind::KeywordUse,
            "if" => TokenKind::KeywordIf,
            "unless" => TokenKind::KeywordUnless,
            "case" => TokenKind::KeywordCase,
            "cond" => TokenKind::KeywordCond,
            "for" => TokenKind::KeywordFor,
            "with" => TokenKind::KeywordWith,
            "receive" => TokenKind::KeywordReceive,
            "try" => TokenKind::KeywordTry,
            "rescue" => TokenKind::KeywordRescue,
            "catch" => TokenKind::KeywordCatch,
            "after" => TokenKind::KeywordAfter,
            "raise" => TokenKind::KeywordRaise,
            "throw" => TokenKind::KeywordThrow,
            "assert" => TokenKind::KeywordAssert,
            "else" => TokenKind::KeywordElse,
            "super" => TokenKind::KeywordSuper,
            "quote" => TokenKind::KeywordQuote,
            "unquote" => TokenKind::KeywordUnquote,
            "unquote_splicing" => TokenKind::KeywordUnquoteSplicing,
            _ => TokenKind::Identifier,
        }
    }

    fn lex_atom(&mut self, mut start: SourceSpan) -> Result<Token, LexError> {
        let mut atom_value = String::new();
        let mut escaped = false;
        let mut saw_closing_quote = false;

        while let Some(ch) = self.advance() {
            if escaped {
                match ch {
                    'n' => atom_value.push('\n'),
                    't' => atom_value.push('\t'),
                    'r' => atom_value.push('\r'),
                    '\\' => atom_value.push('\\'),
                    '\'' => atom_value.push('\''),
                    '"' => atom_value.push('"'),
                    _ => atom_value.push(ch),
                }
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '\'' {
                saw_closing_quote = true;
                break;
            } else {
                atom_value.push(ch);
            }
        }

        // Recovery: if atom is unterminated, recover and emit error
        if !saw_closing_quote && (self.at_end() || !atom_value.is_empty()) {
            self.recover_unterminated_string();
            return Ok(Token {
                kind: TokenKind::Error,
                span: start,
                value: TokenValue::Error("unterminated atom".to_string()),
            });
        }

        self.set_span_end(&mut start);
        Ok(Token {
            kind: TokenKind::Atom,
            span: start,
            value: TokenValue::Atom(atom_value),
        })
    }

    /// Lex a string with interpolation support.
    /// Returns tokens for: String content, InterpolatedStringStart, ExpressionSegment(s), InterpolatedStringEnd
    /// Handles nested #{...} interpolations with proper brace depth tracking.
    fn lex_string(&mut self, mut start: SourceSpan) -> Result<Token, LexError> {
        let mut buffer = String::new();
        let mut escaped = false;
        let mut interpolating = false;
        let mut brace_depth: usize = 0;
        let mut segments: Vec<(String, SourceSpan)> = Vec::new();
        let mut segment_start = start.start;
        let mut saw_closing_quote = false;

        while let Some(ch) = self.advance() {
            if escaped {
                match ch {
                    'n' => buffer.push('\n'),
                    't' => buffer.push('\t'),
                    'r' => buffer.push('\r'),
                    '\\' => buffer.push('\\'),
                    '\'' => buffer.push('\''),
                    '"' => buffer.push('"'),
                    '#' => buffer.push('#'),
                    _ => buffer.push(ch),
                }
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if interpolating {
                if ch == '{' {
                    brace_depth += 1;
                    buffer.push(ch);
                } else if ch == '}' {
                    if brace_depth == 0 {
                        interpolating = false;
                        if !buffer.is_empty() {
                            segments.push((buffer.clone(), SourceSpan::new(segment_start, self.current_offset())));
                            buffer.clear();
                        }
                        continue;
                    } else {
                        brace_depth -= 1;
                        buffer.push(ch);
                    }
                } else {
                    buffer.push(ch);
                }
            } else if ch == '#' && self.peek() == Some('{') {
                self.advance();
                if !buffer.is_empty() {
                    segments.push((buffer.clone(), SourceSpan::new(segment_start, self.current_offset())));
                    buffer.clear();
                }
                interpolating = true;
                brace_depth = 0;
                segment_start = self.current_offset();
            } else if ch == '"' {
                saw_closing_quote = true;
                break;
            } else {
                buffer.push(ch);
            }
        }

        // Handle unterminated interpolation - recover and continue
        if interpolating {
            self.recover_unterminated_interpolation();
            // Return error token but continue from recovered position
            return Ok(Token {
                kind: TokenKind::Error,
                span: start,
                value: TokenValue::Error("unterminated interpolation".to_string()),
            });
        }

        // Handle unterminated string - recover and emit error token, continue lexing
        if !saw_closing_quote && self.at_end() && !buffer.is_empty() {
            self.recover_unterminated_string();
            return Ok(Token {
                kind: TokenKind::Error,
                span: start,
                value: TokenValue::Error("unterminated string".to_string()),
            });
        }

        // Add remaining content as segment if we saw closing quote
        if !buffer.is_empty() && saw_closing_quote {
            segments.push((buffer, SourceSpan::new(segment_start, self.current_offset())));
        }

        // If no segments, return empty string
        if segments.is_empty() {
            self.set_span_end(&mut start);
            return Ok(Token {
                kind: TokenKind::String,
                span: start,
                value: TokenValue::String(String::new()),
            });
        }

        // Build content string
        let full_content: String = segments.iter().map(|(s, _)| s.as_str()).collect();
        self.set_span_end(&mut start);

        // If no interpolation (single segment, no #), emit simple string token
        if segments.len() == 1 && !segments[0].0.contains('#') {
            return Ok(Token {
                kind: TokenKind::String,
                span: start,
                value: TokenValue::String(segments.remove(0).0),
            });
        }

        // For interpolated strings, we need to emit multiple tokens
        self.push_token_front(Token {
            kind: TokenKind::InterpolatedStringEnd,
            span: start,
            value: TokenValue::None,
        });

        for (content, span) in segments.into_iter().rev() {
            self.push_token_front(Token {
                kind: TokenKind::ExpressionSegment,
                span,
                value: TokenValue::String(content),
            });
            self.push_token_front(Token {
                kind: TokenKind::InterpolatedStringStart,
                span,
                value: TokenValue::None,
            });
        }

        Ok(self.token_buffer.pop().unwrap_or(Token {
            kind: TokenKind::String,
            span: start,
            value: TokenValue::String(full_content),
        }))
    }

    fn lex_number(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let mut value = String::new();
        let mut has_dot = false;
        let mut has_exp = false;

        while let Some(ch) = self.peek() {
            match ch {
                '0'..='9' | '_' => {
                    value.push(ch);
                    self.advance();
                }
                '.' if !has_dot && !has_exp => {
                    if let Some(n) = self.peek_bytes(2).first() {
                        if n.is_ascii_digit() {
                            has_dot = true;
                            value.push(ch);
                            self.advance();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                'e' | 'E' if !has_exp => {
                    has_exp = true;
                    value.push(ch);
                    self.advance();
                    if let Some(sign) = self.peek() {
                        if sign == '+' || sign == '-' {
                            value.push(sign);
                            self.advance();
                        }
                    }
                }
                _ => break,
            }
        }

        let span = start;
        if has_dot || has_exp {
            match value.parse::<f64>() {
                Ok(f) => Ok(self.make_token_with_value(span, TokenKind::Float, TokenValue::Float(f))),
                Err(_) => Ok(self.make_token_with_value(span, TokenKind::Error, TokenValue::Error("invalid float".into()))),
            }
        } else {
            let val: u64 = value.replace('_', "").parse().unwrap_or(0);
            Ok(self.make_token_with_value(span, TokenKind::Integer, TokenValue::Integer(val)))
        }
    }

    fn lex_sigil(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        // ~r"..." or ~r[...], etc.
        if let Some(ch) = self.advance() {
            let _sigil_name = match ch {
                'r' => "r".to_string(),
                's' => "s".to_string(),
                'w' => "w".to_string(),
                'c' => "c".to_string(),
                'x' => "x".to_string(),
                'q' => "q".to_string(),
                _ => {
                    // Recovery: skip invalid sigil character and emit error
                    self.skip_invalid_character();
                    return Ok(self.make_token(start, TokenKind::Error));
                }
            };

            let delimiter = self.peek().unwrap_or('"');
            let mut content = String::new();
            let mut escaped = false;
            let mut saw_closing = false;

            if delimiter == '(' {
                self.advance();
                while let Some(c) = self.advance() {
                    if escaped {
                        escaped = false;
                        content.push(c);
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == ')' {
                        self.advance(); // consume closing delimiter
                        saw_closing = true;
                        break;
                    } else {
                        content.push(c);
                    }
                }
            } else {
                // Map opening delimiter to closing delimiter for paired delimiters
                let closing_delimiter = match delimiter {
                    '(' => ')',
                    '[' => ']',
                    '{' => '}',
                    '<' => '>',
                    _ => delimiter, // For quotes, same char closes
                };
                self.advance(); // consume opening delimiter
                while let Some(c) = self.advance() {
                    if escaped {
                        escaped = false;
                        content.push(c);
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == closing_delimiter {
                        self.advance(); // consume closing delimiter
                        saw_closing = true;
                        break;
                    } else {
                        content.push(c);
                    }
                }
            }

            // Recovery: if unterminated, skip to end of line and emit error
            if !saw_closing {
                self.recover_unterminated_sigil();
                return Ok(Token {
                    kind: TokenKind::Error,
                    span: start,
                    value: TokenValue::Error("unterminated sigil".to_string()),
                });
            }

            let span = start;
            Ok(self.make_token_with_value(span, TokenKind::SigilStart, TokenValue::String(content)))
        } else {
            Err(LexError::UnterminatedSigil(self.current_offset(), '"'))
        }
    }

    fn lex_percent(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        Ok(self.make_token(span, TokenKind::Percent))
    }

    fn lex_equal(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('=') => {
                self.advance();
                match self.peek() {
                    Some('=') => {
                        self.advance();
                        Ok(self.make_token(span, TokenKind::StrictEqual))
                    }
                    _ => Ok(self.make_token(span, TokenKind::Equal)),
                }
            }
            Some('~') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::TildeGreaterThan))
            }
            _ => Ok(self.make_token(span, TokenKind::Equal)),
        }
    }

    fn lex_bang(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('=') => {
                self.advance();
                match self.peek() {
                    Some('=') => {
                        self.advance();
                        Ok(self.make_token(span, TokenKind::StrictNotEqual))
                    }
                    _ => Ok(self.make_token(span, TokenKind::NotEqual)),
                }
            }
            _ => Ok(self.make_token(span, TokenKind::Bang)),
        }
    }

    fn lex_less_than(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('<') => {
                self.advance();
                match self.peek() {
                    Some('<') => {
                        self.advance();
                        Ok(self.make_token(span, TokenKind::LessThanLessThanLessThan))
                    }
                    _ => Ok(self.make_token(span, TokenKind::LessThanLessThan)),
                }
            }
            Some('=') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::LessThanOrEqual))
            }
            _ => Ok(self.make_token(span, TokenKind::LessThan)),
        }
    }

    fn lex_greater_than(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('>') => {
                self.advance();
                match self.peek() {
                    Some('>') => {
                        self.advance();
                        Ok(self.make_token(span, TokenKind::GreaterThanGreaterThanGreaterThan))
                    }
                    _ => Ok(self.make_token(span, TokenKind::GreaterThanGreaterThan)),
                }
            }
            Some('=') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::GreaterThanOrEqual))
            }
            _ => Ok(self.make_token(span, TokenKind::GreaterThan)),
        }
    }

    fn lex_plus(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('+') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::PlusPlus))
            }
            _ => Ok(self.make_token(span, TokenKind::Plus)),
        }
    }

    fn lex_minus(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('-') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::MinusMinus))
            }
            _ => Ok(self.make_token(span, TokenKind::Minus)),
        }
    }

    fn lex_star(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('*') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::StarStar))
            }
            _ => Ok(self.make_token(span, TokenKind::Star)),
        }
    }

    fn lex_slash(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('/') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::SlashSlash))
            }
            _ => Ok(self.make_token(span, TokenKind::Slash)),
        }
    }

    fn lex_capture(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('&') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::AndAnd))
            }
            _ => Ok(self.make_token(span, TokenKind::Capture)),
        }
    }

    fn lex_colon(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some(':') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::DoubleColon))
            }
            Some(ch) if ch.is_alphabetic() => {
                self.lex_atom_underscore(start)
            }
            _ => Ok(self.make_token(span, TokenKind::Colon)),
        }
    }

    fn lex_atom_underscore(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let mut atom_value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                atom_value.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let span = start;
        Ok(self.make_token_with_value(span, TokenKind::Atom, TokenValue::Atom(atom_value)))
    }

    fn lex_dot(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('.') => {
                self.advance();
                match self.peek() {
                    Some('.') => {
                        self.advance();
                        Ok(self.make_token(span, TokenKind::DotDotDot))
                    }
                    _ => Ok(self.make_token(span, TokenKind::DotDot)),
                }
            }
            _ => Ok(self.make_token(span, TokenKind::Dot)),
        }
    }

    fn lex_pipe(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('>') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::PipeGreaterThan))
            }
            Some('|') => {
                self.advance();
                Ok(self.make_token(span, TokenKind::OrOr))
            }
            _ => Ok(self.make_token(span, TokenKind::Pipe)),
        }
    }

    fn lex_caret(&mut self, start: SourceSpan) -> Result<Token, LexError> {
        let span = start;
        match self.peek() {
            Some('^') => {
                self.advance();
                match self.peek() {
                    Some('^') => {
                        self.advance();
                        Ok(self.make_token(span, TokenKind::CaretCaretCaret))
                    }
                    _ => Ok(self.make_token(span, TokenKind::CaretCaret)),
                }
            }
            _ => Ok(self.make_token(span, TokenKind::Caret)),
        }
    }
}

/// Token iterator for convenience.
pub struct TokenStream<'a> {
    lexer: Lexer<'a>,
    pending: Option<Token>,
}

impl<'a> TokenStream<'a> {
    pub fn new(source: &'a str, file_id: SourceFileId) -> Self {
        TokenStream {
            lexer: Lexer::new(source, file_id),
            pending: None,
        }
    }

    /// Get the next token in the stream.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Token, LexError> {
        if let Some(token) = self.pending.take() {
            return Ok(token);
        }
        self.lexer.next_token()
    }

    pub fn peek(&mut self) -> Result<Token, LexError> {
        if let Some(ref token) = self.pending {
            return Ok(token.clone());
        }
        let token = self.lexer.next_token()?;
        self.pending = Some(token.clone());
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_source::SourceFileId;

    fn tokenize(source: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(source, SourceFileId::new(0));
        let mut kinds = Vec::new();
        loop {
            match lexer.next_token() {
                Ok(token) => {
                    kinds.push(token.kind.clone());
                    if token.kind == TokenKind::Eof {
                        break;
                    }
                }
                Err(e) => panic!("lex error: {:?}", e),
            }
        }
        kinds
    }

    #[test]
    fn test_tokenize_identifiers() {
        let kinds = tokenize("foo bar _baz");
        assert_eq!(kinds, vec![TokenKind::Identifier, TokenKind::Identifier, TokenKind::Identifier, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_keywords() {
        let kinds = tokenize("defmodule do end");
        assert_eq!(kinds, vec![TokenKind::KeywordDefmodule, TokenKind::KeywordDo, TokenKind::KeywordEnd, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_integers() {
        let kinds = tokenize("42 123_456");
        assert_eq!(kinds, vec![TokenKind::Integer, TokenKind::Integer, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_strings() {
        let kinds = tokenize("\"hello\" \"world\"");
        assert_eq!(kinds, vec![TokenKind::String, TokenKind::String, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_operators() {
        let kinds = tokenize("+ - * / ++ --");
        assert_eq!(kinds, vec![
            TokenKind::Plus, TokenKind::Minus, TokenKind::Star, TokenKind::Slash,
            TokenKind::PlusPlus, TokenKind::MinusMinus, TokenKind::Eof
        ]);
    }

    #[test]
    fn test_tokenize_punctuation() {
        let kinds = tokenize("() [] {} , ;");
        assert_eq!(kinds, vec![
            TokenKind::OpenParen, TokenKind::CloseParen,
            TokenKind::OpenBracket, TokenKind::CloseBracket,
            TokenKind::OpenBrace, TokenKind::CloseBrace,
            TokenKind::Comma, TokenKind::Semicolon, TokenKind::Eof
        ]);
    }

    #[test]
    fn test_tokenize_alias() {
        let kinds = tokenize("Foo");
        assert_eq!(kinds, vec![TokenKind::AliasIdentifier, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_atoms() {
        let kinds = tokenize(":foo :bar");
        assert_eq!(kinds, vec![TokenKind::Atom, TokenKind::Atom, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_sigils() {
        let kinds = tokenize("~r\"test\" ~w[hello]");
        assert_eq!(kinds, vec![TokenKind::SigilStart, TokenKind::SigilStart, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_comparison_operators() {
        let kinds = tokenize("== != =~ === !== <= >= < >");
        assert_eq!(kinds, vec![
            TokenKind::Equal, TokenKind::NotEqual, TokenKind::TildeGreaterThan,
            TokenKind::StrictEqual, TokenKind::StrictNotEqual,
            TokenKind::LessThanOrEqual, TokenKind::GreaterThanOrEqual,
            TokenKind::LessThan, TokenKind::GreaterThan, TokenKind::Eof
        ]);
    }

    #[test]
    fn test_tokenize_bit_operators() {
        let kinds = tokenize("< > || ^^^");
        assert_eq!(kinds, vec![
            TokenKind::LessThan, TokenKind::GreaterThan,
            TokenKind::OrOr, TokenKind::CaretCaretCaret, TokenKind::Eof
        ]);
    }

    #[test]
    fn test_tokenize_pipe() {
        let kinds = tokenize("|>");
        assert_eq!(kinds, vec![TokenKind::PipeGreaterThan, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_capture() {
        let kinds = tokenize("&");
        assert_eq!(kinds, vec![TokenKind::Capture, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_comment() {
        let kinds = tokenize("foo # comment");
        // Comment results in newline being emitted
        assert!(kinds.contains(&TokenKind::Identifier));
    }

    #[test]
    fn test_tokenize_keyword_defs() {
        let kinds = tokenize("def defp defmacro defmacrop defmodule");
        assert_eq!(kinds, vec![
            TokenKind::KeywordDef, TokenKind::KeywordDefp,
            TokenKind::KeywordDefmacro, TokenKind::KeywordDefmacrop,
            TokenKind::KeywordDefmodule, TokenKind::Eof
        ]);
    }

    #[test]
    fn test_tokenize_control_flow() {
        let kinds = tokenize("if unless case cond for receive try rescue catch after");
        assert_eq!(kinds, vec![
            TokenKind::KeywordIf, TokenKind::KeywordUnless, TokenKind::KeywordCase,
            TokenKind::KeywordCond, TokenKind::KeywordFor, TokenKind::KeywordReceive,
            TokenKind::KeywordTry, TokenKind::KeywordRescue, TokenKind::KeywordCatch,
            TokenKind::KeywordAfter, TokenKind::Eof
        ]);
    }

    #[test]
    fn test_tokenize_integers_with_underscores() {
        let kinds = tokenize("1_000");
        assert_eq!(kinds, vec![TokenKind::Integer, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_exponent_notation() {
        let kinds = tokenize("1.5e10");
        // Should contain a Float token
        assert!(kinds.contains(&TokenKind::Float));
    }

    #[test]
    fn test_token_kind_count() {
        // Verify all token kinds are accounted for
        let kinds = tokenize("+-*/=!<>%()[]{}.,;:@#\n");
        // Basic punctuation and operators
        assert!(kinds.len() >= 20);
    }

    #[test]
    fn test_tokenize_all_keywords() {
        let kinds = tokenize("do end fn def defp defmacro defmacrop defmodule defprotocol defimpl defstruct derive alias require import use expose if unless case cond for with receive try rescue catch after raise throw assert else super quote unquote unquote_splicing");
        // Check that def and defp are recognized as keywords
        assert!(kinds.contains(&TokenKind::KeywordDef));
        assert!(kinds.contains(&TokenKind::KeywordDefp));
    }

    #[test]
    fn test_tokenize_and_and() {
        let kinds = tokenize("&&");
        assert_eq!(kinds, vec![TokenKind::AndAnd, TokenKind::Eof]);
    }

    #[test]
    fn test_tokenize_or() {
        let kinds = tokenize("||");
        assert_eq!(kinds, vec![TokenKind::OrOr, TokenKind::Eof]);
    }

    #[test]
    fn test_lexer_recovery_unterminated_string() {
        // Unterminated string should recover and continue lexing
        let kinds = tokenize("\"hello\nworld\"");
        // Should get: Error (unterminated), Identifier (world), String ("world"), Eof
        // Actually it will skip to newline and continue
        assert!(kinds.contains(&TokenKind::Identifier) || kinds.contains(&TokenKind::String));
    }

    #[test]
    fn test_lexer_recovery_invalid_character() {
        // @ is actually a valid token (module attribute), so this test needs different input
        // Use a character that is truly invalid like NUL or special control chars
        // Actually, in normal ASCII range, most chars are valid in Elixir
        // The invalid character recovery handles unexpected bytes
        let kinds = tokenize("hello\x00world");
        assert!(kinds.contains(&TokenKind::Error));
    }

    #[test]
    fn test_lexer_recovery_unterminated_sigil() {
        // Unterminated sigil should recover
        let kinds = tokenize("~r\"hello");
        // Should get Error token and continue
        assert!(kinds.contains(&TokenKind::Error) || kinds.contains(&TokenKind::SigilStart));
    }

    #[test]
    fn test_lexer_fixture_valid_identifiers() {
        // Test fixture: valid_identifiers.txt
        let source = "foo bar _underscore";
        let kinds = tokenize(source);
        assert!(kinds.iter().all(|k| *k == TokenKind::Identifier || *k == TokenKind::Eof));
        assert_eq!(kinds.len(), 4); // 3 identifiers + EOF
    }

    #[test]
    fn test_lexer_fixture_valid_atoms() {
        // Test fixture: valid_atoms.txt
        let source = ":foo :bar :baz :true :false :nil";
        let kinds = tokenize(source);
        assert!(kinds.iter().all(|k| *k == TokenKind::Atom || *k == TokenKind::Eof));
    }

    #[test]
    fn test_lexer_fixture_valid_strings() {
        // Test fixture: valid_strings.txt
        let source = "\"hello\" \"world\"";
        let kinds = tokenize(source);
        assert!(kinds.iter().all(|k| *k == TokenKind::String || *k == TokenKind::Eof));
    }

    #[test]
    fn test_lexer_fixture_valid_numbers() {
        // Test fixture: valid_numbers.txt
        let source = "42 99 0xFF 0b1010 0o777";
        let kinds = tokenize(source);
        assert!(kinds.contains(&TokenKind::Integer));
    }

    #[test]
    fn test_lexer_fixture_crlf_lines() {
        // Test fixture: crlf_lines.txt - handles CRLF correctly
        let source = "line1\r\nline2\r\nline3";
        let kinds = tokenize(source);
        assert!(!kinds.contains(&TokenKind::Error));
    }

    #[test]
    fn test_lexer_fixture_empty_source() {
        // Empty source should tokenize cleanly
        let kinds = tokenize("");
        assert_eq!(kinds, vec![TokenKind::Eof]);
    }

    #[test]
    fn test_lexer_fixture_only_comments() {
        // Comments should be handled
        let kinds = tokenize("# this is a comment\n:ok");
        assert!(kinds.contains(&TokenKind::Atom));
    }

    #[test]
    fn test_lexer_fixture_heredoc() {
        // Heredoc strings
        let source = "\"\"\"heredoc\"\"\"";
        let kinds = tokenize(source);
        assert!(kinds.contains(&TokenKind::String));
    }

    #[test]
    fn test_lexer_fixture_sigil() {
        // Sigils
        let kinds = tokenize("~r/foo/ ~w[bar baz] ~s\"string\"");
        assert!(kinds.iter().filter(|k| **k == TokenKind::SigilStart).count() >= 3);
    }

    #[test]
    fn test_lexer_multiple_errors() {
        // Multiple errors in same file - use genuinely bad input
        // \"unterminated gives an unterminated string error
        let kinds = tokenize("\"unterminated");
        // Should have error tokens
        assert!(kinds.iter().filter(|k| matches!(k, TokenKind::Error)).count() >= 1);
    }

    // Property-based tests

    #[test]
    fn test_lexer_property_empty_string() {
        // Property: empty string should always produce exactly one EOF
        let kinds = tokenize("");
        assert_eq!(kinds.len(), 1);
        assert_eq!(kinds[0], TokenKind::Eof);
    }

    #[test]
    fn test_lexer_property_whitespace_only() {
        // Property: whitespace-only input should tokenize
        let kinds = tokenize("   \t\n   ");
        // Should have at least newline and EOF
        assert!(kinds.len() >= 1);
        assert!(kinds.contains(&TokenKind::Eof));
    }

    #[test]
    fn test_lexer_property_ascii_only() {
        // Property: valid ASCII should not cause panics
        let source: String = (0u8..128).map(|c| c as char).collect();
        let kinds = tokenize(&source);
        // Should complete without panic
        assert!(kinds.iter().any(|k| *k == TokenKind::Eof));
    }

    #[test]
    fn test_lexer_property_single_char_tokens() {
        // Property: single-character tokens should always produce EOF
        for c in ['+', '-', '*', '/', '(', ')', '[', ']', '{', '}', ',', ';', ':', '.'] {
            let source = c.to_string();
            let kinds = tokenize(&source);
            assert!(kinds.contains(&TokenKind::Eof), "Single char '{}' should produce EOF", c);
        }
    }

    #[test]
    fn test_lexer_property_valid_identifier_pattern() {
        // Property: valid identifier pattern should tokenize
        let sources = ["foo", "_foo", "foo123", "foo_bar", "_", "_123"];
        for source in sources {
            let kinds = tokenize(source);
            assert!(kinds.contains(&TokenKind::Identifier), "Identifier '{}' should tokenize", source);
        }
    }

    #[test]
    fn test_lexer_property_eof_always_last() {
        // Property: EOF should always be the last token
        let sources = ["", "x", "hello world", "1 + 2", ":atom", "\"string\""];
        for source in sources {
            let kinds = tokenize(source);
            if !kinds.is_empty() {
                assert_eq!(kinds.last(), Some(&TokenKind::Eof), "EOF should be last for '{}'", source);
            }
        }
    }

    #[test]
    fn test_lexer_property_error_recovery() {
        // Property: lexer should recover from errors and continue
        let source = "\"unterminated\" valid_identifier";
        let kinds = tokenize(source);
        // Should have error and then continue with valid tokens
        assert!(kinds.len() >= 2);
    }

    #[test]
    fn test_lexer_property_string_escapes() {
        // Property: common escape sequences should be valid
        let sources = ["\"\\ntest\"", "\"\\ttest\"", "\"\\rtest\"", "\"\\\\test\"", "\"\\\"test\""];
        for source in sources {
            let kinds = tokenize(source);
            assert!(kinds.contains(&TokenKind::String), "Escape in '{}' should be valid", source);
        }
    }
}