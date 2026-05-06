use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chimera_codegen::{Codegen, CodegenConfig};
use chimera_core::{CoreExpr, CoreModule, CoreFunction};
use chimera_term::{AtomTable, ModuleName};
use chimera_ast::Meta;
use std::collections::{HashMap, HashSet};

// Create a more complex expression for benchmarking
fn create_complex_expr() -> CoreExpr {
    CoreExpr::Call {
        module: None,
        name: chimera_term::Atom::new(1),
        args: vec![
            CoreExpr::Integer(10),
            CoreExpr::Integer(20),
            CoreExpr::Tuple(vec![
                CoreExpr::Float(3.14),
                CoreExpr::Atom(chimera_term::Atom::new(2)),
            ]),
        ],
    }
}

// Create a simple Core module for benchmarking
fn create_test_module() -> CoreModule {
    CoreModule {
        name: ModuleName(vec![chimera_term::Atom::new(99)]), // benchmark module
        exports: HashSet::new(),
        attributes: HashMap::new(),
        functions: vec![CoreFunction {
            name: chimera_term::Atom::new(100), // Some atom not likely to clash
            arity: 2,
            params: vec![], // Simple case with no parameters
            guards: vec![], // No guards
            meta: Meta::default(), // Default meta
            body: create_complex_expr(), // Fixed: removed Box wrapper
            exported: true,
        }],
        compile_info: chimera_core::CoreCompileInfo {
            file: Some("benchmark".to_string()), // Fixed: made it Option<String>
            line: 1,
            vsn: None,
        },
    }
}

fn bench_opcode_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("opcode_generation");
    group.sample_size(100);
    
    // Setup common objects
    let config = CodegenConfig::default();
    let module = create_test_module();
    
    // Benchmark code generation for a module
    group.bench_function("module_codegen", |b| {
        b.iter(|| {
            // Create fresh atom table for each iteration (no cloning needed)
            let mut atoms = AtomTable::new();
            // Pre-intern some atoms to avoid measuring intern time in the benchmark
            atoms.intern("benchmark");
            atoms.intern("test");
            atoms.intern("hello");
            atoms.intern("world");
            
            let mut codegen = Codegen::new(config.clone(), atoms);
            let output = codegen.generate(&module).unwrap();
            black_box(output.code.len());
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_opcode_generation);
criterion_main!(benches);