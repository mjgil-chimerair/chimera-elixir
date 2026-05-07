//! Core IR (Intermediate Representation) for the Rust/Zig Elixir compiler.
//!
//! This module provides the Core IR representation that sits between
//! the expanded AST and the final target artifact. Core IR is a simplified,
//! validated representation suitable for code generation.

#[cfg(test)]
use chimera_allocator as _;

use chimera_ast::{AST, Meta};
use chimera_term::{Atom, AtomTable, ModuleName};
use std::collections::{HashMap, HashSet};

/// Core IR expression types.
#[derive(Debug, Clone)]
pub enum CoreExpr {
    /// Unit (no value)
    Unit,
    /// Literal atom
    Atom(Atom),
    /// Literal integer
    Integer(i64),
    /// Literal float
    Float(f64),
    /// Literal string
    String(String),
    /// Literal list
    List(Vec<CoreExpr>),
    /// Literal tuple
    Tuple(Vec<CoreExpr>),
    /// Literal map
    Map(Vec<(CoreExpr, CoreExpr)>),
    /// Variable reference
    Var { name: Atom, arity: u8 },
    /// Function call
    Call { module: Option<Atom>, name: Atom, args: Vec<CoreExpr> },
    /// Lambda/anonymous function
    Lambda { args: Vec<Atom>, body: Box<CoreExpr> },
    /// Local binding
    Let { vars: Vec<Atom>, value: Box<CoreExpr>, body: Box<CoreExpr> },
    /// Sequence of expressions
    Seq(Vec<CoreExpr>),
    /// Match expression
    Match { pattern: CorePattern, value: Box<CoreExpr>, body: Box<CoreExpr> },
    /// Case expression
    Case { expr: Box<CoreExpr>, clauses: Vec<CoreClause> },
    /// Try/catch expression
    Try { expr: Box<CoreExpr>, clauses: Vec<CoreClause> },
    /// Receive expression
    Receive { clauses: Vec<CoreClause>, timeout: Option<(Box<CoreExpr>, Box<CoreExpr>)> },
    /// Tuple construction
    TupleCons { elements: Vec<CoreExpr> },
    /// Map update
    MapUpdate { base: Box<CoreExpr>, updates: Vec<(CoreExpr, CoreExpr)> },
    /// Binary construction
    Binary { segments: Vec<CoreBinarySegment> },
}

/// Binary segment in Core IR.
#[derive(Debug, Clone)]
pub struct CoreBinarySegment {
    pub expr: CoreExpr,
    pub size: Option<Box<CoreExpr>>,
    pub unit: Option<u8>,
    pub type_spec: Option<Atom>,
}

/// Core IR pattern for matching.
#[derive(Debug, Clone)]
pub enum CorePattern {
    /// Wildcard pattern (_)
    Wildcard,
    /// Variable binding pattern
    Var(Atom),
    /// Atomic pattern
    Atom(Atom),
    /// Integer pattern
    Integer(i64),
    /// String pattern
    String(String),
    /// List pattern [head | tail]
    List { head: Option<Box<CorePattern>>, tail: Option<Box<CorePattern>> },
    /// Tuple pattern
    Tuple(Vec<CorePattern>),
    /// Map pattern
    Map(Vec<(CorePattern, CorePattern)>),
    /// Cons cell pattern [head | tail]
    Cons { head: Box<CorePattern>, tail: Box<CorePattern> },
    /// Binary pattern
    Binary(Vec<CorePattern>),
}

/// A single clause in case/try/receive.
#[derive(Debug, Clone)]
pub struct CoreClause {
    pub pattern: CorePattern,
    pub guards: Vec<CoreGuard>,
    pub body: CoreExpr,
    pub meta: Meta,
}

/// Guard expression in Core IR.
#[derive(Debug, Clone)]
pub enum CoreGuard {
    /// Atomic comparison
    AtomicCmp(Atom, Box<CoreExpr>, Box<CoreExpr>),
    /// Type test
    TypeTest(Atom, Box<CoreExpr>),
    /// Logical AND
    And(Vec<CoreGuard>),
    /// Logical OR
    Or(Vec<CoreGuard>),
    /// Logical NOT
    Not(Box<CoreGuard>),
    /// Comparison operators
    Lt(Box<CoreExpr>, Box<CoreExpr>),
    Le(Box<CoreExpr>, Box<CoreExpr>),
    Gt(Box<CoreExpr>, Box<CoreExpr>),
    Ge(Box<CoreExpr>, Box<CoreExpr>),
    Eq(Box<CoreExpr>, Box<CoreExpr>),
    Neq(Box<CoreExpr>, Box<CoreExpr>),
}

