//! Module builder for the Rust/Zig Elixir compiler.
//!
//! This module provides module construction services including:
//! - defmodule module creation
//! - def/defp/defmacro/defmacrop function definitions
//! - Module attributes (@)
//! - Guard compilation support
//! - Function clause handling

#[cfg(test)]
use chimera_allocator as _;

use chimera_ast::{Meta, AST};
use chimera_parser::Parser;
use chimera_source::SourceFileId;
use chimera_term::{Atom, ModuleName, SharedAtomTable};
use std::collections::HashMap;

/// Module builder for constructing module definitions.
#[derive(Debug, Clone)]
pub struct ModuleBuilder {
    /// Current module being built
    pub module_name: Option<ModuleName>,
    /// Module attributes
    pub attributes: HashMap<Atom, AST>,
    /// Function definitions
    pub functions: Vec<FunctionDef>,
    /// Macros defined in this module
    pub macros: Vec<MacroDef>,
    /// Nested modules
    pub nested_modules: Vec<Module>,
    /// Atom table for interning
    atoms: SharedAtomTable,
}

/// A function definition in a module.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    /// Function name
    pub name: Atom,
    /// Whether it's a public function (def) or private (defp)
    pub public: bool,
    /// Function clauses (raw AST expressions from body)
    pub clauses: Vec<AST>,
    /// Meta information
    pub meta: Meta,
}

/// A macro definition.
#[derive(Debug, Clone)]
pub struct MacroDef {
    /// Macro name
    pub name: Atom,
    /// Whether it's a public macro (defmacro) or private (defmacrop)
    pub public: bool,
    /// Macro clauses (raw AST from body)
    pub clauses: Vec<AST>,
    /// Meta information
    pub meta: Meta,
}

/// A complete nested module.
#[derive(Debug, Clone)]
pub struct Module {
    /// Module name
    pub name: ModuleName,
    /// Module attributes
    pub attributes: HashMap<Atom, AST>,
    /// Function definitions
    pub functions: Vec<FunctionDef>,
    /// Macros defined in this module
    pub macros: Vec<MacroDef>,
    /// Nested modules
    pub nested_modules: Vec<Module>,
    /// Module body expressions
    pub body: Vec<AST>,
}

impl ModuleBuilder {
    /// Create a new module builder.
    pub fn new(atoms: SharedAtomTable) -> Self {
        ModuleBuilder {
            module_name: None,
            attributes: HashMap::new(),
            functions: Vec::new(),
            macros: Vec::new(),
            nested_modules: Vec::new(),
            atoms,
        }
    }

    /// Set the module name.
    pub fn set_module_name(&mut self, name: ModuleName) {
        self.module_name = Some(name);
    }

    /// Add a module attribute.
    pub fn add_attribute(&mut self, name: Atom, value: AST) {
        self.attributes.insert(name, value);
    }

    /// Get a module attribute.
    pub fn get_attribute(&self, name: &Atom) -> Option<&AST> {
        self.attributes.get(name)
    }

    /// Add a function definition.
    pub fn add_function(&mut self, func: FunctionDef) {
        self.functions.push(func);
    }

    /// Add a macro definition.
    pub fn add_macro(&mut self, macro_def: MacroDef) {
        self.macros.push(macro_def);
    }

    /// Add a nested module.
    pub fn add_nested_module(&mut self, module: Module) {
        self.nested_modules.push(module);
    }

