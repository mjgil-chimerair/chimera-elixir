# Contributing to Rust/Zig Elixir Compiler

Thank you for your interest in contributing to this project!

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Zig 0.11 or later (for kernel development)
- Elixir 1.19.5 / BEAM 16.4 (for testing)
- Optional: Nightly Rust for fuzzing (`rustup install nightly`)

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/your-org/rustzigelixir.git
cd rustzigelixir

# Build the project
cargo build --release

# Run tests to ensure everything works
cargo test --release
```

## Code Style

- **Indentation**: 4 spaces (no tabs)
- **Line Length**: Maximum 100 characters
- **Formatting**: Follow `cargo fmt` conventions
- **File Size**: Keep code files under 800 lines; refactor if they exceed

## Testing

Every new feature requires tests:

```bash
# Run all tests
cargo test --release

# Run tests for a specific crate
cargo test -p rzx_plugin_api --release

# Run tests with output
cargo test -p rzx_diag --release -- --nocapture
```

### Test Categories

- **Unit tests**: Inside each crate's `lib.rs` in `#[cfg(test)]` module
- **Integration tests**: In the `tests/` directory at repo root
- **Oracle tests**: Compare output against official Elixir/OTP
- **Property-based tests**: Using proptest framework (look for `proptest!` macros)
- **Fuzz tests**: Using cargo-fuzz (in `fuzz/` directories)

## Benchmarking

The project uses criterion for benchmarking. To run benchmarks:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmarks
cargo bench --bench codegen_bench
cargo bench --bench parse_bench
cargo bench --bench lex_bench

# Compare with baseline (used in CI)
cargo bench --bench codegen_bench --baseline=ci-baseline
```

Benchmark results are stored in the `target/criterion` directory.

## Fuzz Testing

The project includes fuzz targets for security testing. To run fuzzing:

```bash
# Install cargo-fuzz (if not already installed)
cargo install cargo-fuzz

# Run fuzzing for a specific component
cargo fuzz run fuzz_parser -- -max_len=100
cargo fuzz run fuzz_lexer -- -max_len=100
cargo fuzz run fuzz_etf -- -max_len=100
```

Fuzz tests are located in each crate's `fuzz/` directory and use libfuzzer-sys.

### Adding New Fuzz Targets

1. Create a `fuzz/` directory in the crate if it doesn't exist
2. Add a `Cargo.toml` in the fuzz directory with:
   ```toml
   [package]
   name = "crateName_fuzz"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   crateName = { path = ".." }
   libfuzzer-sys = "0.4"

   [package.metadata]
   cargo-fuzz = true

   [[bin]]
   name = "fuzz_function_name"
   path = "fuzz_function_name.rs"
   ```
3. Create the fuzz target Rust file (e.g., `fuzz/fuzz_function_name.rs`)
4. Add the fuzz directory to the workspace members in the root `Cargo.toml`

## Release Process

The project uses GitHub Actions for automated releases:

### Making a Release

1. Ensure all tests pass on the main branch
2. Tag a commit with a version number (e.g., `v1.0.0`)
3. Push the tag: `git push origin v1.0.0`
4. The Release workflow will automatically:
   - Build the binaries
   - Run tests
   - Create a GitHub release with artifacts
   - Generate checksums
   - Create a Homebrew formula PR

### Beta Releases

Beta releases are automatically generated from pull requests:
- PRs against main trigger beta builds
- Beta versions follow the format: `X.Y.Z-beta.N`
- Beta releases are marked as prereleases in GitHub
- Version numbers are automatically reverted after PR closes

## Commit Messages

Follow this format:
```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`

Scopes should correspond to crate names or major components (e.g., `parser`, `lexer`, `codegen`, `term`).

## Architecture Principles

### Rust Owns

- Compiler state and semantics
- AST manipulation and lowering
- Type checking and validation
- Error handling and reporting
- Module system and imports

### Zig Owns

- Bounded hot-path kernels
- UTF-8 validation
- Bitstring parsing
- ETF encoding/decoding
- Source buffer operations

## Crate Structure

- **Foundation crates** (`rzx_source`, `rzx_diag`, `rzx_term`, `rzx_zig_abi`): No dependencies on compiler stages
- **Frontend crates**: Depend on foundation + earlier frontend stages
- **Tool crates**: Depend on multiple stages for orchestration

## Documentation

When changing APIs:
1. Update inline documentation
2. Update README if user-facing
3. Update design documents if architecture changes
4. Add doc comments to public functions

When adding features:
1. Consider adding examples to key modules
2. Update benchmark suites if performance characteristics change
3. Consider adding fuzz targets for security-critical components
4. Update the changelog script if needed

## Filing Issues

When reporting bugs:
1. Include the Rust version
2. Include steps to reproduce
3. Include expected vs actual behavior
4. Add a minimal test case if possible
5. For performance issues, consider adding a benchmark

## Getting Help

- Check the design documents in `docs/` for architecture context
- Review existing tests for patterns
- Ask in issues for clarification
- For contribution guidance, see this file (CONTRIBUTING.md)