/// Core IR module definition.
#[derive(Debug, Clone)]
pub struct CoreModule {
    /// Module name
    pub name: ModuleName,
    /// Exported functions
    pub exports: HashSet<(Atom, u8)>,
    /// Module attributes
    pub attributes: HashMap<Atom, CoreExpr>,
    /// Function definitions
    pub functions: Vec<CoreFunction>,
    /// Compile info
    pub compile_info: CoreCompileInfo,
}

/// Function definition in Core IR.
#[derive(Debug, Clone)]
pub struct CoreFunction {
    /// Function name
    pub name: Atom,
    /// Function arity
    pub arity: u8,
    /// Formal parameters
    pub params: Vec<Atom>,
    /// Guard expressions
    pub guards: Vec<CoreGuard>,
    /// Function body
    pub body: CoreExpr,
    /// Whether it's exported
    pub exported: bool,
    /// Meta information
    pub meta: Meta,
}

/// Compile information for Core IR.
#[derive(Debug, Clone)]
pub struct CoreCompileInfo {
    /// Source file
    pub file: Option<String>,
    /// Module line
    pub line: u32,
    /// Module version
    pub vsn: Option<Atom>,
}

/// Core IR validation error.
#[derive(Debug, Clone)]
pub enum CoreError {
    /// Invalid pattern
    InvalidPattern(String),
    /// Undefined variable
    UndefinedVariable(Atom),
    /// Duplicate definition
    DuplicateDefinition(String),
    /// Invalid guard
    InvalidGuard(String),
    /// Invalid call
    InvalidCall(String),
    /// Type error
    TypeError(String),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::InvalidPattern(s) => write!(f, "Invalid pattern: {}", s),
            CoreError::UndefinedVariable(v) => write!(f, "Undefined variable: {:?}", v),
            CoreError::DuplicateDefinition(s) => write!(f, "Duplicate definition: {}", s),
            CoreError::InvalidGuard(s) => write!(f, "Invalid guard: {}", s),
            CoreError::InvalidCall(s) => write!(f, "Invalid call: {}", s),
            CoreError::TypeError(s) => write!(f, "Type error: {}", s),
        }
    }
}

impl std::error::Error for CoreError {}

/// Core IR builder from AST.
pub struct CoreBuilder {
    /// Atom table
    atoms: AtomTable,
    /// Current module name
    module: Option<ModuleName>,
    /// Local variables in scope
    vars: HashSet<Atom>,
    /// Errors accumulated
    errors: Vec<CoreError>,
}

impl CoreBuilder {
    /// Create a new Core builder.
    pub fn new(atoms: AtomTable) -> Self {
        CoreBuilder {
            atoms,
            module: None,
            vars: HashSet::new(),
            errors: Vec::new(),
        }
    }

    /// Set the current module.
    pub fn set_module(&mut self, name: ModuleName) {
        self.module = Some(name);
    }

    /// Add a variable to scope.
    pub fn add_var(&mut self, name: Atom) {
        self.vars.insert(name);
    }

    /// Clear all variables from scope.
    pub fn clear_vars(&mut self) {
        self.vars.clear();
    }

    /// Check if a variable is in scope.
    pub fn is_var(&self, name: &Atom) -> bool {
        self.vars.contains(name)
    }

