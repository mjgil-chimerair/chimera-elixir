use chimera_ast::{to_term, AST};
use chimera_ast_validate::{ValidationError, Validator};
use chimera_term::AtomTable;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Create a complex AST for validation benchmarking
fn create_complex_ast() -> AST {
    // Create a moderately complex AST that exercises various validation rules
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
            AST::Call {
                name: chimera_term::Atom::new(5), // Another function
                meta: chimera_ast::Meta::default(),
                args: vec![AST::Integer(100), AST::Integer(200)],
            },
            AST::Map(vec![
                (AST::Atom(chimera_term::Atom::new(6)), AST::Integer(1)),
                (
                    AST::Atom(chimera_term::Atom::new(7)),
                    AST::String("value".to_string()),
                ),
            ]),
        ],
        meta: chimera_ast::Meta::default(),
    }
}

fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");
    group.sample_size(100);

    group.bench_function("validate_ast", |b| {
        let ast = create_complex_ast();
        let validator = Validator::default();
        b.iter(|| {
            let result = validator.validate(&ast);
            // We expect validation to succeed for our test AST
            assert!(result.is_ok());
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_validation);
criterion_main!(benches);
