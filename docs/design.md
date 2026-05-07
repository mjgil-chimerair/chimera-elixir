# Chimera-Elixir Design

## Summary
`chimera-elixir` is a reimplementation of Elixir with Rust as the host language for the compiler, runtime, and process model.

Implementation split from the source table:

| Host language | Share | Role |
| --- | ---: | --- |
| Rust | 55% | Parser, compiler, BEAM-adjacent runtime, scheduler, process model, tooling |
| OCaml | 20% | Frontend and macro/type-analysis prototypes |
| C++ | 15% | NIF or JIT-adjacent components if needed |
| Zig | 10% | VM hot-path utilities and low-level runtime helpers |

Winner: Rust. Reason: an Elixir implementation outside BEAM needs strong concurrency, scheduler safety, process isolation, and runtime correctness.

## Goals
- Recreate the Elixir language experience with a modern, memory-safe implementation base.
- Support macros, modules, pattern matching, processes, and OTP-style building blocks.
- Allow two execution modes: transpile-to-BEAM-compatible artifacts where practical, and a native Chimera runtime path for deeper control.

## Non-Goals
- Full OTP ecosystem compatibility in phase 1.
- Perfect BEAM bytecode compatibility if it compromises the architecture.
- A tracing JIT before the runtime semantics are stable.

## Architecture
1. Frontend
- Rust lexer/parser for Elixir syntax, quoting/unquoting, pattern matching, modules, and protocols.
- Expand macros in explicit stages with source hygiene preserved.

1. Semantic model
- Module environment, imports/aliases, protocol dispatch metadata, and pattern-match exhaustiveness checks where useful.

1. IR
- High-level Elixir IR after macro expansion.
- Lowered process/runtime IR that makes message passing, receives, and fault boundaries explicit.

1. Runtime
- Rust-owned lightweight processes, mailboxes, schedulers, supervision primitives, and binary/term representation.
- Zig can support compact low-level pieces such as term layout helpers or lock-free queue experiments.

1. Interop
- C++ reserved for optional JIT, special native integration layers, or compatibility shims.
- OCaml reserved for frontend experimentation and formalization of macro expansion invariants.

## Percentage-to-Subsystem Mapping
- Rust 55%: `frontend/`, `macro-expander/`, `hir/`, `runtime/`, `scheduler/`, `mailbox/`, `supervision/`, `cli/`.
- OCaml 20%: `semantics-lab/`, `macro-formalization/`, `pattern-analysis/`.
- C++ 15%: `jit/` or specialized native interop only if required.
- Zig 10%: `vm-hotpath/`, `term-layout/`, `atom-table/` helpers.

## Key Design Decisions
- The runtime is not an afterthought; it is the product.
- Message passing, reduction accounting, and fault isolation are explicit IR concerns.
- Macro hygiene and quote/unquote behavior are defined before optimization.

## Phases
1. Frontend, AST, macro quoting model, and diagnostics.
1. Module system, pattern matching, functions, and protocols.
1. Process runtime, mailboxes, receive semantics, and supervision baseline.
1. OTP-inspired libraries and interoperability strategy.
1. Optional JIT/native acceleration and distributed-node experiments.

## Testing Strategy
- Parser and macro-expansion golden tests.
- Semantic tests for pattern matching, guards, protocols, and module resolution.
- Runtime tests for mailbox ordering, process crashes, links/monitors, and supervisors.
- Differential tests against Elixir/BEAM behavior where semantics are externally visible.

## Major Risks
- Reimplementing Elixir without a strong runtime plan leads to a shallow compiler and weak execution model.
- Macro behavior plus process semantics can make debugging difficult unless observability is built in early.
- OTP compatibility pressure can explode scope.

## Staffing Plan
- Rust team owns the mainline compiler/runtime.
- OCaml team supports semantic modeling and macro correctness.
- C++ and Zig are narrowly scoped performance/interop contributors.

## Exit Criteria For V1
- Runs Elixir-like programs with modules, pattern matching, macros, and processes.
- Has working supervision and observable fault boundaries.
- Supports a stable internal term format and scheduler contract.

---

## Implementation Task List

A complete, numbered task list for achieving V1 exit criteria. A task is **Complete** only when implementation, feature-specific tests, and feature documentation are all present.

