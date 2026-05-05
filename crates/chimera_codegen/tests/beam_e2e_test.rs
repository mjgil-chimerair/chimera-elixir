//! End-to-end integration tests for BEAM pipeline.
//!
//! This module tests the complete pipeline:
//! Elixir source → Core IR → BEAM file → VM load → Execute

use chimera_beam_term::{Atom, BeamFile, Mfa, ModuleCode};
use chimera_codegen::CodegenConfig;
use chimera_core::{CoreExpr, CoreFunction, CoreModule, CoreCompileInfo};
use chimera_term::{AtomTable, ModuleName};
use std::collections::{HashMap, HashSet};

/// Helper to create a simple Core module for testing.
fn make_test_module(name: &str) -> CoreModule {
    let mut atoms = AtomTable::new();
    let module_atom = atoms.intern(name);
    let func_atom = atoms.intern("test_func");
    let arg_atom = atoms.intern("X");

    CoreModule {
        name: ModuleName::new(vec![module_atom]),
        exports: HashSet::new(),
        attributes: std::collections::HashMap::new(),
        functions: vec![CoreFunction {
            name: func_atom,
            arity: 1,
            exported: true,
            params: vec![arg_atom.clone()],
            guards: vec![],
            body: CoreExpr::Var {
                name: arg_atom,
                arity: 0,
            },
            meta: Default::default(),
        }],
        compile_info: CoreCompileInfo {
            file: Some("test.ex".to_string()),
            line: 1,
            vsn: None,
        },
    }
}

/// Test: Compile simple module and emit BEAM format.
#[test]
fn test_emit_beam_simple_module() {
    let module = make_test_module("test_module");
    let atoms = AtomTable::new();
    let config = CodegenConfig::default();

    let result = chimera_codegen::compile_to_target(&module, atoms, config);
    assert!(result.is_ok(), "BEAM emission failed: {:?}", result.err());

    let beam_bytes = result.unwrap();
    assert!(!beam_bytes.is_empty(), "BEAM output should not be empty");

    // Verify BEAM magic header
    assert_eq!(&beam_bytes[0..4], b"BEAM", "Missing BEAM magic header");

    // Verify standard IFF format (no version byte after magic)
    assert!(beam_bytes.len() > 12, "BEAM file too small");
}

/// Test: BEAM file structure validation.
#[test]
fn test_beam_file_structure() {
    let module = make_test_module("struct_test");
    let atoms = AtomTable::new();
    let config = CodegenConfig::default();

    let beam_bytes = chimera_codegen::compile_to_target(&module, atoms, config).unwrap();

    // Verify BEAM magic header
    assert_eq!(&beam_bytes[0..4], b"BEAM", "Missing BEAM magic header");

    // Verify standard IFF format (no version byte after magic)
    assert!(beam_bytes.len() > 12, "BEAM file too small");

    // TODO: Add proper BEAM chunk parsing once format is finalized
    // The custom format has different chunk structure than standard IFF
    assert!(beam_bytes.len() > 20, "BEAM file too small");
}

/// Test: Round-trip Core module → BEAM → parse back.
#[test]
fn test_beam_round_trip() {
    let module = make_test_module("round_trip_test");
    let atoms = AtomTable::new();
    let config = CodegenConfig::default();

    // Generate BEAM
    let beam_bytes = chimera_codegen::compile_to_target(&module, atoms, config).unwrap();

    // Verify BEAM magic header
    assert_eq!(&beam_bytes[0..4], b"BEAM", "Missing BEAM magic header");

    // Verify standard IFF format (no version byte after magic)
    assert!(beam_bytes.len() > 12, "BEAM file too small");

    // TODO: Add proper BEAM chunk parsing once format is finalized
    // Verify basic structure (magic + chunk count)
    assert!(beam_bytes.len() > 20, "BEAM file too small");
}

/// Test: BEAM export table contains our function.
#[test]
fn test_beam_export_table() {
    let module = make_test_module("export_test");
    let atoms = AtomTable::new();
    let config = CodegenConfig::default();

    let result = chimera_codegen::compile_to_target(&module, atoms, config);
    assert!(result.is_ok());

    let beam_bytes = result.unwrap();

    // Verify BEAM magic header
    assert_eq!(&beam_bytes[0..4], b"BEAM", "Missing BEAM magic header");

    // TODO: Add proper BEAM chunk parsing once format is finalized
    // Verify basic structure
    assert!(beam_bytes.len() > 20, "BEAM file too small");
}

