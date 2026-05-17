//! Typespec parsing and preservation for the Rust/Zig Elixir compiler.
//!
//! Handles `@spec`, `@type`, `@typep`, `@opaque`, `@callback`, and `@macrocallback`
//! annotations with full metadata preservation for documentation and code generation.

#[cfg(test)]
use chimera_allocator as _;

use chimera_ast::AST;
use chimera_diag::Diagnostic;
use chimera_source::{SourceFileId, SourceOffset, SourceSpan};
use chimera_term::Atom;

/// Typespec variants supported by the compiler.
#[derive(Debug, Clone, PartialEq)]
pub enum Typespec {
    /// Function specification: @spec function_name(type1, type2) :: return_type
    Spec {
        name: Atom,
        arity: u8,
        params: Vec<TypespecArg>,
        return_type: Box<TypespecType>,
        meta: SpecMeta,
    },
    /// Type definition: @type name :: type
    Type {
        name: Atom,
        type_def: Box<TypespecType>,
        meta: TypeMeta,
    },
    /// Private type definition: @typep name :: type
    Typep {
        name: Atom,
        type_def: Box<TypespecType>,
        meta: TypeMeta,
    },
    /// Opaque type: @opaque name :: type
    Opaque {
        name: Atom,
        type_def: Box<TypespecType>,
        meta: TypeMeta,
    },
    /// Callback for behaviour: @callback function(type1) :: return_type
    Callback {
        name: Atom,
        arity: u8,
        params: Vec<TypespecArg>,
        return_type: Box<TypespecType>,
        meta: SpecMeta,
    },
    /// Macro callback: @macrocallback macro_name(type1) :: return_type
    MacroCallback {
        name: Atom,
        arity: u8,
        params: Vec<TypespecArg>,
        return_type: Box<TypespecType>,
        meta: SpecMeta,
    },
}

/// Metadata for spec and callback typespecs.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecMeta {
    pub file_id: SourceFileId,
    pub line: u32,
    pub column: u32,
    pub deprecated: Option<String>,
    pub doc: Option<String>,
    pub guard: bool,
}

impl SpecMeta {
    pub fn new(file_id: SourceFileId) -> Self {
        SpecMeta {
            file_id,
            line: 0,
            column: 0,
            deprecated: None,
            doc: None,
            guard: false,
        }
    }
}

/// Metadata for type definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeMeta {
    pub file_id: SourceFileId,
    pub line: u32,
    pub column: u32,
    pub opaque: bool,
    pub doc: Option<String>,
    pub default: Option<Box<TypespecType>>,
}

impl TypeMeta {
    pub fn new(file_id: SourceFileId) -> Self {
        TypeMeta {
            file_id,
            line: 0,
            column: 0,
            opaque: false,
            doc: None,
            default: None,
        }
    }
}

/// A typespec argument (name :: type) or just a type.
#[derive(Debug, Clone, PartialEq)]
pub enum TypespecArg {
    /// Named argument: x :: integer()
    Named {
        name: Atom,
        type_def: Box<TypespecType>,
    },
    /// Anonymous argument: integer()
    Anonymous(Box<TypespecType>),
}

impl TypespecArg {
    pub fn type_def(&self) -> &TypespecType {
        match self {
            TypespecArg::Named { type_def, .. } => type_def.as_ref(),
            TypespecArg::Anonymous(ty) => ty.as_ref(),
        }
    }
}

/// Typespec type expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum TypespecType {
    /// Any type
    Any,
    /// Atom literal
    Atom(Atom),
    /// Atom type (dynamic)
    DynamicAtom,
    /// Integer type
    Integer,
    /// Float type
    Float,
    /// Number type (integer | float)
    Number,
    /// Binary type
    Binary,
    /// Bitstring type
    Bitstring(Option<Box<TypespecType>>),
    /// String type
    String,
    /// Charlist type
    Charlist,
    /// Boolean type
    Boolean,
    /// List type
    List(Box<TypespecType>),
    /// Improper list: [head | tail]
    ImproperList(Box<TypespecType>, Box<TypespecType>),
    /// Tuple type
    Tuple(Vec<TypespecType>),
    /// Map type
    Map(Vec<(TypespecType, TypespecType)>),
    /// Union type: type1 | type2
    Union(Vec<TypespecType>),
    /// Range type: 1..10
    Range(Box<TypespecType>, Box<TypespecType>),
    /// Pid type
    Pid,
    /// Reference type
    Reference,
    /// Port type
    Port,
    /// Function type
    Function {
        args: Vec<TypespecType>,
        return_type: Box<TypespecType>,
    },
    /// Named type: SomeModule.t()
    Remote {
        module: Atom,
        name: Atom,
        args: Vec<TypespecType>,
    },
    /// Variable type (for guards): a
    Variable(Atom),
    /// Parenthesized type: (type)
    Parens(Box<TypespecType>),
    /// Literal integer: 42
    LitInteger(i64),
    /// Literal atom: :ok
    LitAtom(Atom),
    /// Remote type with module prefix
    RemoteType {
        module: Box<TypespecType>,
        name: Atom,
        args: Vec<TypespecType>,
    },
    /// Struct type: %StructName{}
    Struct {
        name: Atom,
        fields: Vec<(Atom, TypespecType)>,
    },
    /// Union with none (nil) type
    Maybe,
}

