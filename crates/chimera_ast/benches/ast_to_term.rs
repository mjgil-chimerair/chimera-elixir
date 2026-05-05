use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chimera_ast::{AST, to_term};
use chimera_term::AtomTable;

fn create_complex_ast() -> AST {
    // Create a moderately complex AST for benchmarking
    AST::Block {
        exprs: vec![
            AST::Integer(42),
            AST::String("hello world".to_string()),
            AST::List(vec![
                AST::Atom(chimera_term::Atom::new(1)), // Using interned atom for true (reserved atom ID 1)
                AST::Tuple(vec![
                    AST::Float(3.14),
                    AST::Atom(chimera_term::Atom::new(2)), // Using interned atom for false (reserved atom ID 2)
                ]),
            ]),
            AST::Call {
                name: chimera_term::Atom::new(3), // Some function atom (ID 3, after reserved atoms)
                meta: chimera_ast::Meta::default(),
                args: vec![
                    AST::Nil,
                    AST::Var {
                        name: chimera_term::Atom::new(4), // Some variable atom (ID 4)
                        meta: chimera_ast::Meta::default(),
                    },
                ],
            },
        ],
        meta: chimera_ast::Meta::default(),
    }
}

fn bench_ast_to_term(c: &mut Criterion) {
    let mut group = c.benchmark_group("ast_to_term");
    group.sample_size(100);
    
    group.bench_function("complex_ast", |b| {
        let ast = create_complex_ast();
        b.iter(|| {
            let mut atoms = AtomTable::new();
            black_box(to_term(black_box(&ast), black_box(&mut atoms)));
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_ast_to_term);
criterion_main!(benches);