    /// Build the final module AST.
    pub fn build(mut self) -> AST {
        let module_name = match self.module_name.take() {
            Some(name) => name,
            None => return AST::Tuple(vec![]),
        };

        let mut body = Vec::new();

        // Add module attributes as @attribute calls
        for (name, value) in &self.attributes {
            let attr_ast = AST::Call {
                name: name.clone(),
                meta: Meta::default(),
                args: vec![value.clone()],
            };
            body.push(attr_ast);
        }

        // Add function definitions
        for func in &self.functions {
            let ast = if func.public {
                AST::Def {
                    name: func.name.clone(),
                    meta: func.meta.clone(),
                    clauses: func.clauses.clone(),
                }
            } else {
                AST::Defp {
                    name: func.name.clone(),
                    meta: func.meta.clone(),
                    clauses: func.clauses.clone(),
                }
            };
            body.push(ast);
        }

        // Add macro definitions
        for macro_def in &self.macros {
            let ast = if macro_def.public {
                AST::Defmacro {
                    name: macro_def.name.clone(),
                    meta: macro_def.meta.clone(),
                    clauses: macro_def.clauses.clone(),
                }
            } else {
                AST::Defmacrop {
                    name: macro_def.name.clone(),
                    meta: macro_def.meta.clone(),
                    clauses: macro_def.clauses.clone(),
                }
            };
            body.push(ast);
        }

        // Add nested modules
        for nested in &self.nested_modules {
            body.push(nested.to_ast());
        }

        // Use first segment of module name as the atom
        let name_atom = module_name
            .segments()
            .first()
            .cloned()
            .unwrap_or(Atom::new(0));
        AST::Defmodule {
            name: Box::new(AST::Atom(name_atom)),
            body,
            meta: Meta::default(),
        }
    }

    /// Compile a module from source code.
    #[allow(dead_code)]
    pub fn compile_source(
        &mut self,
        source: &'static str,
        file_id: SourceFileId,
    ) -> Result<AST, ModuleError> {
        let mut parser = Parser::new(source, file_id);
        let asts = parser.parse_source()?;

        // Process module-level forms
        self.process_module_forms(asts)
    }

    /// Process module-level forms (def, defp, defmacro, etc.)
    #[allow(dead_code)]
    fn process_module_forms(&mut self, asts: Vec<AST>) -> Result<AST, ModuleError> {
        for ast in asts {
            if let AST::Defmodule {
                name,
                body,
                meta: _,
            } = ast
            {
                // Extract module name
                if let AST::Atom(atom) = *name {
                    self.module_name = Some(ModuleName::new(vec![atom]));
                }

                // Process each form in the body
                for form in body {
                    self.process_form(form)?;
                }
            }
        }
        // Build result from current state without consuming self
        let result = self.build_from_state();
        Ok(result)
    }

    /// Build AST from current state without consuming the builder.
    fn build_from_state(&self) -> AST {
        let module_name = match &self.module_name {
            Some(name) => name.clone(),
            None => return AST::Tuple(vec![]),
        };

        let mut body = Vec::new();

        // Add module attributes as @attribute calls
        for (name, value) in &self.attributes {
            let attr_ast = AST::Call {
                name: name.clone(),
                meta: Meta::default(),
                args: vec![value.clone()],
            };
            body.push(attr_ast);
        }

        // Add function definitions
        for func in &self.functions {
            let ast = if func.public {
                AST::Def {
                    name: func.name.clone(),
                    meta: func.meta.clone(),
                    clauses: func.clauses.clone(),
                }
            } else {
                AST::Defp {
                    name: func.name.clone(),
                    meta: func.meta.clone(),
                    clauses: func.clauses.clone(),
                }
            };
            body.push(ast);
        }

        // Add macro definitions
        for macro_def in &self.macros {
            let ast = if macro_def.public {
                AST::Defmacro {
                    name: macro_def.name.clone(),
                    meta: macro_def.meta.clone(),
                    clauses: macro_def.clauses.clone(),
                }
            } else {
                AST::Defmacrop {
                    name: macro_def.name.clone(),
                    meta: macro_def.meta.clone(),
                    clauses: macro_def.clauses.clone(),
                }
            };
            body.push(ast);
        }

        // Add nested modules
        for nested in &self.nested_modules {
            body.push(nested.to_ast());
        }

        // Use first segment of module name as the atom
        let name_atom = module_name
            .segments()
            .first()
            .cloned()
            .unwrap_or(Atom::new(0));
        AST::Defmodule {
            name: Box::new(AST::Atom(name_atom)),
            body,
            meta: Meta::default(),
        }
    }