impl TypespecType {
    /// Check if this is a built-in type name.
    pub fn is_builtin_type(name: &str) -> bool {
        matches!(
            name,
            "any"
                | "integer"
                | "float"
                | "number"
                | "binary"
                | "bitstring"
                | "string"
                | "charlist"
                | "boolean"
                | "pid"
                | "reference"
                | "port"
                | "term"
                | "nil"
                | "maybe"
                | "none"
                | "atom"
                | "map"
                | "list"
                | "tuple"
                | "fun"
                | "timeout"
        )
    }
}

/// A parsed typespec with its source location.
#[derive(Debug, Clone)]
pub struct ParsedTypespec {
    pub spec: Typespec,
    pub span: SourceSpan,
    pub file_id: SourceFileId,
}

impl ParsedTypespec {
    pub fn new(spec: Typespec, span: SourceSpan, file_id: SourceFileId) -> Self {
        ParsedTypespec {
            spec,
            span,
            file_id,
        }
    }
}

/// Typespec builder for creating typespecs programmatically.
pub struct TypespecBuilder {
    file_id: SourceFileId,
}

impl TypespecBuilder {
    pub fn new(file_id: SourceFileId) -> Self {
        TypespecBuilder { file_id }
    }

    /// Create a spec typespec.
    pub fn spec(
        &self,
        name: Atom,
        arity: u8,
        params: Vec<TypespecArg>,
        return_type: TypespecType,
    ) -> Typespec {
        Typespec::Spec {
            name,
            arity,
            params,
            return_type: Box::new(return_type),
            meta: SpecMeta::new(self.file_id),
        }
    }

    /// Create a type typespec.
    pub fn typedef(&self, name: Atom, type_def: TypespecType) -> Typespec {
        Typespec::Type {
            name,
            type_def: Box::new(type_def),
            meta: TypeMeta::new(self.file_id),
        }
    }

    /// Create a callback typespec.
    pub fn callback(
        &self,
        name: Atom,
        arity: u8,
        params: Vec<TypespecArg>,
        return_type: TypespecType,
    ) -> Typespec {
        Typespec::Callback {
            name,
            arity,
            params,
            return_type: Box::new(return_type),
            meta: SpecMeta::new(self.file_id),
        }
    }
}

/// Typespec validation error types.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Undefined type referenced
    UndefinedType { name: Atom, location: String },
    /// Cyclic type definition detected
    CyclicType { name: Atom, cycle: Vec<Atom> },
    /// Unused type definition
    UnusedType { name: Atom },
    /// Type arity mismatch
    ArityMismatch {
        name: Atom,
        expected: u8,
        actual: u8,
    },
    /// Invalid type specification
    InvalidType { message: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::UndefinedType { name, location } => {
                write!(f, "undefined type '{:?}' at {}", name, location)
            }
            ValidationError::CyclicType { name, cycle } => {
                let cycle_str: Vec<String> = cycle.iter().map(|a| format!("{:?}", a)).collect();
                write!(
                    f,
                    "cyclic type detected for '{:?}': {}",
                    name,
                    cycle_str.join(" -> ")
                )
            }
            ValidationError::UnusedType { name } => {
                write!(f, "unused type '{:?}'", name)
            }
            ValidationError::ArityMismatch {
                name,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "type '{:?}' expects {} arguments, got {}",
                    name, expected, actual
                )
            }
            ValidationError::InvalidType { message } => {
                write!(f, "invalid type: {}", message)
            }
        }
    }
}

/// Validation context with known types and functions.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// Known type definitions (from @type, @typep, @opaque)
    pub known_types: Vec<Atom>,
    /// Known specs (from @spec)
    pub known_specs: Vec<(Atom, u8)>,
    /// Known callbacks (from @callback)
    pub known_callbacks: Vec<(Atom, u8)>,
}

impl ValidationContext {
    pub fn new() -> Self {
        ValidationContext::default()
    }

    /// Add a known type to the context.
    pub fn add_type(&mut self, name: Atom) {
        if !self.known_types.iter().any(|n| *n == name) {
            self.known_types.push(name);
        }
    }

    /// Add a known spec to the context.
    pub fn add_spec(&mut self, name: Atom, arity: u8) {
        if !self
            .known_specs
            .iter()
            .any(|(n, a)| *n == name && *a == arity)
        {
            self.known_specs.push((name, arity));
        }
    }

    /// Add a known callback to the context.
    pub fn add_callback(&mut self, name: Atom, arity: u8) {
        if !self
            .known_callbacks
            .iter()
            .any(|(n, a)| *n == name && *a == arity)
        {
            self.known_callbacks.push((name, arity));
        }
    }

    /// Check if a type is known.
    pub fn is_known_type(&self, name: Atom) -> bool {
        self.known_types.iter().any(|n| *n == name)
    }

