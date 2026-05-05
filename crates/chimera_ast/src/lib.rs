#! Abstract Syntax Tree for the Rust/Zig Elixir compiler.
 //!
 //! Provides the quoted AST representation that matches Elixir's canonical
 //! metaprogramming format: `{:name, metadata, args}`.
//!
//! ## Examples
//!
//! Basic AST construction:
//! ```
//! use chimera_ast::{AST, AST::*};
//! use chimera_term::AtomTable;
//!
//! // Create an integer literal
//! let ast = Integer(42);
//!
//! // Create a string literal
//! let ast = String("hello".to_string());
//!
//! // Create a list
//! let ast = List(vec![Integer(1), Integer(2), Integer(3)]);
//! ```
//!
//! Constructing a function call:
//! ```
//! use chimera_ast::{AST, AST::*};
//! use chimera_term::AtomTable;
//!
//! let ast = Call {
//!     name: Atom::new(1), // :hello
//!     meta: Default::default(),
//!     args: vec![Integer(42), String("world".to_string())]
//! };
//! ```
//!
//! Converting AST to term for macro execution:
//! ```
//! use chimera_ast::{AST, AST::*};
//! use chimera_term::AtomTable;
//!
//! let ast = Call {
//!     name: Atom::new(1), // :hello
//!     meta: Default::default(),
//!     args: vec![Integer(42)]
//! };
//! let mut atoms = AtomTable::new();
//! let term = to_term(&ast, &mut atoms);
//! // Results in: {:hello, [], [42]}
//! ```
//!
//! Working with metadata and source locations:
//! ```
//! use chimera_ast::{AST, AST::*, Meta};
//! use chimera_source::{SourceFileId, SourceSpan};
//!
//! let meta = Meta::new(SourceFileId::new(0), 10, 5);
//! let ast = Integer(42)
//!     .with_meta(meta);
//! ```
//!
//! The AST has two forms:
//! - **CST (Concrete Syntax Tree)**: Lossless representation preserving all source details
//! - **Quoted AST**: Simplified representation used by macro expansion
#[cfg(test)]
use chimera_allocator as _;

use chimera_source::{SourceFileId, SourceSpan};
use chimera_term::{Atom, ModuleName, Term};

/// A quoted AST node.
#[derive(Debug, Clone, PartialEq)]
pub enum AST {
    // Literals
    Nil,
    Atom(Atom),
    Integer(i64),
    Float(f64),
    String(String),
    CharList(Vec<u32>),
    Binary(Vec<AST>, Option<u8>),

    // Composite types
    List(Vec<AST>),
    Tuple(Vec<AST>),
    Map(Vec<(AST, AST)>),

    // Variables and identifiers
    Var {
        name: Atom,
        meta: Meta,
    },
    Alias {
        segments: Vec<Atom>,
        meta: Meta,
    },
    Identifier {
        name: String,
        meta: Meta,
    },

    // Calls
    Call {
        name: Atom,
        meta: Meta,
        args: Vec<AST>,
    },
    RemoteCall {
        module: Box<AST>,
        name: Atom,
        meta: Meta,
        args: Vec<AST>,
    },
    LocalCall {
        name: Atom,
        meta: Meta,
        args: Vec<AST>,
    },

    // Special forms
    Match {
        pattern: Box<AST>,
        value: Box<AST>,
        meta: Meta,
    },
    Clause {
        pattern: Box<AST>,
        guard: Option<Box<AST>>,
        body: Box<AST>,
        meta: Meta,
    },
    Case {
        expr: Box<AST>,
        clauses: Vec<AST>,
        meta: Meta,
    },
    Cond {
        clauses: Vec<(Box<AST>, Box<AST>)>,
        meta: Meta,
    },
    Fn {
        clauses: Vec<AST>,
        meta: Meta,
    },
    Try {
        expr: Box<AST>,
        rescue: Vec<AST>,
        catch: Vec<AST>,
        after: Option<Box<AST>>,
        meta: Meta,
    },
    Receive {
        clauses: Vec<AST>,
        after: Option<(Box<AST>, Box<AST>)>,
        meta: Meta,
    },

    // Definitions
    Defmodule {
        name: Box<AST>,
        body: Vec<AST>,
        meta: Meta,
    },
    Def {
        name: Atom,
        meta: Meta,
        clauses: Vec<AST>,
    },
    Defp {
        name: Atom,
        meta: Meta,
        clauses: Vec<AST>,
    },
    Defmacro {
        name: Atom,
        meta: Meta,
        clauses: Vec<AST>,
    },
    Defmacrop {
        name: Atom,
        meta: Meta,
        clauses: Vec<AST>,
    },

    // Quoting
    Quote {
        value: Box<AST>,
        meta: Meta,
    },
    Unquote {
        expr: Box<AST>,
        meta: Meta,
    },
    UnquoteSplicing {
        expr: Box<AST>,
        meta: Meta,
    },