### Foundation: Term System and Atom Layer

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 1. | **Atom Table** | Rust | Implement atom interning with unique integer IDs. Support UTF-8 atom names. Define atom-to-index and index-to-atom bidirectional mapping. Thread-safe reference counting for atoms. | **Complete** | Unit tests for atom creation, uniqueness, equality, hashing. Concurrent access tests. | Atom table design doc, public API docs | `chimera_term` |
| 2. | **Term Layout** | Rust | Define enum for all Elixir terms: `Atom`, `Integer`, `Float`, `Binary`, `List`, `Tuple`, `Map`, `Pid`, `Port`, `Reference`, `Fun`, `BitString`, `Struct`. Implement tag_bits representation for immediate vs heap-allocated terms. Implement term comparison trait. | **Complete** | Property tests for term equality, ordering. Marshall/unmarshal round-trip tests. | Term layout specification | `chimera_term` |
| 3. | **Binary/Term Encoding (ETF)** | Rust | Implement Erlang Term Format encoding/decoding. Support all term types per ETF spec. Handle version header. Optimize for small binaries. | **Complete** | Encode/decode golden tests for all term types. Interop tests with Erlang/Elixir nodes. | ETF specification doc | `chimera_term` |
| 4. | **Heap Allocation** | Rust | Implement bump allocation arena. Define allocation strategies for terms (young vs old generation). Implement term copying on write. GC-safe pointer handling. | **Complete** | Allocation stress tests. Memory reclamation tests. Fragmentation tests. | Memory management design | `chimera_allocator` |

### Foundation: Lexer and Parser

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 5. | **Lexer** | Rust | Implement tokenization for Elixir syntax: keywords, identifiers, atoms, sigils, strings, char literals, integers, floats, operators, delimiters. Handle heredocs, interpolation, escaped characters. Preserve source location for every token. | **Complete** | Golden tests against Elixir reference tokenizer. Unicode identifier tests. Sigil variants. | Lexer specification | `chimera_lexer` |
| 6. | **Parser (CST)** | Rust | Implement concrete syntax tree parser following Elixir grammar. Handle all expression forms: assignment, binop, unary, call, fn, cond, receive, try, raise, case, with. Support guard expressions. Error recovery with panic mode. | **Complete** | Parser golden tests against Elixir reference parse. Error recovery tests. Source location preservation tests. | CST specification | `chimera_cst`, `chimera_parser` |
| 7. | **AST Definitions** | Rust | Define AST node types: `Atom`, `Integer`, `Float`, `String`, `Charlist`, `InterpolatedString`, `List`, `Tuple`, `Map`, `Struct`, `Binary`, `Block`, `Assignment`, `Call`, `Fn`, `If`, `Case`, `Receive`, `Try`, `Raise`, `MacroCall`, `Macroexpand`, `Quote`, `Unquote`, `Alias`, `Import`, `Require`, `Use`, `Def`, `Defp`, `Defmacro`, `Defmacrop`, `Protocol`, `Impl`, `Behaviour`. | **Complete** | AST round-trip tests (parse -> AST -> parse). Transformation tests. | AST node specification | `chimera_ast` |

### Foundation: Macro Expansion

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 8. | **AST Transform Pipeline** | Rust | Implement sequential transform stages: desugar, expand, normalize. Define Transform trait with `enter`/`leave` hooks. Support visitor pattern traversal. | **Complete** | Transform pipeline tests for each stage. Hook invocation order tests. | Transform framework doc | `chimera_ast_transform` |
| 9. | **Macro Expansion** | Rust | Implement `Code.expand_macros/2` semantics. Handle `__ENV__`, `__CALLER__`, `__MODULE__` built-ins. Support alias expansion, import resolution, require checking. Implement unquote and unquote_splicing. Preserve source hygiene. | **Complete** | Macro expansion golden tests. Hygiene preservation tests. Cross-module expansion tests. | Macro expansion design | `chimera_expand` |
| 10. | **Quote/Unquote** | Rust | Implement AST quoting with `quote/1`/`quote/2`. Handle interpolation in quoted blocks. Implement `unquote/1` and `unquote_splicing/1`. Support context passing for hygiene. | **Complete** | Quote/unquote round-trip tests. Splicing tests. Context inheritance tests. | Quote/unquote spec | `chimera_expand` |

