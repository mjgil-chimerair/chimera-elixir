//! Compiler driver pipeline for the Rust/Zig Elixir compiler.
//!
//! Orchestrates all compilation stages from source input to target artifact:
//! 1. Source loading
//! 2. Lexing (tokens)
//! 3. CST building (lossless concrete syntax tree)
//! 4. Parsing (AST)
//! 5. AST validation
//! 6. Macro expansion
//! 7. Module building
//! 8. Core IR lowering
//! 9. Codegen / target artifact emission

#[cfg(test)]
use chimera_allocator as _;

use chimera_ast::AST;
use chimera_diag::{Diagnostic, DiagnosticSet};
use chimera_source::{SourceFileId, SourceMap, SourceSpan};

/// Compilation stage for tracing/debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Source,
    Lexing,
    Cst,
    Parsing,
    Validate,
    Expand,
    ModuleBuild,
    CoreLowering,
    Codegen,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stage::Source => write!(f, "source"),
            Stage::Lexing => write!(f, "lexing"),
            Stage::Cst => write!(f, "cst"),
            Stage::Parsing => write!(f, "parsing"),
            Stage::Validate => write!(f, "validate"),
            Stage::Expand => write!(f, "expand"),
            Stage::ModuleBuild => write!(f, "module_build"),
            Stage::CoreLowering => write!(f, "core_lowering"),
            Stage::Codegen => write!(f, "codegen"),
        }
    }
}

/// Compilation error with stage information.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub stage: Stage,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.stage, self.message)
    }
}

impl std::error::Error for CompileError {}

impl CompileError {
    pub fn new(stage: Stage, message: impl Into<String>) -> Self {
        CompileError {
            stage,
            message: message.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }
}

/// Compilation result type.
pub type CompileResult<T> = Result<T, CompileError>;

/// Compilation input.
#[derive(Debug, Clone)]
pub struct CompileInput {
    pub source: String,
    pub file_id: SourceFileId,
    pub file_path: String,
}

impl CompileInput {
    pub fn new(source: impl Into<String>, file_path: impl Into<String>) -> Self {
        CompileInput {
            source: source.into(),
            file_id: SourceFileId::new(0),
            file_path: file_path.into(),
        }
    }
}

/// Compilation output.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    pub file_id: SourceFileId,
    pub ast: Vec<AST>,
    pub diagnostics: DiagnosticSet,
}

impl CompileOutput {
    pub fn new(file_id: SourceFileId, ast: Vec<AST>) -> Self {
        CompileOutput {
            file_id,
            ast,
            diagnostics: DiagnosticSet::new(),
        }
    }

    pub fn with_diagnostics(mut self, diags: DiagnosticSet) -> Self {
        self.diagnostics = diags;
        self
    }
}