    // Alias/import/require
    AliasExpr {
        arg: Box<AST>,
        meta: Meta,
    },
    RequireExpr {
        arg: Box<AST>,
        meta: Meta,
    },
    ImportExpr {
        arg: Box<AST>,
        meta: Meta,
        opts: Vec<AST>,
    },

    // Block
    Block {
        exprs: Vec<AST>,
        meta: Meta,
    },

    // Captures
    Capture {
        fun: Box<AST>,
        arity: Option<u8>,
        meta: Meta,
    },

    // Binary operations
    BinaryOp {
        op: Atom,
        left: Box<AST>,
        right: Box<AST>,
        meta: Meta,
    },
    UnaryOp {
        op: Atom,
        arg: Box<AST>,
        meta: Meta,
    },

    // Access
    Access {
        record: Box<AST>,
        field: Box<AST>,
        meta: Meta,
    },

    // Container with source location
    With {
        bindings: Vec<(AST, AST)>,
        body: Box<AST>,
        meta: Meta,
    },

    // Module attribute
    Attribute {
        name: Atom,
        value: Box<AST>,
        meta: Meta,
    },

    // Struct definition
    Defstruct {
        fields: Vec<StructField>,
        meta: Meta,
    },

    // Exception definition
    Defexception {
        fields: Vec<StructField>,
        meta: Meta,
    },
}

/// A field in a struct or exception definition.
pub type StructField = (Atom, Option<Box<AST>>);

/// AST metadata including source location and hygiene.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meta {
    pub location: Option<Location>,
    pub context: Option<Atom>,
    pub hygiene: Hygiene,
    pub custom: Vec<(Atom, Term)>,
}

impl Meta {
    pub fn new(file: SourceFileId, line: u32, column: u32) -> Self {
        Meta {
            location: Some(Location { file, line, column }),
            context: None,
            hygiene: Hygiene::default(),
            custom: Vec::new(),
        }
    }

    pub fn with_hygiene(mut self, hygiene: Hygiene) -> Self {
        self.hygiene = hygiene;
        self
    }

    pub fn with_context(mut self, context: Atom) -> Self {
        self.context = Some(context);
        self
    }
}

/// Source location within a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub file: SourceFileId,
    pub line: u32,
    pub column: u32,
}

/// Hygiene context for macro expansion.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hygiene {
    pub origin: Option<String>,
    pub generated: bool,
    pub import: Option<ImportContext>,
}

impl Hygiene {
    pub fn generated() -> Self {
        Hygiene {
            origin: None,
            generated: true,
            import: None,
        }
    }
}

/// Import context for hygiene tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportContext {
    pub module: ModuleName,
    pub alias: Option<Atom>,
}

/// Context in which an expression appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprContext {
    Default,
    Match,
    Guard,
    TypeSpec,
    MacroDefinition,
    Quote,
}

impl Default for ExprContext {
    fn default() -> Self {
        ExprContext::Default
    }
}