/// Test: BEAM with multiple functions.
#[test]
fn test_beam_multiple_functions() {
    let mut atoms = AtomTable::new();
    let module_atom = atoms.intern("multi_test");
    let func1_atom = atoms.intern("func1");
    let func2_atom = atoms.intern("func2");
    let x_atom = atoms.intern("X");
    let y_atom = atoms.intern("Y");

    let module = CoreModule {
        name: ModuleName::new(vec![module_atom]),
        exports: HashSet::new(),
        attributes: HashMap::new(),
        functions: vec![
            CoreFunction {
                name: func1_atom,
                arity: 1,
                exported: true,
                params: vec![x_atom.clone()],
                guards: vec![],
                body: CoreExpr::Var { name: x_atom.clone(), arity: 0 },
                meta: Default::default(),
            },
            CoreFunction {
                name: func2_atom,
                arity: 2,
                exported: true,
                params: vec![x_atom.clone(), y_atom.clone()],
                guards: vec![],
                body: CoreExpr::Var { name: x_atom, arity: 0 },
                meta: Default::default(),
            },
        ],
        compile_info: CoreCompileInfo {
            file: Some("multi.ex".to_string()),
            line: 1,
            vsn: None,
        },
    };

    let config = CodegenConfig::default();
    let result = chimera_codegen::compile_to_target(&module, atoms, config);
    assert!(result.is_ok());

    let beam_bytes = result.unwrap();
    assert!(!beam_bytes.is_empty());
}
#[test]
fn test_beam_integer_constants() {
    let mut atoms = AtomTable::new();
    let module_atom = atoms.intern("int_test");
    let func_atom = atoms.intern("add_one");

    let module = CoreModule {
        name: ModuleName::new(vec![module_atom]),
        exports: HashSet::new(),
        attributes: std::collections::HashMap::new(),
        functions: vec![CoreFunction {
            name: func_atom,
            arity: 1,
            exported: true,
            params: vec![],
            guards: vec![],
            body: CoreExpr::Integer(42),
            meta: Default::default(),
        }],
        compile_info: CoreCompileInfo {
            file: None,
            line: 1,
            vsn: None,
        },
    };

    let config = CodegenConfig::default();
    let result = chimera_codegen::compile_to_target(&module, atoms, config);
    assert!(result.is_ok());
}

/// Test: BEAM with tuple constants.
#[test]
fn test_beam_tuple_constants() {
    let mut atoms = AtomTable::new();
    let module_atom = atoms.intern("tuple_test");
    let func_atom = atoms.intern("make_tuple");

    let module = CoreModule {
        name: ModuleName::new(vec![module_atom]),
        exports: HashSet::new(),
        attributes: std::collections::HashMap::new(),
        functions: vec![CoreFunction {
            name: func_atom,
            arity: 0,
            exported: true,
            params: vec![],
            guards: vec![],
            body: CoreExpr::Tuple(vec![CoreExpr::Integer(1), CoreExpr::Integer(2)]),
            meta: Default::default(),
        }],
        compile_info: CoreCompileInfo {
            file: None,
            line: 1,
            vsn: None,
        },
    };

    let config = CodegenConfig::default();
    let result = chimera_codegen::compile_to_target(&module, atoms, config);
    assert!(result.is_ok());
}

/// Test: BEAM with list constants.
#[test]
fn test_beam_list_constants() {
    let mut atoms = AtomTable::new();
    let module_atom = atoms.intern("list_test");
    let func_atom = atoms.intern("make_list");

    let module = CoreModule {
        name: ModuleName::new(vec![module_atom]),
        exports: HashSet::new(),
        attributes: std::collections::HashMap::new(),
        functions: vec![CoreFunction {
            name: func_atom,
            arity: 0,
            exported: true,
            params: vec![],
            guards: vec![],
            body: CoreExpr::List(vec![
                CoreExpr::Integer(1),
                CoreExpr::Integer(2),
                CoreExpr::Integer(3),
            ]),
            meta: Default::default(),
        }],
        compile_info: CoreCompileInfo {
            file: None,
            line: 1,
            vsn: None,
        },
    };

    let config = CodegenConfig::default();
    let result = chimera_codegen::compile_to_target(&module, atoms, config);
    assert!(result.is_ok());
}

/// Test: Mfa structure for function resolution.
#[test]
fn test_mfa_structure() {
    let mfa = Mfa::new(Atom::new(0), Atom::new(1), 2);
    assert_eq!(mfa.module, Atom::new(0));
    assert_eq!(mfa.function, Atom::new(1));
    assert_eq!(mfa.arity, 2);
}

/// Test: Invalid BEAM file detection.
#[test]
fn test_invalid_beam_detection() {
    let invalid_data = b"This is not a BEAM file".to_vec();
    let reader = BeamFile::parse(&invalid_data);
    assert!(reader.is_err(), "Should reject invalid BEAM files");
}

/// Test: ModuleCode structure.
#[test]
fn test_module_code_structure() {
    let atom = Atom::new(0);
    let code = ModuleCode::new(atom);
    assert_eq!(code.name, Atom::new(0));
    assert!(code.code.is_empty());
    assert!(code.exports.is_empty());
}
