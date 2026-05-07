//! Target runtime adapter for the Rust/Zig Elixir compiler.
//!
//! This module provides the interface to external BEAM-like targets
//! for macro execution and module loading.

#[cfg(test)]
use chimera_allocator as _;

use chimera_ast::AST;
use chimera_module::{ModuleBuilder, ModuleError};
use chimera_source::SourceFileId;
use chimera_term::{Atom, SharedAtomTable, ModuleName, Term};
use std::collections::HashMap;

/// Target runtime error.
#[derive(Debug, Clone)]
pub enum TargetError {
    /// Module not found
    ModuleNotFound(ModuleName),
    /// Function not found
    FunctionNotFound(Atom, u8),
    /// Macro execution failed
    MacroFailed(String),
    /// Invalid artifact
    InvalidArtifact(String),
    /// Target unavailable
    Unavailable(String),
}

impl std::fmt::Display for TargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetError::ModuleNotFound(name) => write!(f, "Module not found: {:?}", name),
            TargetError::FunctionNotFound(name, arity) => write!(f, "Function not found: {:?}/{}", name, arity),
            TargetError::MacroFailed(msg) => write!(f, "Macro execution failed: {}", msg),
            TargetError::InvalidArtifact(msg) => write!(f, "Invalid artifact: {}", msg),
            TargetError::Unavailable(msg) => write!(f, "Target unavailable: {}", msg),
        }
    }
}

impl std::error::Error for TargetError {}

impl From<ModuleError> for TargetError {
    fn from(err: ModuleError) -> Self {
        TargetError::InvalidArtifact(format!("{:?}", err))
    }
}

/// Target runtime trait - the interface between the Elixir compiler
/// and the BEAM-like target.
pub trait TargetRuntime: Send + Sync {
    /// Ensure a module is loaded in the target.
    fn ensure_loaded(&mut self, module: &ModuleName) -> Result<(), TargetError>;

    /// Call a macro in a module with the given arguments and caller environment.
    fn call_macro(
        &mut self,
        module: &ModuleName,
        function: &Atom,
        args: Vec<Term>,
        caller_env: MacroEnvTerm,
    ) -> Result<Term, TargetError>;

    /// Emit a compiled module artifact to the target.
    fn emit_module(&mut self, artifact: &CompiledModuleArtifact) -> Result<(), TargetError>;

    /// Get the export table for a module.
    fn get_exports(&self, module: &ModuleName) -> Result<Vec<(Atom, u8)>, TargetError>;

    /// Evaluate an expression in the context of a module.
    fn evaluate_expression(
        &mut self,
        module: &ModuleName,
        function: &Atom,
        args: Vec<Term>,
    ) -> Result<Term, TargetError>;
}

/// Macro environment as a term for passing to macro callbacks.
#[derive(Debug, Clone)]
pub struct MacroEnvTerm {
    /// Current module
    pub module: Option<ModuleName>,
    /// Current function
    pub function: Option<(Atom, u8)>,
    /// Current file
    pub file: SourceFileId,
    /// Current line
    pub line: u32,
    /// Aliases
    pub aliases: HashMap<Atom, ModuleName>,
    /// Imports
    pub imports: HashMap<Atom, (ModuleName, Atom)>,
    /// Requires
    pub requires: Vec<ModuleName>,
    /// Variables in scope
    pub vars: Vec<(Atom, Option<String>)>,
}

impl MacroEnvTerm {
    /// Create a new macro environment term.
    pub fn new() -> Self {
        MacroEnvTerm {
            module: None,
            function: None,
            file: SourceFileId::new(0),
            line: 0,
            aliases: HashMap::new(),
            imports: HashMap::new(),
            requires: Vec::new(),
            vars: Vec::new(),
        }
    }

    /// Convert to a Term for passing to macros.
    pub fn to_term(&self, atoms: &mut SharedAtomTable) -> Term {
        let mut pairs = Vec::new();

        // Module
        if let Some(ref m) = self.module {
            let key = atoms.intern("module");
            let val = Term::Atom(m.segments().first().cloned().unwrap_or(Atom::new(0)));
            pairs.push((Term::Atom(key), val));
        }

        // Line
        let key = atoms.intern("line");
        let val = Term::SmallInt(self.line as i64);
        pairs.push((Term::Atom(key), val));

        Term::Map(pairs)
    }
}