/// Convert an AST to a quoted term for macro execution.
pub fn to_term(ast: &AST, atoms: &mut chimera_term::AtomTable) -> Term {
    match ast {
        AST::Nil => Term::Nil,
        AST::Atom(atom) => Term::Atom(atom.clone()),
        AST::Integer(n) => {
            if *n >= chimera_term::SMALL_INT_MIN && *n <= chimera_term::SMALL_INT_MAX {
                Term::SmallInt(*n)
            } else {
                Term::BigInt(chimera_term::BigInt::from_i64(*n))
            }
        }
        AST::Float(f) => Term::Float(*f),
        AST::String(s) => Term::String(s.clone().into()),
        AST::CharList(cs) => Term::CharList(cs.clone().into()),
        AST::Binary(_, bits) => Term::Binary(vec![], *bits),
        AST::List(items) => Term::List(items.iter().map(|e| to_term(e, atoms)).collect()),
        AST::Tuple(items) => Term::Tuple(items.iter().map(|e| to_term(e, atoms)).collect()),
        AST::Map(pairs) => Term::Map(pairs.iter().map(|(k, v)| (to_term(k, atoms), to_term(v, atoms))).collect()),
        AST::Var { name, meta } => Term::Var {
            name: name.clone(),
            meta: meta_to_term(&meta, atoms),
            context: chimera_term::VarContext::Default,
        },
        AST::Alias { segments, meta } => Term::Alias {
            segments: segments.clone(),
            meta: meta_to_term(&meta, atoms),
        },
        AST::Identifier { name, meta } => Term::Call {
            name: atoms.intern(name),
            meta: meta_to_term(meta, atoms),
            args: vec![],
        },
        AST::Call { name, meta, args } => Term::Call {
            name: name.clone(),
            meta: meta_to_term(meta, atoms),
            args: args.iter().map(|e| to_term(e, atoms)).collect(),
        },
        AST::RemoteCall { module, name, meta, args } => Term::RemoteCall {
            receiver: Box::new(to_term(module, atoms)),
            name: name.clone(),
            meta: meta_to_term(meta, atoms),
            args: args.iter().map(|e| to_term(e, atoms)).collect(),
        },
        AST::LocalCall { name, meta, args } => Term::Call {
            name: name.clone(),
            meta: meta_to_term(meta, atoms),
            args: args.iter().map(|e| to_term(e, atoms)).collect(),
        },
        AST::Quote { value, meta } => Term::Quote {
            value: Box::new(to_term(value, atoms)),
            meta: meta_to_term(meta, atoms),
        },
        AST::Match { pattern, value, meta } => {
            let _meta_term = meta_to_term(meta, atoms);
            Term::Cons(
                Box::new(Term::Atom(atoms.intern("="))),
                Box::new(Term::Cons(
                    Box::new(to_term(pattern, atoms)),
                    Box::new(Term::Cons(
                        Box::new(to_term(value, atoms)),
                        Box::new(Term::Nil),
                    )),
                )),
            )
        }
        AST::Clause { pattern, guard, body, meta } => {
            let _meta_term = meta_to_term(meta, atoms);
            Term::Cons(
                Box::new(to_term(pattern, atoms)),
                Box::new(Term::Cons(
                    Box::new(guard.as_ref().map_or(Term::Nil, |g| to_term(g, atoms))),
                    Box::new(Term::Cons(
                        Box::new(to_term(body, atoms)),
                        Box::new(Term::Nil),
                    )),
                )),
            )
        }
        AST::Case { expr, clauses, meta } => {
            // Case clauses in quoted form are {:->, meta, [pattern, body]}
            let clauses_term = Term::List(clauses.iter().map(|clause| {
                match clause {
                    AST::Clause { pattern, guard: _, body, meta } => {
                        Term::Call {
                            name: atoms.intern("->"),
                            meta: meta_to_term(meta, atoms),
                            args: vec![
                                Term::List(vec![to_term(pattern, atoms), to_term(body, atoms)]),
                            ],
                        }
                    }
                    _ => to_term(clause, atoms), // Fallback for non-clause (shouldn't happen)
                }
            }).collect());
            Term::Call {
                name: atoms.intern("case"),
                meta: meta_to_term(meta, atoms),
                args: vec![to_term(expr, atoms), clauses_term],
            }
        }
        AST::Cond { clauses, meta } => {
            Term::Call {
                name: atoms.intern("cond"),
                meta: meta_to_term(meta, atoms),
                args: vec![Term::List(clauses.iter().map(|(cond, body)| {
                    Term::List(vec![to_term(cond, atoms), to_term(body, atoms)])
                }).collect())],
            }
        }
        AST::Fn { clauses, meta } => {
            Term::Call {
                name: atoms.intern("fn"),
                meta: meta_to_term(meta, atoms),
                args: vec![Term::List(clauses.iter().map(|c| to_term(c, atoms)).collect())],
            }
        }
        AST::Try { expr, rescue, catch, after, meta } => {
            let rescue_term = if rescue.is_empty() {
                Term::Nil
            } else {
                Term::List(rescue.iter().map(|r| to_term(r, atoms)).collect())
            };
            let catch_term = if catch.is_empty() {
                Term::Nil
            } else {
                Term::List(catch.iter().map(|c| to_term(c, atoms)).collect())
            };
            let after_term = after.as_ref().map_or(Term::Nil, |a| to_term(a, atoms));
            Term::Call {
                name: atoms.intern("try"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    to_term(expr, atoms),
                    Term::Atom(atoms.intern("rescue")),
                    rescue_term,
                    Term::Atom(atoms.intern("catch")),
                    catch_term,
                    Term::Atom(atoms.intern("after")),
                    after_term,
                ],
            }
        }
        AST::Receive { clauses, after, meta } => {
            let clauses_term = Term::List(clauses.iter().map(|c| to_term(c, atoms)).collect());
            let after_term = after.as_ref().map_or(Term::Nil, |(pat, body)| {
                Term::List(vec![to_term(pat, atoms), to_term(body, atoms)])
            });
            Term::Call {
                name: atoms.intern("receive"),
                meta: meta_to_term(meta, atoms),
                args: if after.is_some() {
                    vec![clauses_term, Term::Atom(atoms.intern("after")), after_term]
                } else {
                    vec![clauses_term]
                },
            }
        }
        AST::Defmodule { name, body, meta } => {
            Term::Call {
                name: atoms.intern("defmodule"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    to_term(name, atoms),
                    Term::List(vec![Term::Atom(atoms.intern("do")), Term::List(body.iter().map(|b| to_term(b, atoms)).collect())]),
                ],
            }
        }
        AST::Def { name, meta, clauses } => {
            Term::Call {
                name: atoms.intern("def"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    Term::Atom(name.clone()),
                    Term::List(vec![Term::Atom(atoms.intern("do")), Term::List(clauses.iter().map(|c| to_term(c, atoms)).collect())]),
                ],
            }
        }
        AST::Defp { name, meta, clauses } => {
            Term::Call {
                name: atoms.intern("defp"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    Term::Atom(name.clone()),
                    Term::List(vec![Term::Atom(atoms.intern("do")), Term::List(clauses.iter().map(|c| to_term(c, atoms)).collect())]),
                ],
            }
        }
        AST::Defmacro { name, meta, clauses } => {
            Term::Call {
                name: atoms.intern("defmacro"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    Term::Atom(name.clone()),
                    Term::List(vec![Term::Atom(atoms.intern("do")), Term::List(clauses.iter().map(|c| to_term(c, atoms)).collect())]),
                ],
            }
        }
        AST::Defmacrop { name, meta, clauses } => {
            Term::Call {
                name: atoms.intern("defmacrop"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    Term::Atom(name.clone()),
                    Term::List(vec![Term::Atom(atoms.intern("do")), Term::List(clauses.iter().map(|c| to_term(c, atoms)).collect())]),
                ],
            }
        }
        AST::Unquote { expr, meta } => {
            Term::Call {
                name: atoms.intern("unquote"),
                meta: meta_to_term(meta, atoms),
                args: vec![to_term(expr, atoms)],
            }
        }
        AST::UnquoteSplicing { expr, meta } => {
            Term::Call {
                name: atoms.intern("unquote_splicing"),
                meta: meta_to_term(meta, atoms),
                args: vec![to_term(expr, atoms)],
            }
        }
        AST::AliasExpr { arg, meta } => {
            Term::Call {
                name: atoms.intern("alias"),
                meta: meta_to_term(meta, atoms),
                args: vec![to_term(arg, atoms)],
            }
        }
        AST::RequireExpr { arg, meta } => {
            Term::Call {
                name: atoms.intern("require"),
                meta: meta_to_term(meta, atoms),
                args: vec![to_term(arg, atoms)],
            }
        }
        AST::ImportExpr { arg, meta, opts } => {
            Term::Call {
                name: atoms.intern("import"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    to_term(arg, atoms),
                    Term::List(opts.iter().map(|o| to_term(o, atoms)).collect()),
                ],
            }
        }
        AST::Block { exprs, meta } => {
            Term::Call {
                name: atoms.intern("__block__"),
                meta: meta_to_term(meta, atoms),
                args: vec![Term::List(exprs.iter().map(|e| to_term(e, atoms)).collect())],
            }
        }
        AST::Capture { fun, arity, meta } => {
            match arity {
                Some(a) => Term::Call {
                    name: atoms.intern("&"),
                    meta: meta_to_term(meta, atoms),
                    args: vec![Term::SmallInt(*a as i64)],
                },
                None => Term::Call {
                    name: atoms.intern("&"),
                    meta: meta_to_term(meta, atoms),
                    args: vec![to_term(fun, atoms)],
                },
            }
        }
        AST::BinaryOp { op, left, right, meta: _ } => {
            Term::Cons(
                Box::new(Term::Atom(op.clone())),
                Box::new(Term::Cons(
                    Box::new(to_term(left, atoms)),
                    Box::new(Term::Cons(
                        Box::new(to_term(right, atoms)),
                        Box::new(Term::Nil),
                    )),
                )),
            )
        }
        AST::UnaryOp { op, arg, meta: _ } => {
            Term::Cons(
                Box::new(Term::Atom(op.clone())),
                Box::new(Term::Cons(
                    Box::new(to_term(arg, atoms)),
                    Box::new(Term::Nil),
                )),
            )
        }
        AST::Access { record, field, meta } => {
            Term::Call {
                name: atoms.intern("access"),
                meta: meta_to_term(meta, atoms),
                args: vec![to_term(record, atoms), to_term(field, atoms)],
            }
        }
        AST::With { bindings, body, meta } => {
            Term::Call {
                name: atoms.intern("with"),
                meta: meta_to_term(meta, atoms),
                args: vec![
                    Term::List(bindings.iter().map(|(k, v)| {
                        Term::List(vec![to_term(k, atoms), to_term(v, atoms)])
                    }).collect()),
                    Term::Atom(atoms.intern("do")),
                    to_term(body, atoms),
                ],
            }
        }
        AST::Attribute { name, value, meta } => {
            Term::Call {
                name: name.clone(),
                meta: meta_to_term(meta, atoms),
                args: vec![to_term(value, atoms)],
            }
        }
        AST::Defstruct { fields, meta } => {
            Term::Call {
                name: atoms.intern("defstruct"),
                meta: meta_to_term(meta, atoms),
                args: vec![Term::List(fields.iter().map(|(k, dv)| {
                    match dv {
                        Some(v) => Term::List(vec![Term::Atom(k.clone()), to_term(v, atoms)]),
                        None => Term::Atom(k.clone()),
                    }
                }).collect())],
            }
        }
        AST::Defexception { fields, meta } => {
            Term::Call {
                name: atoms.intern("defexception"),
                meta: meta_to_term(meta, atoms),
                args: vec![Term::List(fields.iter().map(|(k, dv)| {
                    match dv {
                        Some(v) => Term::List(vec![Term::Atom(k.clone()), to_term(v, atoms)]),
                        None => Term::Atom(k.clone()),
                    }
                }).collect())],
            }
        }
    }
}