### Foundation: Module System

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 11. | **Pattern Matching Compiler** | Rust | Implement pattern to decision tree compilation. Handle literal, variable, wildcard, cons, tuple, map, struct, and complex nested patterns. Implement guard sequence evaluation. | **Complete** | Pattern match exhaustiveness tests. Overlapping pattern tests. Guard evaluation tests. | Pattern matching design | `chimera_core` |
| 12. | **Module Environment** | Rust | Implement `Module.env` data structure. Track current module, imports, aliases, exports, function arities, docs, compile attrs. Implement lexical scope nesting. | **Complete** | Module environment tests. Nested scope tests. Import resolution order tests. | Module environment spec | `chimera_module` |
| 13. | **Import/Alias Resolution** | Rust | Implement import resolution with `unless`, `only`, `except` options. Handle function/macro import. Implement alias tracking and expansion. Support multi-alias syntax. | **Complete** | Import resolution tests. Alias expansion tests. Prefix resolution tests. | Import/alias design | `chimera_module` |
| 14. | **Protocol Dispatch** | Rust | Implement protocol definition and implementation. Define protocol dispatch table. Implement `defprotocol`/`defimpl` compilation. Handle consolidation at compile time. | **Complete** | Protocol dispatch tests. Impl lookup tests. Consolidation tests. | Protocol design | `chimera_core` |
| 15. | **Module Compilation** | Rust | Implement module definition compilation: `defmodule`, `def`, `defp`, `defmacro`, `defmacrop`, `callback`, `optional_callbacks`. Handle attribute storage and retrieval (`@`). Implement docs and typespecs storage. | **Complete** | Module definition compilation tests. Attribute storage tests. Callback implementation tests. | Module compilation spec | `chimera_compile` |
| 16. | **Function Arity Resolution** | Rust | Implement arity-based function lookup. Handle multi-clause functions. Implement default argument desugaring. Support fallback to `__info__(:functions)`. | **Complete** | Arity resolution tests. Default argument tests. Multi-clause tests. | Function resolution spec | `chimera_core` |

### OCaml: Semantic Prototypes (20%)

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 16a. | **Macro Formalization** | OCaml | Formalize macro expansion invariants in OCaml. Define hygiene boundaries, unquote expansion order, context propagation rules. Produce executable specification for Rust implementation. | **Complete** | Formal verification proofs. Reference test suite against Elixir/BEAM. | Macro formalization doc | `semantics-lab/` |
| 16b. | **Pattern Analysis** | OCaml | Prototype exhaustiveness checking algorithms in OCaml. Define pattern tree semantics, guard evaluation order, warning generation rules. | **Complete** | Pattern analysis unit tests. Cross-validation against Rust implementation. | Pattern analysis doc | `semantics-lab/` |
| 16c. | **Type-System Prototyping** | OCaml | Prototype Elixir type inference and typespec validation. Define type environment, union types, structural equivalence. | **Complete** | Type inference tests. Typespec validation tests. | Type system doc | `semantics-lab/` |

### C++: JIT/NIF Components (15%)

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 16d. | **NIF Bridge** | C++ | Implement C++ NIF (Native Implemented Function) bridge. Define `ErlNif` resource management, thread-local environment, dirty scheduler support. | **Complete** | NIF loading tests. Resource cleanup tests. Thread-local env tests. | NIF bridge design | `jit/` |
| 16e. | **JIT Interface** | C++ | Define JIT compilation interface if required. Implement hot-path detection, machine code generation stubs, call optimization. | **Complete** | JIT compilation tests. Hot-path detection tests. | JIT interface spec | `jit/` |
| 16f. | **Native Integration Layer** | C++ | Implement native integration primitives for C++ interop. Handle port communication, binary transfer, term marshalling across FFI boundary. | **Complete** | Native integration tests. Port communication tests. | Native integration doc | `jit/` |

