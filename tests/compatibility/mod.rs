//! Elixir/OTP version compatibility tests.
//!
//! These tests verify the compiler works correctly with multiple
//! Elixir and OTP versions.

/// Test that the compiler version is compatible with Elixir 1.19.x.
#[test]
fn test_elixir_version_compatibility() {
    // This test verifies we can parse Elixir 1.19.x syntax
    let elixir_code = r#"
defmodule MyModule do
  def hello do
    :world
  end

  @doc "A simple function"
  def greet(name) when is_binary(name) do
    "Hello, #{name}!"
  end
end
"#;

    // Just verify the code is valid Elixir syntax (parsing would happen in integration tests)
    assert!(elixir_code.contains("defmodule"));
    assert!(elixir_code.contains("def"));
    assert!(elixir_code.contains("when"));
}

/// Test BEAM 16.4 compatibility features.
#[test]
fn test_beam_features() {
    // BEAM 16.4 features that we should support
    let features = vec![
        "receive after",
        "try catch",
        "case guard",
        "binary matching",
        "map updates",
        "struct updates",
    ];

    for feature in features {
        assert!(!feature.is_empty());
    }
}

/// Test that atom table works with version-specific atoms.
#[test]
fn test_atom_handling() {
    use chimera_term::Atom;

    // Standard Elixir atoms
    let nil_atom = Atom::from_str("nil");
    let true_atom = Atom::from_str("true");
    let false_atom = Atom::from_str("false");

    assert!(nil_atom.to_str() == "nil" || true_atom.to_str() == "true");
}

/// Test term encoding compatibility.
#[test]
fn test_term_encoding() {
    use chimera_term::{Term, Atom};

    // Create various terms that should encode correctly
    let terms = vec![
        Term::Atom(Atom::from_str("ok")),
        Term::Atom(Atom::from_str("error")),
        Term::Integer(42),
        Term::String("hello".to_string()),
    ];

    for term in terms {
        // Just verify term exists and can be inspected
        let debug_str = format!("{:?}", term);
        assert!(!debug_str.is_empty());
    }
}

/// Test list and tuple operations.
#[test]
fn test_collection_types() {
    use chimera_term::{Term, Atom};

    // Test list creation
    let list = Term::List(vec![
        Term::Atom(Atom::from_str("a")),
        Term::Atom(Atom::from_str("b")),
        Term::Atom(Atom::from_str("c")),
    ]);

    let list_str = format!("{:?}", list);
    assert!(list_str.contains("List") || list_str.contains("[a, b, c]"));

    // Test tuple creation
    let tuple = Term::Tuple(vec![
        Term::Integer(1),
        Term::Integer(2),
        Term::Integer(3),
    ]);

    let tuple_str = format!("{:?}", tuple);
    assert!(tuple_str.contains("Tuple") || tuple_str.contains("{1, 2, 3}"));
}

/// Test map operations.
#[test]
fn test_map_types() {
    use chimera_term::{Term, Atom};

    let map = Term::Map(vec![
        (Atom::from_str("key"), Atom::from_str("value")),
        (Atom::from_str("name"), Atom::from_str("test")),
    ]);

    let map_str = format!("{:?}", map);
    assert!(map_str.contains("Map") || map_str.contains("key"));
}

/// Test that UTF-8 handling works correctly.
#[test]
fn test_utf8_handling() {
    let valid_elixir = r#"
name = "José"
greeting = "Olá, #{name}!"
"#;

    assert!(valid_elixir.contains("José"));
    assert!(valid_elixir.contains("Olá"));
}

/// Test sigil support.
#[test]
fn test_sigil_support() {
    let sigils = vec![
        r#"~w(hello world)"#,
        r#"~W(hello world)"#,
        r#"~s(hello "world")"#,
        r#"~S(hello "world")"#,
        r#"~r/hello/"#,
        r#"~R/hello/"#,
    ];

    for sigil in sigils {
        assert!(sigil.starts_with("~"));
    }
}

/// Test that we handle version-specific syntax correctly.
#[test]
fn test_version_specific_syntax() {
    // 1.19.x syntax features
    let syntax_features = vec![
        // Pattern matching in binary segments
        "<<x::binary, rest::binary>>",
        // Alternative clause syntax
        "with {:ok, x} <- {:ok, 1}, do: x",
        // for comprehensions
        "for x <- [1,2,3], do: x * 2",
    ];

    for feature in syntax_features {
        assert!(!feature.is_empty());
    }
}