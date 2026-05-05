# Rust/Zig Elixir Compiler Frontend

A production-ready Elixir compiler frontend (v1.19.5/BEAM 16.4 target) written in Rust with Zig for bounded hot-path kernels.

**Key Principle**: Rust owns compiler state and semantics. Zig owns hot, low-level, bounded operations.

## Architecture

```
Elixir Source (.ex/.exs)
       |
       v
   rzx_lexer    (Zig kernels: UTF-8, sigils, heredocs, ETF)
       |
       v
   rzx_cst       (Lossless Concrete Syntax Tree)
       |
       v
   rzx_parser    (Quoted AST construction)
       |
       v
   rzx_expand    (MacroEnv, hygiene, special forms, quote/unquote)
       |
       v
   rzx_module    (Module builder with attributes, definitions)
       |
       v
   rzx_core      (Core IR: expressions, patterns, guards, clauses)
       |
       v
   rzx_codegen   (Target artifact emission)
       |
       v
   rzx_target    (Opaque adapter to external BEAM runtime)
```

## Features

- **Fast Lexing**: Zig-powered UTF-8 validation, sigil parsing, heredoc handling
- **Precise AST**: Full macro expansion with hygiene support
- **Core IR**: Strongly typed intermediate representation
- **BEAM Target**: Generates BEAM-compatible bytecode (16.4)
- **Plugin System**: Extensible compiler with custom hooks, lints, and transforms
- **Source Maps**: Debugging support with context and location tracking
- **Actionable Diagnostics**: Error messages with suggested fixes

## Crates

### Foundation
- `rzx_source` - Source files, spans, byte offsets, line/column mapping
- `rzx_diag` - Diagnostic types, severity, labels, notes, hints, suggestions
- `rzx_term` - Atom table, terms, ETF encoding/decoding
- `rzx_zig_abi` - Safe Rust wrappers around Zig kernels

### Zig Kernels (`zig/rzx_kernels/src/`)
- `utf8.zig` - UTF-8 validation, identifier scanning
- `scanner.zig` - String, charlist, heredoc, sigil scanning
- `bitstring.zig` - Bitstring segment parsing, validation
- `etf.zig` - Erlang External Term Format encode/decode
- `source_buffer.zig` - Source span operations, buffer scanning

### Frontend
- `rzx_lexer` - Tokenization with error recovery
- `rzx_cst` - Lossless concrete syntax tree
- `rzx_ast` - Typed quoted AST with hygiene
- `rzx_parser` - Expression grammar with precedence
- `rzx_expand` - Macro expansion and hygiene
- `rzx_module` - Module builder with attributes
- `rzx_core` - Core IR lowering
- `rzx_codegen` - BEAM bytecode emission

### Tools
- `rzx_cli` - Command-line driver
- `rzx_fmt` - Formatter from CST
- `rzx_lsp` - Language server protocol
- `rzx_lint` - Lint framework with built-in rules
- `rzx_compile` - Compiler driver pipeline

## Installation

```bash
# Build the compiler
cargo build --release

# Run tests
cargo test --release
```

## Usage

```bash
# Compile an Elixir file
cargo run --release --bin rzx -- compile example.ex

# Run the formatter
cargo run --release --bin rzx -- format example.ex

# Check for errors
cargo run --release --bin rzx -- check example.ex
```

## Plugin System

The compiler supports plugins via the `rzx_plugin_api` crate:

- **Discovery**: Plugins are discovered from directories and environment variables
- **Hooks**: Register callbacks for before/after compile phases
- **Custom Lints**: Register plugin-defined lint rules
- **Transforms**: Register AST transformation passes

Example plugin registration:
```rust
use rzx_plugin_api::{PluginDiscovery, PluginConfig, HookPhase, HookCallback};

// Discover plugins
let discovery = PluginDiscovery::new(PluginConfig::default());
let plugins = discovery.discover_all();

// Register a hook
let hook = HookCallback::new(HookPhase::BeforeCompile, "my-plugin", |ctx| {
    HookResult::success()
});
```

## Error Messages

Diagnostics include actionable suggestions:

```
error: syntax error
  --> example.ex:5:3
    suggestion: Did you mean 'def'?
      -> def foo do
    at 52:58
```

## Testing

- **Unit tests**: Each crate has internal tests
- **Integration tests**: `tests/` directory for cross-crate tests
- **Oracle tests**: Compare against official Elixir/OTP outputs

```bash
# Run all tests
cargo test --release

# Run specific crate tests
cargo test -p rzx_plugin_api --release
```

## Documentation

See `docs/design.md` for the full architecture and implementation task list.

## Contributing

1. Follow `cargo fmt` conventions
2. Add tests for new features
3. Update documentation when changing APIs
4. Keep code files under 800 lines

## Version Requirements

- Rust: 1.70+
- Elixir: 1.19.5
- BEAM (ERT): 16.4
- Zig: 0.11+ (for kernels)