### Core IR and Code Generation

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 17. | **High-Level IR (HIR)** | Rust | Define HIR node types after macro expansion: `HirModule`, `HirDef`, `HirCall`, `HirPattern`, `HirGuard`, `HirLiteral`, `HirTuple`, `HirList`, `HirMap`, `HirBinary`, `HirReceive`, `HirSend`, `HirSpawn`, `HirLink`, `HirMonitor`. Implement HIR builder from AST. | **Complete** | HIR construction tests. Node completeness tests. | HIR design doc | `chimera_core` |
| 18. | **Process Runtime IR (LRIR)** | Rust | Define lowered IR for processes: `ProcSpawn`, `ProcSend`, `ProcReceive`, `ProcLink`, `ProcMonitor`, `ProcExit`, `ProcRaise`, `FaultBoundary`, `SupervisorSpec`. Implement HIR-to-LRIR lowering pass. | **Complete** | LRIR lowering tests. Process boundary tests. | LRIR design doc | `chimera_core` |
| 19. | **Pattern-Match Exhaustiveness** | Rust | Implement exhaustiveness checking for case expressions. Handle guard clauses. Implement warning generation for non-exhaustive patterns. | **Complete** | Exhaustiveness tests. Warning generation tests. Guard handling tests. | Exhaustiveness checker design | `chimera_lint` |
| 20. | **BEAM Term Encoding** | Rust | Implement BEAM .beam file term encoding. Encode atoms, literals, funs, pid, port, reference per BEAM spec. Handle compressed and uncompressed formats. | **Complete** | BEAM term encoding tests. Version header tests. | BEAM term format spec | `chimera_beam_term` |
| 21. | **BEAM Chunk Encoding** | Rust | Implement BEAM file chunks: `Atom`, `Code`, `StrT`, `Line`, `LitT`, `ImpT`, `ExpT`, `FunT`, `LocT`, `CInfo`, `OnLoad`, `Attr`, `Compile`. Implement chunk ordering and size calculation. | **Complete** | Chunk encoding tests. File structure tests. | BEAM file format spec | `chimera_target` |
| 22. | **Code Gen: Module** | Rust | Generate BEAM module header and metadata chunks. Implement function table generation. Handle module attributes in info chunk. | **Complete** | Module codegen tests. Metadata completeness tests. | Codegen design | `chimera_codegen` |
| 23. | **Code Gen: Functions** | Rust | Generate BEAM instructions for function clauses. Handle pattern dispatch. Implement BEAM instruction selection from LRIR. Support guard instruction generation. | **Complete** | Function codegen tests. Instruction selection tests. Guard codegen tests. | BEAM instruction set spec | `chimera_codegen` |
| 24. | **Code Gen: Patterns** | Rust | Implement pattern compilation to BEAM match instructions. Handle variable allocation. Implement term loading instructions. | **Complete** | Pattern codegen tests. Variable allocation tests. | Pattern codegen design | `chimera_codegen` |
| 25. | **Code Gen: Message Passing** | Rust | Generate BEAM instructions for `send` and `receive`. Handle timeout handling. Implement mailbox receive loop compilation. | **Complete** | Message passing codegen tests. Receive loop tests. Timeout handling tests. | Message passing codegen doc | `chimera_codegen` |
| 26. | **Code Gen: Process Ops** | Rust | Generate BEAM instructions for spawn, link, monitor, exit. Implement process creation and termination sequences. | **Complete** | Process ops codegen tests. Link/monitor tests. | Process ops codegen doc | `chimera_codegen` |

### Process Model and Runtime

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 27. | **Process Model** | Rust | Implement `Process` struct with pid, mailbox, dictionary, group leader, links, monitors. Implement process state machine: running, waiting, exiting. Handle process dictionary. | **Complete** | Process lifecycle tests. Dictionary tests. Group leader tests. | Process model design | `chimera_core` |
| 28. | **Mailbox** | Rust | Implement message mailbox with FIFO ordering. Support out-of-order inspection. Implement `receive` pattern matching against mailbox contents. Handle timeout. | **Complete** | Mailbox FIFO tests. Receive pattern matching tests. Timeout tests. | Mailbox design | `chimera_core` |
| 29. | **Scheduler** | Rust | Implement scheduler loop with reduction counting. Handle process ready queue. Implement sleep/wakeup for waiting processes. Support scheduler affinity hints. | **Complete** | Scheduler loop tests. Reduction counting tests. Process scheduling order tests. | Scheduler contract doc | `chimera_core` |
| 30. | **Process Spawn/Exit** | Rust | Implement `spawn`/`spawn_link`/`spawn_monitor`. Handle process exit with reason. Implement exit propagation through links. | **Complete** | Spawn tests. Exit propagation tests. Link chain tests. | Process spawn design | `chimera_core` |
| 31. | **Links and Monitors** | Rust | Implement bidirectional link management. Implement monitor creation and delivery. Handle `Process.link/1` and `Process.monitor/1`. Deliver exit signals on process death. | **Complete** | Link tests. Monitor tests. Signal delivery tests. | Link/monitor design | `chimera_core` |
| 32. | **Fault Boundaries** | Rust | Implement try/catch/after semantics at IR level. Define fault boundary as explicit IR node. Implement stack trace capture on raise. Handle exception unwinding. | **Complete** | Fault boundary tests. Exception propagation tests. Stack trace tests. | Fault boundary design | `chimera_expand` |
| 33. | **Supervision Trees** | Rust | Implement `Supervisor` behavior with `init/1` callback. Support `one_for_one`, `one_for_all`, `rest_for_one` strategies. Implement child specification parsing. Handle child start/restart/terminate lifecycle. | **Complete** | Supervision tree tests. Restart strategy tests. Child lifecycle tests. | Supervisor design | `chimera_stdlib` |
| 34. | **Supervisor Spec** | Rust | Implement child spec format: `id`, `start`, `restart`, `shutdown`, `type`, `modules`. Handle permanent, transient, temporary restart types. | **Complete** | Child spec parsing tests. Restart type tests. Shutdown timeout tests. | Child spec format doc | `chimera_stdlib` |
| 35. | **Fault Recovery** | Rust | Implement supervisor restart logic. Handle exponential backoff. Implement max restarts within period. Implement shutdown on too many failures. | **Complete** | Restart logic tests. Backoff tests. Shutdown tests. | Restart logic design | `chimera_stdlib` |

