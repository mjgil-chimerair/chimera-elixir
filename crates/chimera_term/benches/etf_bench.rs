use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chimera_term::{AtomTable, Term};
use std::io::Cursor;

fn create_test_terms() -> Vec<Term> {
    let mut terms = Vec::new();
    let mut atom_table = AtomTable::new();
    
    // Small integers
    for i in -100..100 {
        terms.push(Term::SmallInt(i));
    }
    
    // Atoms
    terms.push(Term::Atom(atom_table.intern("hello")));
    terms.push(Term::Atom(atom_table.intern("world")));
    terms.push(Term::Atom(atom_table.intern(" Elixir")));
    
    // Binaries
    terms.push(Term::Binary(vec![1, 2, 3, 4, 5], None));
    terms.push(Term::Binary(b"hello world".to_vec(), None));
    
    // Lists
    terms.push(Term::List(vec![
        Term::SmallInt(1),
        Term::SmallInt(2),
        Term::SmallInt(3),
    ]));
    
    // Tuples
    terms.push(Term::Tuple(vec![
        Term::SmallInt(42),
        Term::Atom(atom_table.intern("answer")),
        Term::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF], None),
    ]));
    
    terms
}

fn bench_etf_encode_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("etf_encode_decode");
    group.sample_size(50);
    
    let terms = create_test_terms();
    
    group.bench_function("encode_decode_terms", |b| {
        b.iter(|| {
            for term in &terms {
                // Encode term to ETF
                let mut atoms = AtomTable::new();
                let encoded = chimera_term::encode_term(term, &atoms).unwrap();
                
                // Decode ETF back to term
                let mut atoms = AtomTable::new();
                let (_remaining, decoded) = chimera_term::decode_term(&encoded, &mut atoms).unwrap();
                
                black_box((encoded, decoded));
            }
        });
    });
    
    group.finish();
}

fn bench_large_binary_etf(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_binary_etf");
    group.sample_size(20);
    
    // Create a larger binary for testing
    let large_binary: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
    let term = Term::Binary(large_binary, None);
    
    group.bench_function("encode_large_binary", |b| {
        b.iter(|| {
            let mut atoms = AtomTable::new();
            let encoded = chimera_term::encode_term(&term, &atoms).unwrap();
            black_box(encoded);
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_etf_encode_decode, bench_large_binary_etf);
criterion_main!(benches);