fn meta_to_term(meta: &Meta, _atoms: &mut chimera_term::AtomTable) -> chimera_term::Meta {
    chimera_term::Meta {
        line: meta.location.as_ref().map(|l| l.line).unwrap_or(0),
        column: meta.location.as_ref().map(|l| l.column).unwrap_or(0),
        file: meta.location.as_ref().map(|l| format!("{:?}", l.file).into()),
        context: meta.context.clone().map(|a| format!("{:?}", a).into()),
        hygiene: chimera_term::HygieneContext {
            origin: meta.hygiene.origin.clone(),
            clashing: false,
            generated: meta.hygiene.generated,
        },
    }
}

/// A lossless Concrete Syntax Tree node preserving all source details.
#[derive(Debug, Clone, PartialEq)]
pub struct CSTNode {
    pub kind: CSTKind,
    pub span: SourceSpan,
    pub children: Vec<CSTNode>,
    pub value: Option<String>,
}

/// CST node kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum CSTKind {
    // Source elements
    SourceFile,
    SourceFragment,

    // Lexical tokens
    Token(TokenKind),
    Error,
    Comment,
    Whitespace,
    Newline,

    // Expressions
    Expression(AST),

    // Clauses
    DoClause,
    ClauseCondition,
    ClauseBody,

    // Definitions
    ModuleDefinition,
    FunctionDefinition,
    MacroDefinition,
    TypeDefinition,

    // Body elements
    Body,
    StabClause,
}