### OTP Behaviors

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 36. | **GenServer Behavior** | Rust | Implement `GenServer` behavior with `init/1`, `handle_call/3`, `handle_cast/2`, `handle_info/2`. Implement code change callback. Implement format_status. | **Complete** | GenServer tests. Call/cast/info handling tests. Code change tests. | GenServer design | `chimera_stdlib` |
| 37. | **Task Behavior** | Rust | Implement `Task` and `Task.Supervisor`. Handle async task spawning. Implement task shutdown. | **Complete** | Task tests. Task supervisor tests. | Task design | `chimera_stdlib` |
| 38. | **Agent/Registry** | Rust | Implement `Agent` and `Registry` behaviors for V1 baseline. | **Complete** | Agent tests. Registry tests. | Agent/Registry design | `chimera_stdlib` |

### Standard Library

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 39. | **Kernel Module** | Rust | Implement `Kernel` module: `+`, `-`, `*`, `/`, `div`, `rem`, `and`, `or`, `not`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `apply`, `fun_info`, `inspect`, `is_*` guards, `abs`, `ceil`, `floor`, `round`, `trunc`, `node`, `self`, `make_ref`, `exit`, `throw`, `raise`, `receive`, `send`, `spawn`, `link`, `monitor`, `process_info`, `hd`, `tl`, `list_to_tuple`, `tuple_to_list`, `element`, `setelement`, `map_size`, `map_put`, `map_get`, `map_update`, `binary_part`, `bit_size`, `byte_size`, `match?`, `in`, `++`, `--`, `<>`. | **Complete** | Kernel function tests for each function. | Kernel spec | `chimera_stdlib` |
| 40. | **Enum Module** | Rust | Implement `Enum` module for iteration protocol. Implement `Enumerable` protocol. | **Complete** | Enum tests. Enumerable protocol tests. | Enum spec | `chimera_stdlib` |
| 41. | **List Module** | Rust | Implement `List` module with `flatten/1`, `flatten/2`, `wrap/1`, `delete/2`, `delete_at/2`, `first/1`, `last/1`, `insert_at/3`, `replace_at/3`, `update_at/3`, `duplicate/2`, `keyfind/4`, `keymember/3`, `keysearch/4`, `keysort/2`, `keystore/4`, `keytake/3`. | **Complete** | List module tests. | List module spec | `chimera_stdlib` |
| 42. | **String Module** | Rust | Implement `String` module with unicode support. Implement `String.Chars` protocol. | **Complete** | String tests. Unicode tests. | String spec | `chimera_stdlib` |
| 43. | **Tuple Module** | Rust | Implement `Tuple` module with `append/2`, `delete_at/2`, `duplicate/2`, `insert_at/3`, `set_tile/3`, `to_list/1`. | **Complete** | Tuple tests. | Tuple spec | `chimera_stdlib` |
| 44. | **Map Module** | Rust | Implement `Map` module with all functions. | **Complete** | Map tests. | Map spec | `chimera_stdlib` |
| 45. | **Process Module** | Rust | Implement `Process` module with `alive?/1`, `cancel_timer/1`, `send_destination/1`, `sleep/1`, `yield/2`. | **Complete** | Process module tests. | Process spec | `chimera_stdlib` |
| 46. | **Module Module** | Rust | Implement `Module` module with `get_attribute/2`, `put_attribute/3`, `delete_attribute/2`, `define_attribute/3`, `compile_env/0`, `make_overridable/1`, `assert_no_imports/1`. | **Complete** | Module tests. | Module spec | `chimera_stdlib` |
| 47. | **Code Module** | Rust | Implement `Code` module with `require/1`, `require/2`, `compile_file/1`, `compile_string/1`, `eval_file/1`, `eval_string/1`, `ensure_loaded/1`, `prepend_paths/1`, `append_paths/1`. | **Complete** | Code module tests. | Code spec | `chimera_stdlib` |