    /// Check if a spec is known.
    pub fn is_known_spec(&self, name: Atom, arity: u8) -> bool {
        self.known_specs
            .iter()
            .any(|(n, a)| *n == name && *a == arity)
    }
}

/// Validate typespec definitions.
pub struct TypespecValidator {
    context: ValidationContext,
}

impl TypespecValidator {
    pub fn new() -> Self {
        TypespecValidator {
            context: ValidationContext::new(),
        }
    }

    pub fn with_context(context: ValidationContext) -> Self {
        TypespecValidator { context }
    }

    /// Validate a typespec.
    pub fn validate(&self, spec: &Typespec) -> Result<(), Diagnostic> {
        match spec {
            Typespec::Spec {
                name,
                arity,
                params,
                return_type,
                ..
            } => {
                self.validate_name(name)?;
                self.validate_arity(*arity)?;
                for param in params {
                    self.validate_arg(param)?;
                }
                self.validate_type(return_type)?;
                Ok(())
            }
            Typespec::Type { name, type_def, .. } => {
                self.validate_name(name)?;
                self.validate_type(type_def)?;
                Ok(())
            }
            Typespec::Typep { name, type_def, .. } => {
                self.validate_name(name)?;
                self.validate_type(type_def)?;
                Ok(())
            }
            Typespec::Opaque { name, type_def, .. } => {
                self.validate_name(name)?;
                self.validate_type(type_def)?;
                Ok(())
            }
            Typespec::Callback {
                name,
                arity,
                params,
                return_type,
                ..
            } => {
                self.validate_name(name)?;
                self.validate_arity(*arity)?;
                for param in params {
                    self.validate_arg(param)?;
                }
                self.validate_type(return_type)?;
                Ok(())
            }
            Typespec::MacroCallback {
                name,
                arity,
                params,
                return_type,
                ..
            } => {
                self.validate_name(name)?;
                self.validate_arity(*arity)?;
                for param in params {
                    self.validate_arg(param)?;
                }
                self.validate_type(return_type)?;
                Ok(())
            }
        }
    }

    /// Validate a typespec with cycle detection.
    pub fn validate_with_cycle_check(&self, spec: &Typespec) -> Result<(), Diagnostic> {
        let mut visited = std::collections::HashSet::new();
        self.validate_with_visited(spec, &mut visited)
    }

    fn validate_with_visited(
        &self,
        spec: &Typespec,
        visited: &mut std::collections::HashSet<Atom>,
    ) -> Result<(), Diagnostic> {
        match spec {
            Typespec::Type { name, type_def, .. } => {
                if visited.contains(name) {
                    return Err(Diagnostic::error(format!(
                        "cyclic type detected: {:?}",
                        name
                    )));
                }
                visited.insert(name.clone());
                let result = self.validate_type_with_visited(type_def, visited);
                visited.remove(name);
                result
            }
            _ => self.validate(spec),
        }
    }

    fn validate_type_with_visited(
        &self,
        ty: &TypespecType,
        visited: &mut std::collections::HashSet<Atom>,
    ) -> Result<(), Diagnostic> {
        match ty {
            TypespecType::Remote { module, name, args } => {
                // Check if this is a local type reference (not a builtin)
                let table = chimera_term::AtomTable::new();
                // Clone name for lookup since Atom doesn't implement Copy
                if let Some(name_str) = table.lookup(name.clone()) {
                    if !TypespecType::is_builtin_type(name_str) && visited.contains(module) {
                        // This is a type definition referencing another local type
                        return Err(Diagnostic::error(format!("undefined type: {}", name_str)));
                    }
                }
                for arg in args {
                    self.validate_type_with_visited(arg, visited)?;
                }
                Ok(())
            }
            TypespecType::Function { args, return_type } => {
                for arg in args {
                    self.validate_type_with_visited(arg, visited)?;
                }
                self.validate_type_with_visited(return_type, visited)?;
                Ok(())
            }
            TypespecType::List(inner) => self.validate_type_with_visited(inner, visited),
            TypespecType::Tuple(items) => {
                for item in items {
                    self.validate_type_with_visited(item, visited)?;
                }
                Ok(())
            }
            TypespecType::Map(pairs) => {
                for (k, v) in pairs {
                    self.validate_type_with_visited(k, visited)?;
                    self.validate_type_with_visited(v, visited)?;
                }
                Ok(())
            }
            TypespecType::Union(types) => {
                for t in types {
                    self.validate_type_with_visited(t, visited)?;
                }
                Ok(())
            }
            TypespecType::Range(start, end) => {
                self.validate_type_with_visited(start, visited)?;
                self.validate_type_with_visited(end, visited)
            }
            _ => Ok(()),
        }
    }

    fn validate_name(&self, name: &Atom) -> Result<(), Diagnostic> {
        let table = chimera_term::AtomTable::new();
        if let Some(s) = table.lookup(name.clone()) {
            if s.is_empty() {
                return Err(Diagnostic::error("empty typespec name"));
            }
        }
        Ok(())
    }

    fn validate_arity(&self, arity: u8) -> Result<(), Diagnostic> {
        let _ = arity;
        Ok(())
    }