impl Default for MacroEnvTerm {
    fn default() -> Self {
        Self::new()
    }
}

/// A compiled module artifact ready for emission to the target.
#[derive(Debug, Clone)]
pub struct CompiledModuleArtifact {
    /// Module name
    pub module: ModuleName,
    /// Compiled AST
    pub ast: AST,
    /// Exports (function/arity pairs)
    pub exports: Vec<(Atom, u8)>,
    /// Attributes
    pub attributes: HashMap<Atom, AST>,
    /// Compile info
    pub compile_info: CompileInfo,
}

/// Compilation information attached to the module.
#[derive(Debug, Clone)]
pub struct CompileInfo {
    /// Source file
    pub file: Option<String>,
    /// Line where module was defined
    pub line: u32,
    /// Module vsn
    pub vsn: Option<Atom>,
}

/// Target adapter with internal implementation.
pub struct TargetAdapter {
    /// Loaded modules
    loaded_modules: HashMap<ModuleName, CompiledModuleArtifact>,
    /// Macro registry
    macro_registry: HashMap<(ModuleName, Atom), AST>,
    /// Atom table
    atoms: SharedAtomTable,
}

impl TargetAdapter {
    /// Create a new target adapter.
    pub fn new(atoms: SharedAtomTable) -> Self {
        TargetAdapter {
            loaded_modules: HashMap::new(),
            macro_registry: HashMap::new(),
            atoms,
        }
    }

    /// Register a macro in the registry.
    pub fn register_macro(&mut self, module: ModuleName, name: Atom, ast: AST) {
        self.macro_registry.insert((module, name), ast);
    }

    /// Check if a module is loaded.
    pub fn is_loaded(&self, module: &ModuleName) -> bool {
        self.loaded_modules.contains_key(module)
    }
}

impl TargetRuntime for TargetAdapter {
    fn ensure_loaded(&mut self, module: &ModuleName) -> Result<(), TargetError> {
        if !self.loaded_modules.contains_key(module) {
            Err(TargetError::ModuleNotFound(module.clone()))
        } else {
            Ok(())
        }
    }

    fn call_macro(
        &mut self,
        module: &ModuleName,
        function: &Atom,
        _args: Vec<Term>,
        _caller_env: MacroEnvTerm,
    ) -> Result<Term, TargetError> {
        let key = (module.clone(), function.clone());
        if let Some(_macro_ast) = self.macro_registry.get(&key) {
            // Execute the macro with the given args
            // For now, return nil
            Ok(Term::Nil)
        } else {
            Err(TargetError::MacroFailed(format!(
                "Macro not found in registry: {:?}/{:?}",
                module, function
            )))
        }
    }

    fn emit_module(&mut self, artifact: &CompiledModuleArtifact) -> Result<(), TargetError> {
        self.loaded_modules.insert(artifact.module.clone(), artifact.clone());
        Ok(())
    }

    fn get_exports(&self, module: &ModuleName) -> Result<Vec<(Atom, u8)>, TargetError> {
        if let Some(artifact) = self.loaded_modules.get(module) {
            Ok(artifact.exports.clone())
        } else {
            Err(TargetError::ModuleNotFound(module.clone()))
        }
    }

    fn evaluate_expression(
        &mut self,
        module: &ModuleName,
        function: &Atom,
        _args: Vec<Term>,
    ) -> Result<Term, TargetError> {
        // Check if module is loaded
        if !self.loaded_modules.contains_key(module) {
            return Err(TargetError::ModuleNotFound(module.clone()));
        }

        // Check if function is exported
        if let Some(artifact) = self.loaded_modules.get(module) {
            if !artifact.exports.iter().any(|(name, arity)| {
                name == function && *arity == 0
            }) {
                return Err(TargetError::FunctionNotFound(function.clone(), 0));
            }
        }

        // In a real implementation, this would invoke the BEAM runtime
        // to execute the function with the given arguments
        // For now, return nil as a placeholder
        Ok(Term::Nil)
    }
}