    /// Process a single form in the module body.
    #[allow(dead_code)]
    fn process_form(&mut self, form: AST) -> Result<(), ModuleError> {
        match form {
            AST::Def {
                name,
                clauses,
                meta,
                ..
            } => {
                let func = FunctionDef {
                    name,
                    public: true,
                    clauses,
                    meta,
                };
                self.add_function(func);
            }
            AST::Defp {
                name,
                clauses,
                meta,
                ..
            } => {
                let func = FunctionDef {
                    name,
                    public: false,
                    clauses,
                    meta,
                };
                self.add_function(func);
            }
            AST::Defmacro {
                name,
                clauses,
                meta,
                ..
            } => {
                let macro_def = MacroDef {
                    name,
                    public: true,
                    clauses,
                    meta,
                };
                self.add_macro(macro_def);
            }
            AST::Defmacrop {
                name,
                clauses,
                meta,
                ..
            } => {
                let macro_def = MacroDef {
                    name,
                    public: false,
                    clauses,
                    meta,
                };
                self.add_macro(macro_def);
            }
            _ => {}
        }
        Ok(())
    }
}

impl Module {
    /// Convert a module to its AST representation.
    pub fn to_ast(&self) -> AST {
        let mut body = Vec::new();

        // Add module attributes as @attribute calls
        for (name, value) in &self.attributes {
            let attr_ast = AST::Call {
                name: name.clone(),
                meta: Meta::default(),
                args: vec![value.clone()],
            };
            body.push(attr_ast);
        }

        // Add function definitions
        for func in &self.functions {
            let ast = if func.public {
                AST::Def {
                    name: func.name.clone(),
                    meta: func.meta.clone(),
                    clauses: func.clauses.clone(),
                }
            } else {
                AST::Defp {
                    name: func.name.clone(),
                    meta: func.meta.clone(),
                    clauses: func.clauses.clone(),
                }
            };
            body.push(ast);
        }

        // Add macro definitions
        for macro_def in &self.macros {
            let ast = if macro_def.public {
                AST::Defmacro {
                    name: macro_def.name.clone(),
                    meta: macro_def.meta.clone(),
                    clauses: macro_def.clauses.clone(),
                }
            } else {
                AST::Defmacrop {
                    name: macro_def.name.clone(),
                    meta: macro_def.meta.clone(),
                    clauses: macro_def.clauses.clone(),
                }
            };
            body.push(ast);
        }

        // Add nested modules
        for nested in &self.nested_modules {
            body.push(nested.to_ast());
        }

        // Use first segment of module name as the atom
        let name_atom = self
            .name
            .segments()
            .first()
            .cloned()
            .unwrap_or(Atom::new(0));
        AST::Defmodule {
            name: Box::new(AST::Atom(name_atom)),
            body,
            meta: Meta::default(),
        }
    }
}

/// Module compilation error.
#[derive(Debug, Clone)]
pub enum ModuleError {
    /// Parse error
    Parse(String),
    /// Expand error
    Expand(String),
    /// Invalid module name
    InvalidModuleName,
    /// Missing required attribute
    MissingAttribute(Atom),
    /// Duplicate definition
    DuplicateDefinition(String),
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleError::Parse(s) => write!(f, "Parse error: {}", s),
            ModuleError::Expand(s) => write!(f, "Expand error: {}", s),
            ModuleError::InvalidModuleName => write!(f, "Invalid module name"),
            ModuleError::MissingAttribute(atom) => {
                write!(f, "Missing required attribute: {:?}", atom)
            }
            ModuleError::DuplicateDefinition(name) => write!(f, "Duplicate definition: {}", name),
        }
    }
}

impl std::error::Error for ModuleError {}

impl From<chimera_parser::ParseError> for ModuleError {
    fn from(err: chimera_parser::ParseError) -> Self {
        ModuleError::Parse(format!("{:?}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_builder_new() {
        let atoms = SharedAtomTable::new();
        let builder = ModuleBuilder::new(atoms);
        assert!(builder.module_name.is_none());
        assert!(builder.attributes.is_empty());
        assert!(builder.functions.is_empty());
    }

    #[test]
    fn test_module_builder_set_name() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        let name = ModuleName::new(vec![Atom::new(1)]);
        builder.set_module_name(name.clone());
        assert_eq!(builder.module_name, Some(name));
    }

    #[test]
    fn test_module_builder_attribute() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        let attr_name = Atom::new(1);
        let attr_value = AST::Integer(42);
        builder.add_attribute(attr_name, attr_value);
        assert!(builder.get_attribute(&Atom::new(1)).is_some());
    }