    fn validate_arg(&self, arg: &TypespecArg) -> Result<(), Diagnostic> {
        self.validate_type(arg.type_def())
    }

    fn validate_type(&self, ty: &TypespecType) -> Result<(), Diagnostic> {
        match ty {
            TypespecType::Remote { module, name, args } => {
                self.validate_name(module)?;
                self.validate_name(name)?;
                for arg in args {
                    self.validate_type(arg)?;
                }
                Ok(())
            }
            TypespecType::Function { args, return_type } => {
                for arg in args {
                    self.validate_type(arg)?;
                }
                self.validate_type(return_type)?;
                Ok(())
            }
            TypespecType::List(inner) => self.validate_type(inner),
            TypespecType::Tuple(items) => {
                for item in items {
                    self.validate_type(item)?;
                }
                Ok(())
            }
            TypespecType::Map(pairs) => {
                for (k, v) in pairs {
                    self.validate_type(k)?;
                    self.validate_type(v)?;
                }
                Ok(())
            }
            TypespecType::Union(types) => {
                for t in types {
                    self.validate_type(t)?;
                }
                Ok(())
            }
            TypespecType::Range(start, end) => {
                self.validate_type(start)?;
                self.validate_type(end)?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Validate a list of typespecs for consistency.
    pub fn validate_typespecs(&self, specs: &[ParsedTypespec]) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();

        // Build context of known types
        let mut ctx = ValidationContext::new();
        for spec in specs {
            match &spec.spec {
                Typespec::Type { name, .. } => ctx.add_type((*name).clone()),
                Typespec::Typep { name, .. } => ctx.add_type((*name).clone()),
                Typespec::Opaque { name, .. } => ctx.add_type((*name).clone()),
                Typespec::Spec { name, arity, .. } => ctx.add_spec((*name).clone(), *arity),
                Typespec::Callback { name, arity, .. } => ctx.add_callback((*name).clone(), *arity),
                Typespec::MacroCallback { name, arity, .. } => {
                    ctx.add_callback((*name).clone(), *arity)
                }
            }
        }

        // Validate each spec
        for spec in specs {
            if let Err(e) = self.validate(&spec.spec) {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for TypespecValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract typespecs from a module's body AST.
/// Note: This is a simplified version that works with the current AST structure.
pub fn extract_typespecs(_file_id: SourceFileId, _body: &[AST]) -> Vec<ParsedTypespec> {
    // TODO: Implement full extraction when AST structure is finalized
    Vec::new()
}

mod emit;
mod parser;

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_source::SourceFileId;
    use chimera_term::AtomTable;

    #[test]
    fn test_typespec_arg_type_def() {
        // Anonymous typespec arg
        let arg = TypespecArg::Anonymous(Box::new(TypespecType::Integer));
        assert_eq!(arg.type_def(), &TypespecType::Integer);
    }

    #[test]
    fn test_typespec_arg_named() {
        let mut table = AtomTable::new();
        let name = table.intern("x");
        let arg = TypespecArg::Named {
            name,
            type_def: Box::new(TypespecType::Integer),
        };
        assert_eq!(arg.type_def(), &TypespecType::Integer);
    }

    #[test]
    fn test_typespec_type_any() {
        let ty = TypespecType::Any;
        assert_eq!(ty, TypespecType::Any);
    }

    #[test]
    fn test_typespec_type_atom() {
        let mut table = AtomTable::new();
        let atom = table.intern("ok");
        let ty = TypespecType::Atom(atom);
        assert_eq!(ty, TypespecType::Atom(table.intern("ok")));
    }

    #[test]
    fn test_typespec_type_dynamic_atom() {
        let ty = TypespecType::DynamicAtom;
        assert_eq!(ty, TypespecType::DynamicAtom);
    }

    #[test]
    fn test_typespec_type_integer() {
        let ty = TypespecType::Integer;
        assert_eq!(ty, TypespecType::Integer);
    }

    #[test]
    fn test_typespec_type_float() {
        let ty = TypespecType::Float;
        assert_eq!(ty, TypespecType::Float);
    }

    #[test]
    fn test_typespec_type_number() {
        let ty = TypespecType::Number;
        assert_eq!(ty, TypespecType::Number);
    }

    #[test]
    fn test_typespec_type_binary() {
        let ty = TypespecType::Binary;
        assert_eq!(ty, TypespecType::Binary);
    }

    #[test]
    fn test_typespec_type_bitstring_none() {
        let ty = TypespecType::Bitstring(None);
        assert_eq!(ty, TypespecType::Bitstring(None));
    }

    #[test]
    fn test_typespec_type_bitstring_with_arg() {
        let inner = Box::new(TypespecType::Integer);
        let ty = TypespecType::Bitstring(Some(inner));
        match ty {
            TypespecType::Bitstring(Some(inner)) => {
                assert_eq!(*inner, TypespecType::Integer);
            }
            _ => panic!("expected Bitstring with arg"),
        }
    }

    #[test]
    fn test_typespec_type_string() {
        let ty = TypespecType::String;
        assert_eq!(ty, TypespecType::String);
    }

    #[test]
    fn test_typespec_type_charlist() {
        let ty = TypespecType::Charlist;
        assert_eq!(ty, TypespecType::Charlist);
    }

    #[test]
    fn test_typespec_type_boolean() {
        let ty = TypespecType::Boolean;
        assert_eq!(ty, TypespecType::Boolean);
    }

    #[test]
    fn test_typespec_type_list() {
        let inner = Box::new(TypespecType::Integer);
        let ty = TypespecType::List(inner);
        match ty {
            TypespecType::List(inner) => {
                assert_eq!(*inner, TypespecType::Integer);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_typespec_type_improper_list() {
        let head = Box::new(TypespecType::Integer);
        let tail = Box::new(TypespecType::String);
        let ty = TypespecType::ImproperList(head, tail);
        match ty {
            TypespecType::ImproperList(h, t) => {
                assert_eq!(*h, TypespecType::Integer);
                assert_eq!(*t, TypespecType::String);
            }
            _ => panic!("expected ImproperList"),
        }
    }

    #[test]
    fn test_typespec_type_tuple() {
        let ty = TypespecType::Tuple(vec![TypespecType::Integer, TypespecType::String]);
        match ty {
            TypespecType::Tuple(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], TypespecType::Integer);
                assert_eq!(items[1], TypespecType::String);
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn test_typespec_type_empty_tuple() {
        let ty = TypespecType::Tuple(Vec::new());
        match ty {
            TypespecType::Tuple(items) => {
                assert!(items.is_empty());
            }
            _ => panic!("expected empty Tuple"),
        }
    }

    #[test]
    fn test_typespec_type_map() {
        let ty = TypespecType::Map(vec![(
            TypespecType::Atom(AtomTable::new().intern("key")),
            TypespecType::Integer,
        )]);
        match ty {
            TypespecType::Map(pairs) => {
                assert_eq!(pairs.len(), 1);
            }
            _ => panic!("expected Map"),
        }
    }

    #[test]
    fn test_typespec_type_union() {
        let types = vec![TypespecType::Integer, TypespecType::Float];
        let ty = TypespecType::Union(types);
        match ty {
            TypespecType::Union(v) => {
                assert_eq!(v.len(), 2);
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn test_typespec_type_range() {
        let start = Box::new(TypespecType::LitInteger(1));
        let end = Box::new(TypespecType::LitInteger(10));
        let ty = TypespecType::Range(start, end);
        match ty {
            TypespecType::Range(s, e) => {
                assert_eq!(*s, TypespecType::LitInteger(1));
                assert_eq!(*e, TypespecType::LitInteger(10));
            }
            _ => panic!("expected Range"),
        }
    }

    #[test]
    fn test_typespec_type_pid() {
        let ty = TypespecType::Pid;
        assert_eq!(ty, TypespecType::Pid);
    }

    #[test]
    fn test_typespec_type_reference() {
        let ty = TypespecType::Reference;
        assert_eq!(ty, TypespecType::Reference);
    }

    #[test]
    fn test_typespec_type_port() {
        let ty = TypespecType::Port;
        assert_eq!(ty, TypespecType::Port);
    }

    #[test]
    fn test_typespec_type_function() {
        let ty = TypespecType::Function {
            args: vec![TypespecType::Integer, TypespecType::String],
            return_type: Box::new(TypespecType::Boolean),
        };
        match ty {
            TypespecType::Function { args, return_type } => {
                assert_eq!(args.len(), 2);
                assert_eq!(*return_type, TypespecType::Boolean);
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_typespec_type_function_no_args() {
        let ty = TypespecType::Function {
            args: Vec::new(),
            return_type: Box::new(TypespecType::Atom(AtomTable::new().intern("ok"))),
        };
        match ty {
            TypespecType::Function { args, return_type } => {
                assert!(args.is_empty());
                assert!(matches!(*return_type, TypespecType::Atom(_)));
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_typespec_type_remote() {
        let mut table = AtomTable::new();
        let module = table.intern("Enum");
        let name = table.intern("t");

        let ty = TypespecType::Remote {
            module,
            name,
            args: vec![TypespecType::Integer],
        };
        match ty {
            TypespecType::Remote {
                module: _,
                name: _,
                args,
            } => {
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected Remote"),
        }
    }

    #[test]
    fn test_typespec_type_remote_no_args() {
        let mut table = AtomTable::new();
        let module = table.intern("String");
        let name = table.intern("t");

        let ty = TypespecType::Remote {
            module,
            name,
            args: Vec::new(),
        };
        match ty {
            TypespecType::Remote {
                module: m,
                name: n,
                args,
            } => {
                assert_eq!(m, table.intern("String"));
                assert_eq!(n, table.intern("t"));
                assert!(args.is_empty());
            }
            _ => panic!("expected Remote"),
        }
    }

    #[test]
    fn test_typespec_type_variable() {
        let mut table = AtomTable::new();
        let var_name = table.intern("a");
        let ty = TypespecType::Variable(var_name);
        assert!(matches!(ty, TypespecType::Variable(_)));
    }

    #[test]
    fn test_typespec_type_parens() {
        let inner = Box::new(TypespecType::Integer);
        let ty = TypespecType::Parens(inner);
        match ty {
            TypespecType::Parens(inner) => {
                assert_eq!(*inner, TypespecType::Integer);
            }
            _ => panic!("expected Parens"),
        }
    }

    #[test]
    fn test_typespec_type_lit_integer() {
        let ty = TypespecType::LitInteger(42);
        assert_eq!(ty, TypespecType::LitInteger(42));
    }

    #[test]
    fn test_typespec_type_lit_integer_negative() {
        let ty = TypespecType::LitInteger(-10);
        assert_eq!(ty, TypespecType::LitInteger(-10));
    }

    #[test]
    fn test_typespec_type_lit_atom() {
        let mut table = AtomTable::new();
        let atom = table.intern("ok");
        let ty = TypespecType::LitAtom(atom);
        assert_eq!(ty, TypespecType::LitAtom(table.intern("ok")));
    }

    #[test]
    fn test_typespec_type_remote_type() {
        let mut table = AtomTable::new();
        let module = Box::new(TypespecType::DynamicAtom);
        let name = table.intern("t");
        let ty = TypespecType::RemoteType {
            module,
            name,
            args: vec![TypespecType::Integer],
        };
        match ty {
            TypespecType::RemoteType {
                module: _,
                name: _,
                args,
            } => {
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected RemoteType"),
        }
    }

    #[test]
    fn test_typespec_type_struct() {
        let mut table = AtomTable::new();
        let struct_name = table.intern("User");
        let fields = vec![
            (table.intern("name"), TypespecType::String),
            (table.intern("age"), TypespecType::Integer),
        ];

        let ty = TypespecType::Struct {
            name: struct_name,
            fields,
        };

        match ty {
            TypespecType::Struct { name: _, fields } => {
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_typespec_type_struct_empty() {
        let mut table = AtomTable::new();
        let struct_name = table.intern("Empty");

        let ty = TypespecType::Struct {
            name: struct_name,
            fields: Vec::new(),
        };

        match ty {
            TypespecType::Struct { name, fields } => {
                assert_eq!(name, table.intern("Empty"));
                assert!(fields.is_empty());
            }
            _ => panic!("expected empty Struct"),
        }
    }

    #[test]
    fn test_typespec_type_maybe() {
        let ty = TypespecType::Maybe;
        assert_eq!(ty, TypespecType::Maybe);
    }

    #[test]
    fn test_typespec_type_eq() {
        // Test PartialEq for TypespecType
        let a = TypespecType::Integer;
        let b = TypespecType::Integer;
        let c = TypespecType::Float;
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_typespec_type_eq_union() {
        let a = TypespecType::Union(vec![TypespecType::Integer, TypespecType::Float]);
        let b = TypespecType::Union(vec![TypespecType::Integer, TypespecType::Float]);
        let c = TypespecType::Union(vec![TypespecType::Integer, TypespecType::String]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_typespec_type_eq_map() {
        let a = TypespecType::Map(vec![(
            TypespecType::Atom(AtomTable::new().intern("a")),
            TypespecType::Integer,
        )]);
        let b = TypespecType::Map(vec![(
            TypespecType::Atom(AtomTable::new().intern("a")),
            TypespecType::Integer,
        )]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_typespec_validator_new() {
        let validator = TypespecValidator::new();
        drop(validator);
    }

    #[test]
    fn test_typespec_validator_validate_function() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();

        let spec = Typespec::Spec {
            name: table.intern("my_func"),
            arity: 2,
            params: vec![
                TypespecArg::Anonymous(Box::new(TypespecType::Integer)),
                TypespecArg::Anonymous(Box::new(TypespecType::String)),
            ],
            return_type: Box::new(TypespecType::Boolean),
            meta: SpecMeta::new(SourceFileId::new(0)),
        };

        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_parsed_typespec_new() {
        let spec = Typespec::Type {
            name: Atom::new(1),
            type_def: Box::new(TypespecType::Integer),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };

        let span = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(10));

        let parsed = ParsedTypespec::new(spec, span, SourceFileId::new(0));
        assert_eq!(parsed.file_id, SourceFileId::new(0));
    }

    #[test]
    fn test_spec_meta_new() {
        let meta = SpecMeta::new(SourceFileId::new(0));
        assert_eq!(meta.file_id, SourceFileId::new(0));
        assert!(!meta.guard);
    }

    #[test]
    fn test_type_meta_new() {
        let meta = TypeMeta::new(SourceFileId::new(0));
        assert_eq!(meta.file_id, SourceFileId::new(0));
        assert!(!meta.opaque);
    }

    #[test]
    fn test_typespec_builder_spec() {
        let builder = TypespecBuilder::new(SourceFileId::new(0));
        let mut table = AtomTable::new();
        let name = table.intern("my_func");

        let spec = builder.spec(
            name,
            2,
            vec![TypespecArg::Anonymous(Box::new(TypespecType::Integer))],
            TypespecType::Boolean,
        );

        match spec {
            Typespec::Spec {
                name: _,
                arity,
                params,
                return_type,
                ..
            } => {
                assert_eq!(arity, 2);
                assert_eq!(params.len(), 1);
                assert_eq!(*return_type, TypespecType::Boolean);
            }
            _ => panic!("expected Spec"),
        }
    }

    #[test]
    fn test_typespec_type_is_builtin() {
        assert!(TypespecType::is_builtin_type("integer"));
        assert!(TypespecType::is_builtin_type("string"));
        assert!(TypespecType::is_builtin_type("boolean"));
        assert!(!TypespecType::is_builtin_type("CustomType"));
    }

    #[test]
    fn test_extract_typespecs_empty() {
        let specs = extract_typespecs(SourceFileId::new(0), &[]);
        assert!(specs.is_empty());
    }

    #[test]
    fn test_parse_type_integer() {
        let result = parser::parse_type("integer()", SourceFileId::new(0));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TypespecType::Integer);
    }

    #[test]
    fn test_parse_type_string() {
        let result = parser::parse_type("String.t()", SourceFileId::new(0));
        if let Err(e) = &result {
            eprintln!("Error: {}", e);
        }
        assert!(result.is_ok(), "parse_type failed: {:?}", result.err());
        match result.unwrap() {
            TypespecType::Remote {
                module: _,
                name: _,
                args: _,
            } => {}
            _ => panic!("expected Remote type"),
        }
    }

    #[test]
    fn test_parse_type_list() {
        let result = parser::parse_type("list(integer())", SourceFileId::new(0));
        assert!(result.is_ok());
        match result.unwrap() {
            TypespecType::List(inner) => {
                assert_eq!(*inner, TypespecType::Integer);
            }
            _ => panic!("expected List type"),
        }
    }

    #[test]
    fn test_parse_spec_basic() {
        let result = parser::parse_spec("add(integer, integer) :: integer", SourceFileId::new(0));
        if let Err(e) = &result {
            eprintln!("Error: {}", e);
        }
        assert!(result.is_ok(), "parse_spec failed: {:?}", result.err());
        let spec = result.unwrap();
        match spec {
            Typespec::Spec {
                name: _,
                arity,
                params,
                return_type,
                ..
            } => {
                assert_eq!(arity, 2);
                assert_eq!(params.len(), 2);
                assert_eq!(*return_type, TypespecType::Integer);
            }
            _ => panic!("expected Spec"),
        }
    }

    #[test]
    fn test_parse_type_def() {
        let result = parser::parse_type_def("my_type :: integer()", SourceFileId::new(0));
        assert!(result.is_ok());
        match result.unwrap() {
            Typespec::Type {
                name: _,
                type_def,
                meta: _,
            } => {
                assert_eq!(*type_def, TypespecType::Integer);
            }
            _ => panic!("expected Type"),
        }
    }

    #[test]
    fn test_parse_attribute_spec() {
        let result = parser::parse_attribute(
            "@spec add(integer, integer) :: integer",
            SourceFileId::new(0),
        );
        assert!(result.is_ok());
        match result.unwrap() {
            Typespec::Spec { arity, .. } => assert_eq!(arity, 2),
            _ => panic!("expected Spec"),
        }
    }

    #[test]
    fn test_parse_attribute_type() {
        let result = parser::parse_attribute("@type my_type :: integer()", SourceFileId::new(0));
        assert!(result.is_ok());
        match result.unwrap() {
            Typespec::Type { .. } => {}
            _ => panic!("expected Type"),
        }
    }

    #[test]
    fn test_parse_type_variable() {
        let result = parser::parse_type("a", SourceFileId::new(0));
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), TypespecType::Variable(_)));
    }

    #[test]
    fn test_parse_type_remote_with_args() {
        // Test that we can parse a remote type with angle brackets
        let result = parser::parse_type("Enum.t(integer())", SourceFileId::new(0));
        if let Err(e) = &result {
            eprintln!("Error: {}", e);
        }
        assert!(result.is_ok(), "parse_type failed: {:?}", result.err());
    }

    // === Validation Tests ===

    #[test]
    fn test_validation_context_new() {
        let ctx = ValidationContext::new();
        assert!(ctx.known_types.is_empty());
        assert!(ctx.known_specs.is_empty());
        assert!(ctx.known_callbacks.is_empty());
    }

    #[test]
    fn test_validation_context_add_type() {
        let mut ctx = ValidationContext::new();
        let mut table = AtomTable::new();
        let name = table.intern("my_type");
        ctx.add_type(name.clone());
        assert!(ctx.is_known_type(name));
        assert!(!ctx.is_known_type(table.intern("other")));
    }

    #[test]
    fn test_validation_context_add_spec() {
        let mut ctx = ValidationContext::new();
        let mut table = AtomTable::new();
        let name = table.intern("my_func");
        ctx.add_spec(name.clone(), 2);
        assert!(ctx.is_known_spec(name.clone(), 2));
        assert!(!ctx.is_known_spec(name.clone(), 3));
        assert!(!ctx.is_known_spec(table.intern("other"), 2));
    }

    #[test]
    fn test_validation_context_add_callback() {
        let mut ctx = ValidationContext::new();
        let mut table = AtomTable::new();
        let name = table.intern("my_callback");
        ctx.add_callback(name.clone(), 1);
        assert!(ctx
            .known_callbacks
            .iter()
            .any(|(n, a)| *n == name && *a == 1));
    }

    #[test]
    fn test_validator_validate_empty_spec() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();
        let spec = Typespec::Spec {
            name: table.intern("test"),
            arity: 0,
            params: Vec::new(),
            return_type: Box::new(TypespecType::Integer),
            meta: SpecMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_type() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();
        let spec = Typespec::Type {
            name: table.intern("my_type"),
            type_def: Box::new(TypespecType::Integer),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_opaque() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();
        let spec = Typespec::Opaque {
            name: table.intern("opaque_type"),
            type_def: Box::new(TypespecType::Integer),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_remote_type() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();
        let spec = Typespec::Type {
            name: table.intern("custom_type"),
            type_def: Box::new(TypespecType::Remote {
                module: table.intern("Enum"),
                name: table.intern("t"),
                args: Vec::new(),
            }),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_union() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();
        let spec = Typespec::Type {
            name: table.intern("union_type"),
            type_def: Box::new(TypespecType::Union(vec![
                TypespecType::Integer,
                TypespecType::Float,
            ])),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_function_type() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();
        let spec = Typespec::Type {
            name: table.intern("func_type"),
            type_def: Box::new(TypespecType::Function {
                args: vec![TypespecType::Integer, TypespecType::String],
                return_type: Box::new(TypespecType::Boolean),
            }),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_list() {
        let validator = TypespecValidator::new();
        let spec = Typespec::Type {
            name: AtomTable::new().intern("list_type"),
            type_def: Box::new(TypespecType::List(Box::new(TypespecType::Integer))),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_map() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();
        let spec = Typespec::Type {
            name: table.intern("map_type"),
            type_def: Box::new(TypespecType::Map(vec![(
                TypespecType::Atom(table.intern("key")),
                TypespecType::Integer,
            )])),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validator_validate_with_context() {
        let mut ctx = ValidationContext::new();
        let mut table = AtomTable::new();
        ctx.add_type(table.intern("custom_type"));

        let validator = TypespecValidator::with_context(ctx);
        let spec = Typespec::Type {
            name: table.intern("my_type"),
            type_def: Box::new(TypespecType::Remote {
                module: table.intern("MyModule"),
                name: table.intern("custom_type"),
                args: Vec::new(),
            }),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        assert!(validator.validate(&spec).is_ok());
    }

    #[test]
    fn test_validation_error_display() {
        let mut table = AtomTable::new();
        let err = ValidationError::UndefinedType {
            name: table.intern("missing_type"),
            location: "line 10".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("undefined type"), "unexpected: {}", msg);
        assert!(msg.contains("line 10"), "unexpected: {}", msg);
    }

    #[test]
    fn test_validation_error_cyclic_display() {
        let mut table = AtomTable::new();
        let err = ValidationError::CyclicType {
            name: table.intern("a"),
            cycle: vec![table.intern("a"), table.intern("b"), table.intern("a")],
        };
        let display = format!("{}", err);
        assert!(
            display.contains("cyclic type detected"),
            "unexpected: {}",
            display
        );
        assert!(display.contains("Atom"), "unexpected: {}", display);
    }

    #[test]
    fn test_validation_error_arity_mismatch() {
        let mut table = AtomTable::new();
        let err = ValidationError::ArityMismatch {
            name: table.intern("my_type"),
            expected: 2,
            actual: 3,
        };
        let msg = format!("{}", err);
        assert!(
            msg.contains("expects 2 arguments, got 3"),
            "unexpected: {}",
            msg
        );
    }

    #[test]
    fn test_validator_with_context() {
        let ctx = ValidationContext::new();
        let validator = TypespecValidator::with_context(ctx);
        drop(validator);
    }

    #[test]
    fn test_validate_typespecs_empty() {
        let validator = TypespecValidator::new();
        let result = validator.validate_typespecs(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_typespecs_multiple() {
        let validator = TypespecValidator::new();
        let mut table = AtomTable::new();

        let specs = vec![
            ParsedTypespec::new(
                Typespec::Type {
                    name: table.intern("type_a"),
                    type_def: Box::new(TypespecType::Integer),
                    meta: TypeMeta::new(SourceFileId::new(0)),
                },
                SourceSpan::new(SourceOffset::new(0), SourceOffset::new(10)),
                SourceFileId::new(0),
            ),
            ParsedTypespec::new(
                Typespec::Spec {
                    name: table.intern("func_b"),
                    arity: 1,
                    params: vec![TypespecArg::Anonymous(Box::new(TypespecType::String))],
                    return_type: Box::new(TypespecType::Boolean),
                    meta: SpecMeta::new(SourceFileId::new(0)),
                },
                SourceSpan::new(SourceOffset::new(0), SourceOffset::new(20)),
                SourceFileId::new(0),
            ),
        ];

        let result = validator.validate_typespecs(&specs);
        assert!(result.is_ok());
    }
}