/// Compile a module to an artifact ready for emission.
pub fn compile_module(
    source: &'static str,
    file_id: SourceFileId,
    atoms: SharedAtomTable,
) -> Result<CompiledModuleArtifact, TargetError> {
    let mut builder = ModuleBuilder::new(atoms);
    let ast = builder.compile_source(source, file_id)?;

    // Extract module name and body
    let (module_name, exports, ast) = match ast {
        AST::Defmodule { name, body, meta } => {
            // Extract module name - clone the atom since we still need name for the output
            let name_atom = if let AST::Atom(ref atom) = *name {
                atom.clone()
            } else {
                return Err(TargetError::InvalidArtifact("Invalid module name".to_string()));
            };
            let module_name = ModuleName::new(vec![name_atom]);

            // Extract exports from def/defp forms
            let mut exports = Vec::new();
            for form in &body {
                match form {
                    AST::Def { name: ref n, clauses, .. } => {
                        exports.push((n.clone(), clauses.len() as u8));
                    }
                    AST::Defp { name: ref n, clauses, .. } => {
                        exports.push((n.clone(), clauses.len() as u8));
                    }
                    _ => {}
                }
            }
            (module_name, exports, AST::Defmodule { name, body, meta })
        }
        _ => return Err(TargetError::InvalidArtifact("Expected defmodule".to_string())),
    };

    Ok(CompiledModuleArtifact {
        module: module_name,
        ast,
        exports,
        attributes: builder.attributes,
        compile_info: CompileInfo {
            file: None,
            line: 1,
            vsn: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_ast::Meta;

    #[test]
    fn test_target_adapter_new() {
        let atoms = SharedAtomTable::new();
        let adapter = TargetAdapter::new(atoms);
        assert!(adapter.loaded_modules.is_empty());
    }

    #[test]
    fn test_target_adapter_register_macro() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);
        let module = ModuleName::new(vec![Atom::new(1)]);
        let name = Atom::new(2);
        let ast = AST::Integer(42);
        adapter.register_macro(module.clone(), name.clone(), ast);
        assert!(adapter.macro_registry.contains_key(&(module, name.clone())));
    }

    #[test]
    fn test_target_adapter_is_loaded() {
        let atoms = SharedAtomTable::new();
        let adapter = TargetAdapter::new(atoms);
        let module = ModuleName::new(vec![Atom::new(1)]);
        assert!(!adapter.is_loaded(&module));
    }

    #[test]
    fn test_macro_env_term_new() {
        let env = MacroEnvTerm::new();
        assert!(env.module.is_none());
        assert!(env.aliases.is_empty());
    }

    #[test]
    fn test_macro_env_term_default() {
        let env = MacroEnvTerm::default();
        assert!(env.module.is_none());
    }

    #[test]
    fn test_target_error_display() {
        let err = TargetError::ModuleNotFound(ModuleName::new(vec![Atom::new(1)]));
        assert!(!format!("{}", err).is_empty());
    }

    #[test]
    fn test_target_error_unavailable() {
        let err = TargetError::Unavailable("Target not running".to_string());
        assert!(format!("{}", err).contains("not running"));
    }

    #[test]
    fn test_emit_and_ensure_loaded() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);

        let module_name = ModuleName::new(vec![Atom::new(1)]);
        let artifact = CompiledModuleArtifact {
            module: module_name.clone(),
            ast: AST::Defmodule {
                name: Box::new(AST::Atom(Atom::new(1))),
                body: vec![],
                meta: Meta::default(),
            },
            exports: vec![],
            attributes: HashMap::new(),
            compile_info: CompileInfo {
                file: None,
                line: 1,
                vsn: None,
            },
        };

        // Emit the module
        let result = adapter.emit_module(&artifact);
        assert!(result.is_ok());

        // Now it should be loaded
        assert!(adapter.is_loaded(&module_name));

        // ensure_loaded should succeed
        let result = adapter.ensure_loaded(&module_name);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_loaded_not_found() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);
        let module_name = ModuleName::new(vec![Atom::new(99)]);

        let result = adapter.ensure_loaded(&module_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_exports_after_emit() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);

        let module_name = ModuleName::new(vec![Atom::new(1)]);
        let artifact = CompiledModuleArtifact {
            module: module_name.clone(),
            ast: AST::Defmodule {
                name: Box::new(AST::Atom(Atom::new(1))),
                body: vec![],
                meta: Meta::default(),
            },
            exports: vec![(Atom::new(10), 2), (Atom::new(11), 1)],
            attributes: HashMap::new(),
            compile_info: CompileInfo {
                file: None,
                line: 1,
                vsn: None,
            },
        };

        adapter.emit_module(&artifact).unwrap();

        let exports = adapter.get_exports(&module_name).unwrap();
        assert_eq!(exports.len(), 2);
    }

    #[test]
    fn test_get_exports_not_found() {
        let atoms = SharedAtomTable::new();
        let adapter = TargetAdapter::new(atoms);
        let module_name = ModuleName::new(vec![Atom::new(99)]);

        let result = adapter.get_exports(&module_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_call_macro_not_found() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);
        let module_name = ModuleName::new(vec![Atom::new(1)]);
        let func_name = Atom::new(2);

        let result = adapter.call_macro(
            &module_name,
            &func_name,
            vec![],
            MacroEnvTerm::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_call_macro_found() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);
        let module_name = ModuleName::new(vec![Atom::new(1)]);
        let func_name = Atom::new(2);

        // Register a macro
        adapter.register_macro(module_name.clone(), func_name.clone(), AST::Integer(42));

        let result = adapter.call_macro(
            &module_name,
            &func_name,
            vec![Term::Atom(Atom::new(1))],
            MacroEnvTerm::new(),
        );
        assert!(result.is_ok());
        // Currently returns Term::Nil as placeholder
        assert_eq!(result.unwrap(), Term::Nil);
    }

    #[test]
    fn test_evaluate_expression_not_loaded() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);
        let module_name = ModuleName::new(vec![Atom::new(1)]);
        let func_name = Atom::new(2);

        let result = adapter.evaluate_expression(&module_name, &func_name, vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_expression_function_not_exported() {
        let atoms = SharedAtomTable::new();
        let mut adapter = TargetAdapter::new(atoms);

        let module_name = ModuleName::new(vec![Atom::new(1)]);
        let artifact = CompiledModuleArtifact {
            module: module_name.clone(),
            ast: AST::Defmodule {
                name: Box::new(AST::Atom(Atom::new(1))),
                body: vec![],
                meta: Meta::default(),
            },
            exports: vec![(Atom::new(10), 2)], // Only func/2 is exported
            attributes: HashMap::new(),
            compile_info: CompileInfo {
                file: None,
                line: 1,
                vsn: None,
            },
        };

        adapter.emit_module(&artifact).unwrap();

        // Try to call func/1 which is not exported
        let result = adapter.evaluate_expression(
            &module_name,
            &Atom::new(11), // different function
            vec![],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_module_invalid() {
        let atoms = SharedAtomTable::new();
        let result = compile_module("not a defmodule", SourceFileId::new(0), atoms);
        assert!(result.is_err());
    }

    #[test]
    fn test_macro_env_term_to_term() {
        let mut atoms = SharedAtomTable::new();
        let mut env = MacroEnvTerm::new();
        env.line = 42;
        env.module = Some(ModuleName::new(vec![Atom::new(1)]));

        let term = env.to_term(&mut atoms);
        // Should return a Map term
        assert!(matches!(term, Term::Map(_)));
    }

    #[test]
    fn test_macro_env_term_vars() {
        let mut env = MacroEnvTerm::new();
        env.vars.push((Atom::new(1), Some("x".to_string())));
        env.vars.push((Atom::new(2), None));

        assert_eq!(env.vars.len(), 2);
    }

    #[test]
    fn test_macro_env_term_imports() {
        let mut env = MacroEnvTerm::new();
        env.imports.insert(
            Atom::new(1),
            (ModuleName::new(vec![Atom::new(10)]), Atom::new(11)),
        );

        assert_eq!(env.imports.len(), 1);
    }

    #[test]
    fn test_macro_env_term_requires() {
        let mut env = MacroEnvTerm::new();
        env.requires.push(ModuleName::new(vec![Atom::new(1)]));
        env.requires.push(ModuleName::new(vec![Atom::new(2), Atom::new(3)]));

        assert_eq!(env.requires.len(), 2);
    }
}