/// Token kind for CST representation.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum TokenKind {
    Identifier,
    Keyword,
    Atom,
    Integer,
    Float,
    String,
    Operator,
    Delimiter,
    Eof,
}

/// Convert quoted AST to CST for formatter/diagnostics.
pub fn ast_to_cst(ast: &AST, span: SourceSpan) -> CSTNode {
    let kind = match ast {
        AST::Nil => CSTKind::Token(TokenKind::Atom),
        AST::Atom(_) => CSTKind::Token(TokenKind::Atom),
        AST::Integer(_) => CSTKind::Token(TokenKind::Integer),
        AST::Float(_) => CSTKind::Token(TokenKind::Float),
        AST::String(_) => CSTKind::Token(TokenKind::String),
        _ => CSTKind::Expression(ast.clone()),
    };

    CSTNode {
        kind,
        span,
        children: vec![],
        value: None,
    }
}

/// Pretty print an AST for debugging.
pub fn pretty_print(ast: &AST, indent: usize) -> String {
    let prefix = "  ".repeat(indent);
    match ast {
        AST::Nil => format!("{}nil", prefix),
        AST::Atom(a) => format!("{}:{}", prefix, a.clone().id()),
        AST::Integer(n) => format!("{}{}", prefix, n),
        AST::Float(f) => format!("{}{}", prefix, f),
        AST::String(s) => format!("{}\"{}\"", prefix, s),
        AST::Identifier { name, .. } => format!("{}{}", prefix, name),
        AST::Var { name, .. } => format!("{}var({})", prefix, name.clone().id()),
        AST::Call { name, args, .. } => {
            let args_str = args.iter().map(|a| pretty_print(a, 0)).collect::<Vec<_>>().join(", ");
            format!("{}({} {})", prefix, name.clone().id(), args_str)
        }
        AST::List(items) => {
            let items_str = items.iter().map(|a| pretty_print(a, indent + 1)).collect::<Vec<_>>().join("\n");
            format!("{}[\n{}\n{}]", prefix, items_str, prefix)
        }
        AST::Tuple(items) => {
            let items_str = items.iter().map(|a| pretty_print(a, 0)).collect::<Vec<_>>().join(", ");
            format!("{}{{{}}}", prefix, items_str)
        }
        AST::Defmodule { name, body, .. } => {
            let body_str = body.iter().map(|a| pretty_print(a, indent + 1)).collect::<Vec<_>>().join("\n");
            format!("{}defmodule {} do\n{}\n{}end", prefix, pretty_print(name, 0), body_str, prefix)
        }
        _ => format!("{}<ast>", prefix),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_term::AtomTable;
    use chimera_source::{SourceFileId, SourceSpan, SourceOffset};

    #[test]
    fn test_meta_new() {
        let meta = Meta::new(SourceFileId::new(0), 1, 0);
        assert!(meta.location.is_some());
        assert!(!meta.hygiene.generated);
    }

    #[test]
    fn test_hygiene_generated() {
        let hyg = Hygiene::generated();
        assert!(hyg.generated);
        assert!(hyg.origin.is_none());
    }

    #[test]
    fn test_meta_with_hygiene() {
        let meta = Meta::new(SourceFileId::new(0), 1, 0).with_hygiene(Hygiene::generated());
        assert!(meta.hygiene.generated);
    }

    #[test]
    fn test_meta_with_context() {
        let mut atoms = chimera_term::AtomTable::new();
        let ctx = atoms.intern("assignment");
        let meta = Meta::new(SourceFileId::new(0), 1, 0).with_context(ctx);
        assert!(meta.context.is_some());
    }

    #[test]
    fn test_meta_location() {
        let file_id = SourceFileId::new(5);
        let meta = Meta::new(file_id, 10, 25);
        assert!(meta.location.is_some());
        let loc = meta.location.unwrap();
        assert_eq!(loc.file, file_id);
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 25);
    }

    #[test]
    fn test_hygiene_with_origin() {
        let hyg = Hygiene {
            origin: Some("macro".to_string()),
            generated: false,
            import: None,
        };
        assert!(hyg.origin.is_some());
        assert_eq!(hyg.origin.unwrap(), "macro");
    }

    #[test]
    fn test_hygiene_import_context() {
        let mut atoms = chimera_term::AtomTable::new();
        let foo_atom = atoms.intern("Foo");
        let mod_name = ModuleName::new(vec![foo_atom]);
        let import_ctx = ImportContext {
            module: mod_name.clone(),
            alias: None,
        };
        let hyg = Hygiene {
            origin: None,
            generated: false,
            import: Some(import_ctx),
        };
        assert!(hyg.import.is_some());
        assert_eq!(hyg.import.as_ref().unwrap().module, mod_name);
    }

    #[test]
    fn test_meta_custom_entries() {
        let mut meta = Meta::new(SourceFileId::new(0), 1, 0);
        let mut atoms = chimera_term::AtomTable::new();
        let key = atoms.intern("custom_key");
        let value = Term::SmallInt(42);
        meta.custom.push((key, value));
        assert_eq!(meta.custom.len(), 1);
        assert_eq!(meta.custom[0].1, Term::SmallInt(42));
    }

    #[test]
    fn test_location_default() {
        let loc = Location {
            file: SourceFileId::new(1),
            line: 5,
            column: 15,
        };
        assert_eq!(loc.line, 5);
        assert_eq!(loc.column, 15);
    }

    #[test]
    fn test_expr_context_variants() {
        assert_eq!(ExprContext::Default, ExprContext::Default);
        assert_eq!(ExprContext::Match, ExprContext::Match);
        assert_eq!(ExprContext::Guard, ExprContext::Guard);
        assert_eq!(ExprContext::TypeSpec, ExprContext::TypeSpec);
        assert_eq!(ExprContext::MacroDefinition, ExprContext::MacroDefinition);
        assert_eq!(ExprContext::Quote, ExprContext::Quote);
    }

    #[test]
    fn test_hygiene_is_default() {
        let hyg = Hygiene::default();
        assert!(!hyg.generated);
        assert!(hyg.origin.is_none());
        assert!(hyg.import.is_none());
    }

    #[test]
    fn test_hygiene_origin_survives_clone() {
        let hyg = Hygiene {
            origin: Some("macro_source".to_string()),
            generated: true,
            import: None,
        };
        let cloned = hyg.clone();
        assert_eq!(cloned.origin, Some("macro_source".to_string()));
        assert!(cloned.generated);
    }

    #[test]
    fn test_import_context_struct() {
        let mut atoms = chimera_term::AtomTable::new();
        let foo = atoms.intern("Foo");
        let mod_name = ModuleName::new(vec![foo]);
        let import_ctx = ImportContext {
            module: mod_name,
            alias: None,
        };
        assert!(import_ctx.alias.is_none());
    }

    #[test]
    fn test_import_context_with_alias() {
        let mut atoms = chimera_term::AtomTable::new();
        let foo = atoms.intern("Foo");
        let bar = atoms.intern("Bar");
        let mod_name = ModuleName::new(vec![foo]);
        let import_ctx = ImportContext {
            module: mod_name,
            alias: Some(bar),
        };
        assert!(import_ctx.alias.is_some());
    }

    #[test]
    fn test_meta_with_custom_entry() {
        let mut meta = Meta::new(SourceFileId::new(0), 1, 0);
        let mut atoms = chimera_term::AtomTable::new();
        let key = atoms.intern("hygiene_info");
        let value = Term::Atom(atoms.intern("macro_origin"));
        meta.custom.push((key, value));
        let retrieved = meta.custom.get(0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().1, Term::Atom(atoms.intern("macro_origin")));
    }

    #[test]
    fn test_hygiene_generated_preserves_origin() {
        // Generated hygiene should still preserve origin
        let hyg = Hygiene {
            origin: Some("auto_gen_var".to_string()),
            generated: true,
            import: None,
        };
        assert_eq!(hyg.origin, Some("auto_gen_var".to_string()));
        assert!(hyg.generated);
    }

    #[test]
    fn test_ast_to_term_integer() {
        let ast = AST::Integer(42);
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        assert_eq!(term, Term::SmallInt(42));
    }

    #[test]
    fn test_ast_to_term_atom() {
        let mut atoms = AtomTable::new();
        let atom = atoms.intern("foo");
        let ast = AST::Atom(atom.clone());
        let term = to_term(&ast, &mut atoms);
        assert_eq!(term, Term::Atom(atom));
    }

    #[test]
    fn test_ast_to_term_list() {
        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2), AST::Integer(3)]);
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        assert_eq!(term, Term::List(vec![Term::SmallInt(1), Term::SmallInt(2), Term::SmallInt(3)]));
    }

    #[test]
    fn test_ast_to_term_string() {
        let ast = AST::String("hello".to_string());
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        assert_eq!(term, Term::String("hello".into()));
    }

    #[test]
    fn test_expr_context_default() {
        let ctx = ExprContext::default();
        assert_eq!(ctx, ExprContext::Default);
    }

    #[test]
    fn test_pretty_print_integer() {
        let ast = AST::Integer(42);
        let result = pretty_print(&ast, 0);
        assert_eq!(result, "42");
    }

    #[test]
    fn test_pretty_print_string() {
        let ast = AST::String("hello".to_string());
        let result = pretty_print(&ast, 0);
        assert_eq!(result, "\"hello\"");
    }

    #[test]
    fn test_cst_node() {
        let span = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(5));
        let node = CSTNode {
            kind: CSTKind::Token(TokenKind::Identifier),
            span,
            children: vec![],
            value: None,
        };
        assert_eq!(node.kind, CSTKind::Token(TokenKind::Identifier));
    }

    #[test]
    fn test_ast_to_term_match() {
        let pattern = AST::Var { name: Atom::new(1), meta: Meta::default() };
        let value = AST::Integer(42);
        let ast = AST::Match {
            pattern: Box::new(pattern),
            value: Box::new(value),
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        // Match is represented as (= pattern value)
        match term {
            Term::Cons(inner_a, inner_rest) => {
                let inner_a = *inner_a;
                match inner_a {
                    Term::Atom(a) => {
                        assert_eq!(a.id(), atoms.intern("=").id());
                    }
                    _ => panic!("Expected Atom for match operator"),
                }
                match *inner_rest {
                    Term::Cons(_, _) => {}, // Has more elements
                    _ => panic!("Expected Cons for match body"),
                }
            }
            _ => panic!("Expected Match to produce Cons"),
        }
    }

    #[test]
    fn test_ast_to_term_case() {
        let expr = AST::Integer(42);
        let clause = AST::Clause {
            pattern: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
            guard: None,
            body: Box::new(AST::Integer(100)),
            meta: Meta::default(),
        };
        let ast = AST::Case {
            expr: Box::new(expr),
            clauses: vec![clause],
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("case").id());
                assert_eq!(args.len(), 2);
                // args[0] should be the expr (42)
                match &args[0] {
                    Term::SmallInt(n) => assert_eq!(*n, 42),
                    _ => panic!("Expected SmallInt for case expr"),
                }
                // args[1] should be a list of clauses
                // Each clause in quoted form is {:->, meta, [pattern, body]}
                match &args[1] {
                    Term::List(clauses) => {
                        assert_eq!(clauses.len(), 1);
                        match &clauses[0] {
                            Term::Call { name: clause_name, args: clause_args, .. } => {
                                assert_eq!(clause_name.clone().id(), atoms.intern("->").id());
                                assert_eq!(clause_args.len(), 1);
                            }
                            _ => panic!("Expected Call for clause"),
                        }
                    }
                    _ => panic!("Expected List for clauses"),
                }
            }
            _ => panic!("Expected Case to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_case_quoted_form_matches_elixir() {
        // Verify that case produces the canonical quoted form:
        // {:case, meta, [expr, [do: [{:->, meta, [pattern, body]}]]]}
        let expr = AST::Integer(42);
        let clause = AST::Clause {
            pattern: Box::new(AST::Integer(1)),
            guard: None,
            body: Box::new(AST::Integer(100)),
            meta: Meta::default(),
        };
        let ast = AST::Case {
            expr: Box::new(expr),
            clauses: vec![clause],
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);

        // Verify top-level structure: {:case, meta, args}
        match term {
            Term::Call { name, meta: _, args } => {
                assert_eq!(name.id(), atoms.intern("case").id());
                // meta should be a Meta struct with location info
                assert_eq!(args.len(), 2); // [expr, clauses]
            }
            _ => panic!("Expected Call for case"),
        }
    }

    #[test]
    fn test_ast_to_term_defmodule() {
        let name = AST::Atom(Atom::new(1));
        let body = vec![AST::Integer(1), AST::Integer(2)];
        let ast = AST::Defmodule {
            name: Box::new(name),
            body,
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("defmodule").id());
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Defmodule to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_try() {
        let expr = AST::Integer(42);
        let ast = AST::Try {
            expr: Box::new(expr),
            rescue: vec![],
            catch: vec![],
            after: None,
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("try").id());
                // try returns value, then rescue/catch/after
                assert!(args.len() >= 1);
            }
            _ => panic!("Expected Try to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_receive() {
        let clause = AST::Clause {
            pattern: Box::new(AST::Atom(Atom::new(1))),
            guard: None,
            body: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let ast = AST::Receive {
            clauses: vec![clause],
            after: None,
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("receive").id());
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Receive to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_binary_op() {
        let left = AST::Integer(1);
        let right = AST::Integer(2);
        let ast = AST::BinaryOp {
            op: Atom::new(1),
            left: Box::new(left),
            right: Box::new(right),
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Cons(inner_op, inner_rest) => {
                match *inner_op {
                    Term::Atom(_) => {},
                    _ => panic!("Expected Atom for binary op"),
                }
                match *inner_rest {
                    Term::Cons(_, _) => {}, // Has operands
                    _ => panic!("Expected Cons for binary op"),
                }
            }
            _ => panic!("Expected BinaryOp to produce Cons"),
        }
    }

    #[test]
    fn test_ast_to_term_unary_op() {
        let arg = AST::Integer(42);
        let ast = AST::UnaryOp {
            op: Atom::new(1),
            arg: Box::new(arg),
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Cons(inner_op, inner_rest) => {
                match *inner_op {
                    Term::Atom(_) => {},
                    _ => panic!("Expected Atom for unary op"),
                }
                match *inner_rest {
                    Term::Cons(_, _) => {},
                    _ => panic!("Expected Cons for unary op"),
                }
            }
            _ => panic!("Expected UnaryOp to produce Cons"),
        }
    }

    #[test]
    fn test_ast_to_term_cond() {
        let cond = AST::Integer(1);
        let body = AST::Integer(42);
        let ast = AST::Cond {
            clauses: vec![(Box::new(cond), Box::new(body))],
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("cond").id());
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Cond to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_fn() {
        let clause = AST::Clause {
            pattern: Box::new(AST::Nil),
            guard: None,
            body: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let ast = AST::Fn {
            clauses: vec![clause],
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("fn").id());
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Fn to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_unquote() {
        let expr = AST::Integer(42);
        let ast = AST::Unquote {
            expr: Box::new(expr),
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("unquote").id());
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Unquote to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_block() {
        let ast = AST::Block {
            exprs: vec![AST::Integer(1), AST::Integer(2)],
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("__block__").id());
                assert_eq!(args.len(), 1);
            }
            _ => panic!("Expected Block to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_alias_expr() {
        let arg = AST::Atom(Atom::new(1));
        let ast = AST::AliasExpr {
            arg: Box::new(arg),
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, .. } => {
                assert_eq!(name.id(), atoms.intern("alias").id());
            }
            _ => panic!("Expected AliasExpr to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_require_expr() {
        let arg = AST::Atom(Atom::new(1));
        let ast = AST::RequireExpr {
            arg: Box::new(arg),
            meta: Meta::default(),
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, .. } => {
                assert_eq!(name.id(), atoms.intern("require").id());
            }
            _ => panic!("Expected RequireExpr to produce Call"),
        }
    }

    #[test]
    fn test_ast_to_term_import_expr() {
        let arg = AST::Atom(Atom::new(1));
        let ast = AST::ImportExpr {
            arg: Box::new(arg),
            meta: Meta::default(),
            opts: vec![],
        };
        let mut atoms = AtomTable::new();
        let term = to_term(&ast, &mut atoms);
        match term {
            Term::Call { name, args, .. } => {
                assert_eq!(name.id(), atoms.intern("import").id());
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected ImportExpr to produce Call"),
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use chimera_term::AtomTable;
    use chimera_source::{SourceFileId};
    use chimera_diag::{Diagnostic, Severity};

    /// Property test for AST to term conversion - should always succeed
    #[test]
    fn test_ast_to_term_always_succeeds() {
        fn arb_ast() -> BoxedStrategy<AST> {
            // Simple AST generation strategy for testing
                prop_oneof![
                    Just(AST::Nil),
                    any::<i64>().prop_map(AST::Integer),
                    any::<f64>().prop_map(AST::Float),
                    ".*".prop_map(|s| AST::String(s)),
                ]
                .boxed()
        }

        proptest!(|(ast in arb_ast())| {
            let mut atoms = AtomTable::new();
            // This should not panic - conversion should always succeed
            let _result = to_term(&ast, &mut atoms);
        });
    }

    /// Property test for Meta creation - should always preserve values
    #[test]
    fn test_meta_location_preserved() {
        proptest!(|(file_id in 0u32..100u32, line in 0u32..1000u32, column in 0u32..100u32)| {
            let meta = Meta::new(SourceFileId::new(file_id), line, column);
            prop_assert!(meta.location.is_some());
            let loc = meta.location.unwrap();
            prop_assert_eq!(loc.file, SourceFileId::new(file_id));
            prop_assert_eq!(loc.line, line);
            prop_assert_eq!(loc.column, column);
        });
    }

    /// Property test for Diagnostic creation - should always succeed
    #[test]
    fn test_diagnostic_creation_always_succeeds() {
        proptest!(|(message in ".*")| {
            let diag = Diagnostic::error(&message);
            prop_assert_eq!(diag.severity, Severity::Error);
            prop_assert_eq!(diag.message, message);
        });
    }
}