    /// Convert an AST to Core IR.
    pub fn from_ast(&mut self, ast: AST) -> Result<CoreExpr, CoreError> {
        match ast {
            AST::Nil => Ok(CoreExpr::Atom(self.atoms.intern("nil"))),
            AST::Atom(a) => Ok(CoreExpr::Atom(a)),
            AST::Integer(i) => Ok(CoreExpr::Integer(i)),
            AST::Float(f) => Ok(CoreExpr::Float(f)),
            AST::String(s) => Ok(CoreExpr::String(s)),
            AST::List(items) => {
                let core_items: Result<Vec<_>, _> = items.into_iter()
                    .map(|item| self.from_ast(item))
                    .collect();
                Ok(CoreExpr::List(core_items?))
            }
            AST::Tuple(elements) => {
                let core_elements: Result<Vec<_>, _> = elements.into_iter()
                    .map(|elem| self.from_ast(elem))
                    .collect();
                Ok(CoreExpr::Tuple(core_elements?))
            }
            AST::Var { name, .. } => {
                if self.is_var(&name) {
                    Ok(CoreExpr::Var { name, arity: 0 })
                } else {
                    Err(CoreError::UndefinedVariable(name))
                }
            }
            AST::Call { name, args, .. } => {
                let core_args: Result<Vec<_>, _> = args.into_iter()
                    .map(|arg| self.from_ast(arg))
                    .collect();
                Ok(CoreExpr::Call { module: None, name, args: core_args? })
            }
            AST::Defmodule { name, body, .. } => {
                // Extract module name
                if let AST::Atom(atom) = *name {
                    self.module = Some(ModuleName::new(vec![atom]));
                }
                let core_body: Result<Vec<_>, _> = body.into_iter()
                    .map(|form| self.from_ast(form))
                    .collect();
                Ok(CoreExpr::Seq(core_body?))
            }
            AST::Def { name: _, clauses, .. } => {
                // Convert function definition - just convert all clauses as a sequence
                let core_clauses: Result<Vec<_>, _> = clauses.into_iter()
                    .map(|c| self.from_ast(c))
                    .collect();
                Ok(CoreExpr::Seq(core_clauses?))
            }
            AST::Quote { value, .. } => {
                self.from_ast(*value)
            }
            _ => Ok(CoreExpr::Unit),
        }
    }

    /// Convert guards from AST to Core IR.
    fn convert_guards(&mut self, guards: Vec<AST>) -> Result<Vec<CoreGuard>, CoreError> {
        guards.into_iter().map(|g| self.guard_from_ast(g)).collect()
    }

    /// Convert a single guard.
    fn guard_from_ast(&mut self, ast: AST) -> Result<CoreGuard, CoreError> {
        match ast {
            AST::Call { name, args, .. } => {
                if args.len() == 1 {
                    let arg = self.from_ast(args[0].clone())?;
                    Ok(CoreGuard::TypeTest(name, Box::new(arg)))
                } else if args.len() == 2 {
                    let left = self.from_ast(args[0].clone())?;
                    let right = self.from_ast(args[1].clone())?;
                    Ok(CoreGuard::AtomicCmp(name, Box::new(left), Box::new(right)))
                } else {
                    Err(CoreError::InvalidGuard("Invalid guard".to_string()))
                }
            }
            _ => Err(CoreError::InvalidGuard("Invalid guard".to_string())),
        }
    }

    /// Get any accumulated errors.
    pub fn errors(&self) -> &[CoreError] {
        &self.errors
    }