    #[test]
    fn test_module_builder_function() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        let func = FunctionDef {
            name: Atom::new(1),
            public: true,
            clauses: vec![AST::Integer(42)],
            meta: Meta::default(),
        };
        builder.add_function(func);
        assert_eq!(builder.functions.len(), 1);
    }

    #[test]
    fn test_module_builder_build_empty() {
        let atoms = SharedAtomTable::new();
        let builder = ModuleBuilder::new(atoms);
        let result = builder.build();
        assert!(matches!(result, AST::Tuple(vec) if vec.is_empty()));
    }

    #[test]
    fn test_module_builder_build_with_name() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        builder.set_module_name(ModuleName::new(vec![Atom::new(42)]));
        let result = builder.build();
        match result {
            AST::Defmodule { name, .. } => {
                if let AST::Atom(atom) = *name {
                    assert_eq!(atom.id(), 42);
                } else {
                    panic!("Expected atom for module name");
                }
            }
            _ => panic!("Expected Defmodule AST"),
        }
    }

    #[test]
    fn test_module_to_ast() {
        let module = Module {
            name: ModuleName::new(vec![Atom::new(1)]),
            attributes: HashMap::new(),
            functions: vec![],
            macros: vec![],
            nested_modules: vec![],
            body: vec![],
        };
        let ast = module.to_ast();
        match ast {
            AST::Defmodule { name, .. } => {
                if let AST::Atom(atom) = *name {
                    assert_eq!(atom.id(), 1);
                } else {
                    panic!("Expected atom");
                }
            }
            _ => panic!("Expected Defmodule"),
        }
    }

    #[test]
    fn test_module_builder_macro() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        let macro_def = MacroDef {
            name: Atom::new(1),
            public: true,
            clauses: vec![AST::Integer(42)],
            meta: Meta::default(),
        };
        builder.add_macro(macro_def);
        assert_eq!(builder.macros.len(), 1);
    }

    #[test]
    fn test_module_builder_nested_module() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        let nested = Module {
            name: ModuleName::new(vec![Atom::new(1), Atom::new(2)]),
            attributes: HashMap::new(),
            functions: vec![],
            macros: vec![],
            nested_modules: vec![],
            body: vec![],
        };
        builder.add_nested_module(nested);
        assert_eq!(builder.nested_modules.len(), 1);
    }

    #[test]
    fn test_module_builder_multiple_functions() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        builder.set_module_name(ModuleName::new(vec![Atom::new(1)]));

        let func1 = FunctionDef {
            name: Atom::new(10),
            public: true,
            clauses: vec![AST::Integer(1)],
            meta: Meta::default(),
        };
        let func2 = FunctionDef {
            name: Atom::new(11),
            public: true,
            clauses: vec![AST::Integer(2)],
            meta: Meta::default(),
        };
        builder.add_function(func1);
        builder.add_function(func2);

        let result = builder.build();
        match result {
            AST::Defmodule { body, .. } => {
                assert_eq!(body.len(), 2);
            }
            _ => panic!("Expected Defmodule"),
        }
    }

    #[test]
    fn test_module_builder_with_attributes() {
        let atoms = SharedAtomTable::new();
        let mut builder = ModuleBuilder::new(atoms);
        builder.set_module_name(ModuleName::new(vec![Atom::new(1)]));

        let vsn = Atom::new(2);
        builder.add_attribute(vsn, AST::String("1.0.0".to_string()));

        let result = builder.build();
        match result {
            AST::Defmodule { body, .. } => {
                assert_eq!(body.len(), 1);
                match &body[0] {
                    AST::Call { name, args, .. } => {
                        assert_eq!(args.len(), 1);
                    }
                    _ => panic!("Expected Call for attribute"),
                }
            }
            _ => panic!("Expected Defmodule"),
        }
    }
}