### CLI and Tooling

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 48. | **CLI: Compilation Driver** | Rust | Implement `chimera compile` command. Handle file arguments, output directory, include paths. Support `--watch` mode. | **Complete** | Compilation driver tests. Watch mode tests. | CLI user guide | `chimera_cli` |
| 49. | **CLI: REPL** | Rust | Implement `chimera repl` command. Support expression evaluation, history, completion. Implement `-e` and `-r` options. | **Complete** | REPL tests. Expression evaluation tests. | REPL user guide | `chimera_cli` |
| 50. | **CLI: Run** | Rust | Implement `chimera run` command to run scripts. Handle `-S` flag for script search path. | **Complete** | Run command tests. Script loading tests. | CLI user guide | `chimera_cli` |
| 51. | **CLI: Tasks** | Rust | Implement `mix` compatible `mix run`, `mix compile`, `mix test` commands as shims to `chimera`. Support `mix.exs` parsing. | **Complete** | Mix compatibility tests. Task execution tests. | Mix compatibility doc | `chimera_cli` |
| 52. | **Build System** | Rust | Implement `chimera_build` for project compilation management. Handle mix.exs and rebar.config compatibility. | **Complete** | Build system tests. Project compilation tests. | Build system design | `chimera_build` |
| 53. | **Formatter** | Rust | Implement `chimera fmt` for code formatting. Support `--check` mode. Handle configurable style. | **Complete** | Format round-trip tests. Check mode tests. | Formatter spec | `chimera_fmt` |
| 54. | **Diagnostics** | Rust | Implement error and warning reporter with source locations. Support formatted error messages. | **Complete** | Diagnostic output tests. Error format tests. | Diagnostic spec | `chimera_diag` |
| 55. | **Cross-Reference** | Rust | Implement `xref` analysis: calls, exports, implicit imports. | **Complete** | Xref analysis tests. | Xref design | `chimera_xref` |

### Zig Kernel Integration

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 56. | **Zig: Source Buffer** | Zig | Implement `SourceBuffer` in Zig for efficient source text handling. Support line/column tracking. Handle large files. | **Complete** | Source buffer tests. Large file tests. | Source buffer design | `zig/chimera_kernels` |
| 57. | **Zig: UTF-8 Validation** | Zig | Implement UTF-8 validation in Zig. Handle grapheme cluster detection. | **Complete** | UTF-8 validation tests. Grapheme tests. | UTF-8 design | `zig/chimera_kernels` |
| 58. | **Zig: Bitstring Primitives** | Zig | Implement bitstring operations in Zig: pattern matching, bitwise operations, segment extraction. | **Complete** | Bitstring operation tests. Pattern tests. | Bitstring design | `zig/chimera_kernels` |
| 59. | **Zig: Scanner Extension** | Zig | Extend scanner for Elixir-specific patterns: sigils, heredocs, dot separators. | **Complete** | Sigil tests. Heredoc tests. | Scanner design | `zig/chimera_kernels` |
| 60. | **Zig: ETF Primitives** | Zig | Implement efficient ETF encoding/decoding helpers in Zig for hot path. | **Complete** | ETF hot path tests. Performance benchmarks. | ETF design | `zig/chimera_kernels` |
| 61. | **Zig: Term Layout Helpers** | Zig | Implement term comparison and hashing in Zig for hot path. | **Complete** | Term comparison tests. Hash consistency tests. | Term layout design | `zig/chimera_kernels` |

### Language Server and IDE Support

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 62. | **LSP: Basics** | Rust | Implement basic LSP server: text document sync, diagnostics, completions. | **Complete** | LSP integration tests. Basic hover tests. | LSP protocol spec | `chimera_lsp` |
| 63. | **LSP: Goto Definition** | Rust | Implement goto definition for functions, modules, types. | **Complete** | Goto definition tests. | LSP design | `chimera_lsp` |
| 64. | **Typespec Validation** | Rust | Implement @type, @callback, @spec parsing and validation. | **Complete** | Typespec tests. Validation tests. | Typespec design | `chimera_typespec` |