    /// Check if there are errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Validate Core IR module.
pub fn validate_module(module: &CoreModule) -> Result<(), CoreError> {
    // Check for duplicate function names
    let mut seen: HashSet<(Atom, u8)> = HashSet::new();
    for func in &module.functions {
        let key = (func.name.clone(), func.arity);
        if seen.contains(&key) {
            return Err(CoreError::DuplicateDefinition(format!("{:?}/{}", func.name, func.arity)));
        }
        seen.insert(key);
    }
    Ok(())
}

/// Compile a module from AST to Core IR.
pub fn compile_module(ast: AST, atoms: AtomTable) -> Result<CoreModule, CoreError> {
    let mut builder = CoreBuilder::new(atoms);
    let _expr = builder.from_ast(ast)?;

    let module_name = builder.module.unwrap_or(ModuleName::new(vec![Atom::new(0)]));

    Ok(CoreModule {
        name: module_name,
        exports: HashSet::new(),
        attributes: HashMap::new(),
        functions: Vec::new(),
        compile_info: CoreCompileInfo {
            file: None,
            line: 1,
            vsn: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_builder_new() {
        let atoms = AtomTable::new();
        let builder = CoreBuilder::new(atoms);
        assert!(builder.module.is_none());
        assert!(builder.vars.is_empty());
    }

    #[test]
    fn test_core_builder_add_var() {
        let atoms = AtomTable::new();
        let mut builder = CoreBuilder::new(atoms);
        let var = Atom::new(1);
        builder.add_var(var.clone());
        assert!(builder.is_var(&var));
    }

    #[test]
    fn test_core_builder_clear_vars() {
        let atoms = AtomTable::new();
        let mut builder = CoreBuilder::new(atoms);
        let var = Atom::new(1);
        builder.add_var(var.clone());
        builder.clear_vars();
        assert!(!builder.is_var(&var));
    }

    #[test]
    fn test_core_expr_atom() {
        let atoms = AtomTable::new();
        let mut builder = CoreBuilder::new(atoms);
        let ast = AST::Atom(Atom::new(1));
        let result = builder.from_ast(ast);
        assert!(result.is_ok());
        match result.unwrap() {
            CoreExpr::Atom(a) => assert_eq!(a.id(), 1),
            _ => panic!("Expected Atom"),
        }
    }

    #[test]
    fn test_core_expr_integer() {
        let atoms = AtomTable::new();
        let mut builder = CoreBuilder::new(atoms);
        let ast = AST::Integer(42);
        let result = builder.from_ast(ast);
        assert!(result.is_ok());
        match result.unwrap() {
            CoreExpr::Integer(i) => assert_eq!(i, 42),
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_core_expr_list() {
        let atoms = AtomTable::new();
        let mut builder = CoreBuilder::new(atoms);
        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2)]);
        let result = builder.from_ast(ast);
        assert!(result.is_ok());
        match result.unwrap() {
            CoreExpr::List(items) => assert_eq!(items.len(), 2),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_core_expr_tuple() {
        let atoms = AtomTable::new();
        let mut builder = CoreBuilder::new(atoms);
        let ast = AST::Tuple(vec![AST::Integer(1), AST::Integer(2)]);
        let result = builder.from_ast(ast);
        assert!(result.is_ok());
        match result.unwrap() {
            CoreExpr::Tuple(elems) => assert_eq!(elems.len(), 2),
            _ => panic!("Expected Tuple"),
        }
    }

    #[test]
    fn test_core_pattern_wildcard() {
        let pattern = CorePattern::Wildcard;
        match pattern {
            CorePattern::Wildcard => {}
            _ => panic!("Expected Wildcard"),
        }
    }

    #[test]
    fn test_core_pattern_var() {
        let pattern = CorePattern::Var(Atom::new(1));
        match pattern {
            CorePattern::Var(v) => assert_eq!(v.id(), 1),
            _ => panic!("Expected Var"),
        }
    }

    #[test]
    fn test_core_clause() {
        let clause = CoreClause {
            pattern: CorePattern::Wildcard,
            guards: vec![],
            body: CoreExpr::Unit,
            meta: Meta::default(),
        };
        assert!(matches!(clause.pattern, CorePattern::Wildcard));
    }

    #[test]
    fn test_core_binary_segment() {
        let seg = CoreBinarySegment {
            expr: CoreExpr::Integer(1),
            size: None,
            unit: None,
            type_spec: None,
        };
        match seg.expr {
            CoreExpr::Integer(1) => {}
            _ => panic!("Expected Integer"),
        }
    }

    #[test]
    fn test_core_error_display() {
        let err = CoreError::UndefinedVariable(Atom::new(1));
        assert!(!format!("{}", err).is_empty());
    }

    #[test]
    fn test_validate_module_empty() {
        let module = CoreModule {
            name: ModuleName::new(vec![Atom::new(1)]),
            exports: HashSet::new(),
            attributes: HashMap::new(),
            functions: vec![],
            compile_info: CoreCompileInfo {
                file: None,
                line: 1,
                vsn: None,
            },
        };
        assert!(validate_module(&module).is_ok());
    }

    #[test]
    fn test_compile_module_atom() {
        let atoms = AtomTable::new();
        let ast = AST::Atom(Atom::new(42));
        let result = compile_module(ast, atoms);
        assert!(result.is_ok());
    }
}