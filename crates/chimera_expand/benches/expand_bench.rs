use chimera_ast::{to_term, AST};
use chimera_expand::{ExpandOptions, MacroEnv};
use chimera_source::SourceFileId;
use chimera_term::{AtomTable, ModuleName};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Create a complex AST for expansion benchmarking
fn create_complex_ast() -> AST {
    // Create a moderately complex AST with macros and constructs that would trigger expansion
    AST::Block {
        exprs: vec![
            AST::Integer(42),
            AST::String("hello world".to_string()),
            AST::List(vec![
                AST::Atom(chimera_term::Atom::new(1)), // true
                AST::Tuple(vec![
                    AST::Float(3.14),
                    AST::Atom(chimera_term::Atom::new(2)), // false
                ]),
            ]),
            AST::Call {
                name: chimera_term::Atom::new(3), // Some function
                meta: chimera_ast::Meta::default(),
                args: vec![
                    AST::Nil,
                    AST::Var {
                        name: chimera_term::Atom::new(4), // Some variable
                        meta: chimera_ast::Meta::default(),
                    },
                ],
            },
            // This would be a macro call if we had macros defined
            AST::Call {
                name: chimera_term::Atom::new(5), // Another function
                meta: chimera_ast::Meta::default(),
                args: vec![AST::Integer(100), AST::Integer(200)],
            },
        ],
        meta: chimera_ast::Meta::default(),
    }
}

fn bench_expand(c: &mut Criterion) {
    let mut group = c.benchmark_group("expand");
    group.sample_size(100);

    group.bench_function("expand_ast", |b| {
        let ast = create_complex_ast();
        b.iter(|| {
            let mut atoms = AtomTable::new();
            let mut env = MacroEnv::new(
                Some(ModuleName(vec![chimera_term::Atom::new(99)])), // benchmark module
                None,                                                // no current function
                SourceFileId::new(0),
                1, // line number
                chimera_ast::ExprContext::Default,
                HashMap::new(),                  // aliases
                HashMap::new(),                  // imports
                HashSet::new(),                  // requires
                chimera_term::VarContext::new(), // vars
                ExpandOptions::default(),
            );
            let result = env.expand(ast.clone()).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_expand);
criterion_main!(benches);