### Targets and Plugins

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 65. | **BEAM Target Backend** | Rust | Implement `chimera_target` as BEAM code generation backend. Wire up to compilation pipeline. | **Complete** | End-to-end compilation tests. .beam file generation tests. | BEAM target design | `chimera_target` |
| 66. | **WASM Target** | Rust | Implement WASM compilation target as alternative to BEAM. | **Complete** | WASM compilation tests. | WASM target design | `chimera_wasm` |
| 67. | **Plugin API** | Rust | Implement `chimera_plugin_api` for external tooling. Support formatter, linter, compiler plugins. | **Complete** | Plugin API tests. Sample plugin tests. | Plugin API spec | `chimera_plugin_api` |
| 68. | **Plugin System** | Rust | Implement `chimera_plugins` as plugin host. Handle lifecycle, configuration, communication. | **Complete** | Plugin loading tests. Lifecycle tests. | Plugin system design | `chimera_plugins` |

### Testing and Integration

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 69. | **Oracle Tests** | Rust | Implement differential tests against Elixir/BEAM reference behavior. Compare term representation, process semantics. | **Complete** | Oracle test suite. Differential test results. | Oracle test design | `chimera_oracle_tests` |
| 70. | **Integration: End-to-End** | Rust | Implement full compilation and execution pipeline tests: module -> HIR -> LRIR -> BEAM -> execute. | **Complete** | E2E pipeline tests. Full program execution tests. | Pipeline integration doc | `chimera_cli` |
| 71. | **Integration: Process Model** | Rust | Implement tests for spawn, send, receive, link, monitor, exit propagation. Supervision tree tests. | **Complete** | Process model integration tests. | Process integration doc | `chimera_core` |
| 72. | **Integration: Macro Expansion** | Rust | Implement tests for cross-module macro usage, hygiene, unquote. | **Complete** | Macro integration tests. | Macro integration doc | `chimera_expand` |
| 73. | **Integration: Pattern Matching** | Rust | Implement tests for case/function clause pattern matching with complex patterns. | **Complete** | Pattern matching integration tests. | Pattern integration doc | `chimera_core` |
| 74. | **Integration: Protocols** | Rust | Implement tests for protocol definition, implementation, dispatch. | **Complete** | Protocol integration tests. | Protocol integration doc | `chimera_core` |

### Documentation

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 75. | **Documentation: Stdlib** | Rust | Document all stdlib modules with function specs, types, examples. | **Complete** | Stdlib docs review. | Stdlib documentation | `chimera_stdlib` |
| 76. | **Documentation: Runtime** | Rust | Document process model, scheduler contract, fault boundaries, supervision design. | **Complete** | Runtime docs review. | Runtime documentation | `chimera_core` |
| 77. | **Documentation: Compiler Pipeline** | Rust | Document HIR, LRIR, code generation, BEAM target. | **Complete** | Compiler docs review. | Compiler documentation | `chimera_codegen`, `chimera_target` |
| 78. | **Documentation: Tooling** | Rust | Document CLI usage, REPL, formatter, LSP. | **Complete** | Tooling docs review. | Tooling documentation | `chimera_cli` |

### Performance and Release

| # | Task | Language | Implementation Requirements | Status | Tests | Docs | Crate |
|---|------|----------|---------------------------|--------|-------|------|-------|
| 79. | **Performance: Scheduler** | Rust | Benchmark scheduler throughput. Optimize reduction counting. | **Complete** | Scheduler benchmark. | Scheduler performance doc | `chimera_core` |
| 80. | **Performance: Term Representation** | Rust | Benchmark term allocation and garbage collection. | **Complete** | Term benchmark. | Term performance doc | `chimera_term`, `chimera_allocator` |
| 81. | **Validation: BEAM Compatibility** | Rust | Validate generated .beam files against Erlang/Elixir runtime. | **Complete** | Beam compatibility tests. | Compatibility validation doc | `chimera_target` |
| 82. | **Release: Version 1** | Rust | Package chimera-elixir as installable tool. Verify all exit criteria. | **Complete** | Release verification tests. | Release notes | `chimera_cli` |

---

## Concrete Crate Design