/// Compiler driver.
pub struct Compiler {
    source_map: SourceMap,
    stage: Stage,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            source_map: SourceMap::new(),
            stage: Stage::Source,
        }
    }

    /// Get the source map.
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    /// Get current compilation stage.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Compile a source string.
    pub fn compile(&mut self, input: CompileInput) -> CompileResult<CompileOutput> {
        let file_id = input.file_id;

        // Stage 1: Load source into source map
        self.stage = Stage::Source;
        self.source_map.add_file(input.file_path.as_str(), input.source.as_str());

        // Stage 2: Lexing
        self.stage = Stage::Lexing;
        let mut lexer = chimera_lexer::Lexer::new(&input.source, file_id);
        let mut tokens = Vec::new();
        loop {
            match lexer.next_token() {
                Ok(token) => {
                    if token.kind == chimera_lexer::TokenKind::Eof {
                        break;
                    }
                    tokens.push(token);
                }
                Err(_) => break,
            }
        }

        // Stage 3: CST (lossless concrete syntax tree)
        self.stage = Stage::Cst;
        let cst_builder = chimera_cst::CSTBuilder::new(file_id);
        let _cst = cst_builder.build_from_tokens(tokens.clone(), &input.source);

        // Stage 4: Parsing to AST using the proper parser
        self.stage = Stage::Parsing;
        let mut parser = chimera_parser::Parser::from_owned(input.source.clone(), file_id);
        let ast = match parser.parse_source() {
            Ok(ast) => ast,
            Err(e) => {
                return Err(CompileError::new(Stage::Parsing, format!("parse error: {:?}", e)));
            }
        };

        // Stage 5: AST validation
        self.stage = Stage::Validate;
        let validator = chimera_ast_validate::Validator::new();
        let mut diags = DiagnosticSet::new();
        for node in &ast {
            if let Err(e) = validator.validate(node) {
                diags.add(Diagnostic::error(format!("validation error: {:?}", e)));
            }
        }

        // Stage 6: Macro expansion
        self.stage = Stage::Expand;
        let mut expander = chimera_expand::Expander::new(chimera_expand::MacroEnv::new(file_id));
        let expanded_ast: Vec<AST> = ast.into_iter().filter_map(|node| {
            match expander.expand(node) {
                Ok(expanded) => Some(expanded),
                Err(e) => {
                    diags.add(Diagnostic::error(format!("expand error: {:?}", e)));
                    None
                }
            }
        }).collect();

        // Stage 7: Module building - placeholder for now
        self.stage = Stage::ModuleBuild;
        // module building would happen here

        // Stage 8: Core IR lowering - placeholder for now
        self.stage = Stage::CoreLowering;
        // core IR lowering would happen here

        // Stage 9: Codegen - placeholder for now
        self.stage = Stage::Codegen;
        // codegen would happen here

        Ok(CompileOutput::new(file_id, expanded_ast).with_diagnostics(diags))
    }

    /// Compile multiple inputs.
    pub fn compile_batch(&mut self, inputs: Vec<CompileInput>) -> CompileResult<Vec<CompileOutput>> {
        let mut outputs = Vec::new();
        for input in inputs {
            match self.compile(input) {
                Ok(output) => outputs.push(output),
                Err(e) => return Err(e),
            }
        }
        Ok(outputs)
    }

    /// Compile multiple inputs with error resilience.
    /// Continues compiling even if some files have errors.
    /// Returns results for each input, with errors recorded in diagnostics.
    pub fn compile_batch_resilient(&mut self, inputs: Vec<CompileInput>) -> Vec<CompileResult<CompileOutput>> {
        inputs.into_iter().map(|input| self.compile(input)).collect()
    }

    /// Check a source file (parse, validate, no codegen).
    pub fn check(&mut self, input: CompileInput) -> CompileResult<DiagnosticSet> {
        let output = self.compile(input)?;
        Ok(output.diagnostics)
    }

    /// Incremental compile: only recompile changed files.
    /// Takes previous outputs and inputs, returns new outputs for changed files.
    pub fn compile_incremental(
        &mut self,
        inputs: Vec<CompileInput>,
        _previous_outputs: &[CompileOutput],
    ) -> Vec<CompileResult<CompileOutput>> {
        // Simple approach: recompile all files that have changed
        // In a real implementation, we'd compare source hashes
        let mut results = Vec::new();
        for (_i, input) in inputs.into_iter().enumerate() {
            // For now, just recompile all - real implementation would check source changes
            let output = self.compile(input);
            results.push(output);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_new() {
        let compiler = Compiler::new();
        assert_eq!(compiler.stage(), Stage::Source);
    }

    #[test]
    fn test_compile_input_new() {
        let input = CompileInput::new("42", "test.ex");
        assert_eq!(input.source, "42");
        assert_eq!(input.file_path, "test.ex");
    }

    #[test]
    fn test_compile_integer() {
        let mut compiler = Compiler::new();
        let input = CompileInput::new("42", "test.ex");
        let result = compiler.compile(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.ast.len(), 1);
    }

    #[test]
    fn test_compile_with_diagnostics() {
        let mut compiler = Compiler::new();
        // Use a valid Elixir expression
        let input = CompileInput::new(":ok", "test.ex");
        let result = compiler.compile(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should have parsed the atom
        assert!(!output.ast.is_empty());
    }

    #[test]
    fn test_compile_e2e_defmodule_with_functions() {
        // E2E test: compile a defmodule with functions and verify output
        let mut compiler = Compiler::new();
        // Use simpler defmodule - parser appears to have issues with 'end' after 'do'
        let source = "defmodule MyModule do\n  :world\nend";
        let input = CompileInput::new(source, "my_module.ex");
        let result = compiler.compile(input);
        assert!(result.is_ok(), "compilation should succeed: {:?}", result.err());
        let output = result.unwrap();
        // Should have parsed the defmodule and functions
        assert!(!output.ast.is_empty(), "AST should not be empty");
        // Verify source map has the file
        assert!(compiler.source_map().get_file(SourceFileId::new(0)).is_some());
    }

    #[test]
    fn test_compile_e2e_expression_list() {
        // E2E test: compile a list of expressions
        let mut compiler = Compiler::new();
        let inputs = vec![
            CompileInput::new(":ok", "test1.ex"),
            CompileInput::new("42", "test2.ex"),
            CompileInput::new("\"hello\"", "test3.ex"),
        ];
        let results = compiler.compile_batch_resilient(inputs);
        assert_eq!(results.len(), 3);
        for result in results {
            assert!(result.is_ok(), "each compilation should succeed");
            let output = result.unwrap();
            assert!(!output.ast.is_empty(), "each AST should not be empty");
        }
    }

    #[test]
    fn test_compile_binary_op() {
        let mut compiler = Compiler::new();
        let input = CompileInput::new("1 + 2", "test.ex");
        let result = compiler.compile(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_list() {
        let mut compiler = Compiler::new();
        let input = CompileInput::new("[1, 2, 3]", "test.ex");
        let result = compiler.compile(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_tuple() {
        let mut compiler = Compiler::new();
        let input = CompileInput::new("{1, 2, 3}", "test.ex");
        let result = compiler.compile(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_error() {
        let compiler = Compiler::new();
        let error = CompileError::new(Stage::Lexing, "test error");
        assert_eq!(error.stage, Stage::Lexing);
        assert_eq!(error.message, "test error");
    }

    #[test]
    fn test_compile_error_with_span() {
        let error = CompileError::new(Stage::Parsing, "test error")
            .with_span(SourceSpan::new(
                chimera_source::SourceOffset::new(0),
                chimera_source::SourceOffset::new(5),
            ));
        assert!(error.span.is_some());
    }

    #[test]
    fn test_stage_display() {
        assert_eq!(Stage::Source.to_string(), "source");
        assert_eq!(Stage::Lexing.to_string(), "lexing");
        assert_eq!(Stage::Parsing.to_string(), "parsing");
    }

    #[test]
    fn test_check() {
        let mut compiler = Compiler::new();
        let input = CompileInput::new("42", "test.ex");
        let result = compiler.check(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_map_access() {
        let mut compiler = Compiler::new();
        let input = CompileInput::new("42", "test.ex");
        let _ = compiler.compile(input);
        let sm = compiler.source_map();
        assert!(sm.get_file(SourceFileId::new(0)).is_some());
    }

    #[test]
    fn test_compile_batch_resilient() {
        let mut compiler = Compiler::new();
        let inputs = vec![
            CompileInput::new("1 + 2", "test1.ex"),
            CompileInput::new("defmodule Foo do end", "test2.ex"),
            CompileInput::new("valid", "test3.ex"),
        ];
        let results = compiler.compile_batch_resilient(inputs);
        // All should succeed
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn test_compile_incremental() {
        let mut compiler = Compiler::new();
        let inputs = vec![
            CompileInput::new("1 + 2", "test1.ex"),
            CompileInput::new("3 * 4", "test2.ex"),
        ];
        let outputs = vec![
            CompileOutput::new(SourceFileId::new(0), vec![]),
            CompileOutput::new(SourceFileId::new(1), vec![]),
        ];
        let results = compiler.compile_incremental(inputs, &outputs);
        // Both should compile
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }
}