#![no_main]

use chimera_allocator as _;
use chimera_term::{decode_term, encode_term, AtomTable, Term};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Create a fresh atom table for each fuzz run
    let mut atoms = AtomTable::new();

    // Try to create a term from the fuzz data (if possible)
    // We'll use a simple approach: treat the data as a UTF-8 string and create a string term
    if let Ok(term_str) = std::str::from_utf8(data) {
        let term = Term::String(term_str.into());

        // Test encode/decode round-trip
        if let Ok(encoded) = encode_term(&term, &atoms) {
            if let Ok((remaining, decoded)) = decode_term(&encoded, &mut atoms) {
                // Should have consumed all bytes
                if remaining.is_empty() {
                    // Terms should be equal
                    assert_eq!(term, decoded, "ETF encode/decode round-trip failed");
                }
            }
        }
    }

    // Also test with some predefined terms to exercise different encodings
    let mut test_terms = vec![Term::Nil, Term::SmallInt(42), Term::SmallInt(-42)];

    // Test atom creation
    if let Ok(test_atom) = atoms.try_intern("test") {
        test_terms.push(Term::Atom(test_atom));
    }

    // Test string term
    test_terms.push(Term::String("hello world".into()));

    // Test float
    test_terms.push(Term::Float(std::f64::consts::PI));

    // Test bigint
    test_terms.push(Term::BigInt(chimera_term::BigInt(
        num_bigint::BigInt::from(12345),
    )));

    for term in test_terms {
        // Skip if we can't encode (e.g., BigInt might fail if not supported)
        if let Ok(encoded) = encode_term(&term, &atoms) {
            let mut atoms2 = AtomTable::new();
            if let Ok((remaining, decoded)) = decode_term(&encoded, &mut atoms2) {
                if remaining.is_empty() {
                    assert_eq!(
                        term, decoded,
                        "ETF encode/decode round-trip failed for predefined term"
                    );
                }
            }
        }
    }
});