| Crate / Component | Language | Status | Concrete Responsibility | Dependency Rule |
|------------------|----------|--------|------------------------|------------------|
| `chimera_allocator` | Rust | **Complete** | Bump allocator, memory arena, GC primitives | No dependency on process, scheduler, VM, or Zig |
| `chimera_term` | Rust | **Complete** | Tagged terms, atom table, boxed layout descriptors, PID/ref/port terms, term comparison, term printing, ETF encode/decode | Depends on `chimera_allocator` only |
| `chimera_source` | Rust | **Complete** | Source file handling, source spans, file IDs | No dependencies |
| `chimera_diag` | Rust | **Complete** | Diagnostics, warnings, errors, source location mapping | Depends on `chimera_source` |
| `chimera_ast` | Rust | **Complete** | Quoted AST node definitions, AST builder, AST transformations | Depends on `chimera_term`, `chimera_source` |
| `chimera_ast_transform` | Rust | **Complete** | Transform trait, visitor pattern, sequential transform stages | Depends on `chimera_ast` |
| `chimera_ast_validate` | Rust | **Complete** | AST validation, hygiene checks | Depends on `chimera_ast` |
| `chimera_cst` | Rust | **Complete** | Concrete syntax tree, lossless CST representation | Depends on `chimera_lexer` |
| `chimera_lexer` | Rust | **Complete** | Tokenization, source location tracking, sigils, heredocs | Depends on `chimera_source` |
| `chimera_parser` | Rust | **Complete** | Expression parsing, module parsing, error recovery | Depends on `chimera_lexer`, `chimera_cst`, `chimera_ast` |
| `chimera_expand` | Rust | **Complete** | Macro expansion, quote/unquote, hygiene, MacroEnv | Depends on `chimera_ast`, `chimera_module`, `chimera_target` |
| `chimera_module` | Rust | **Complete** | Module environment, import/alias resolution, module builder | Depends on `chimera_ast`, `chimera_term` |
| `chimera_core` | Rust | **Complete** | HIR/LRIR definitions, pattern matching compiler, process model, mailbox, scheduler, links, monitors | Depends on `chimera_term`, `chimera_ast` |
| `chimera_typespec` | Rust | **Complete** | @type, @callback, @spec parsing and validation | Depends on `chimera_ast` |
| `chimera_lint` | Rust | **Complete** | Exhaustiveness checking, warnings | Depends on `chimera_core`, `chimera_diag` |
| `chimera_compile` | Rust | **Complete** | Module compilation orchestration | Depends on `chimera_expand`, `chimera_module`, `chimera_core` |
| `chimera_beam_term` | Rust | **Complete** | BEAM term encoding, IFF file format | Depends on `chimera_term` |
| `chimera_codegen` | Rust | **Complete** | BEAM instruction generation, code emission | Depends on `chimera_core`, `chimera_beam_term` |
| `chimera_target` | Rust | **Complete** | TargetRuntime trait, BEAM artifact emission, module loading | Depends on `chimera_compile`, `chimera_beam_term` |
| `chimera_stdlib` | Rust | **Complete** | Kernel, Enum, List, String, Tuple, Map, Process, Module, Code, Protocol, Exception, GenServer, Task, Supervisor | Depends on `chimera_core`, `chimera_term` |
| `chimera_build` | Rust | **Complete** | Mix-like build tool, project graph, dependency resolution | Depends on `chimera_compile` |
| `chimera_cli` | Rust | **Complete** | CLI entry point, compile/run/test/format/xref commands | Depends on all above |
| `chimera_fmt` | Rust | **Complete** | Code formatter from CST | Depends on `chimera_cst` |
| `chimera_lsp` | Rust | **Complete** | Language server protocol implementation | Depends on `chimera_*` |
| `chimera_xref` | Rust | **Complete** | Cross-reference analysis | Depends on `chimera_core`, `chimera_compile` |
| `chimera_oracle_tests` | Rust | **Complete** | Differential testing against Elixir/BEAM | Depends on `chimera_target` |
| `chimera_zig_abi` | Rust | **Complete** | C ABI structs, safe Rust wrappers around Zig kernels, FFI unsafe blocks | Only crate allowed to contain routine FFI unsafe |
| `chimera_wasm` | Rust | **Complete** | WASM compilation target | Depends on `chimera_core` |
| `chimera_plugin_api` | Rust | **Complete** | Plugin metadata, phase definitions, plugin trait | No dependencies |
| `chimera_plugins` | Rust | **Complete** | Plugin manager, lifecycle, configuration | Depends on `chimera_plugin_api` |
| `zig/chimera_kernels` | Zig | **Complete** | Bounded data-plane kernels: ETF, UTF-8, bitstring, scanner, source buffer, term helpers | Must not own VM state or retain Rust pointers |
