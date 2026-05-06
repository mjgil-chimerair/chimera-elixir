# Zig Kernel Library (`rzx_kernels`)

Low-level kernels for the Rust/Zig Elixir compiler. These kernels are designed to be called from Rust through FFI and must follow strict safety rules.

## Safety Rules

1. **No Rust pointers retained** - Kernels must not hold references to Rust-managed memory
2. **No compiler state modified** - Kernels operate only on provided input data
3. **Bounded buffer operations** - All operations respect buffer boundaries
4. **No dynamic allocation** - Kernels use fixed-size data structures

## Project Structure

```
zig/rzx_kernels/
├── build.zig          # Zig build configuration
├── build.zig.zon      # Package manifest
└── src/
    ├── rzx_kernels.h  # C ABI header
    ├── utf8.zig       # UTF-8 validation and identifier scanning
    ├── bitstring.zig   # Bitstring parsing and operations
    ├── source_buffer.zig # Source span and buffer operations
    └── etf.zig         # Erlang External Term Format encoding/decoding
```

## Kernel Modules

### UTF-8 Scanner (`utf8.zig`)
- `rzx_utf8_validate` - Validate UTF-8 byte sequence
- `rzx_utf8_is_valid` - Check if bytes are valid UTF-8
- `rzx_scan_identifier` - Scan identifier in source text
- `rzx_scan_alias` - Scan alias identifier (e.g., `Foo.Bar`)
- `rzx_offset_to_line_col` - Convert byte offset to line/column

### Bitstring Operations (`bitstring.zig`)
- `rzx_bitstring_parse_segment` - Parse bitstring segment
- `rzx_bitstring_calculate_size` - Calculate total bitstring size
- `rzx_bitstring_validate_opts` - Validate segment options

### Source Buffer (`source_buffer.zig`)
- `rzx_span_create` - Create a source span
- `rzx_span_is_valid` - Check if span is valid
- `rzx_span_merge` - Merge adjacent spans
- `rzx_span_text` - Extract text from span
- `rzx_find_next_newline` - Find next newline
- `rzx_find_prev_newline` - Find previous newline
- `rzx_count_newlines` - Count newlines up to offset
- `rzx_line_offset` - Get byte offset for line number

### ETF Encoding (`etf.zig`)
- `rzx_etf_decode_small_int` - Decode small integer
- `rzx_etf_decode_atom` - Decode atom
- `rzx_etf_decode_nil` - Decode nil
- `rzx_etf_decode_cons` - Decode cons cell
- `rzx_etf_decode_string` - Decode string
- `rzx_etf_decode_binary` - Decode binary
- `rzx_etf_encode_nil` - Encode nil
- `rzx_etf_encode_small_int` - Encode small integer
- `rzx_etf_version` - Get ETF version byte
- `rzx_etf_estimate_size` - Estimate encoded size

## Building

```bash
cd zig/rzx_kernels
zig build
zig build test
```

## Integration with Rust

The `chimera_zig_abi` crate provides safe Rust wrappers around these kernels.

```toml
[dependencies]
chimera_zig_abi = { path = "crates/chimera_zig_abi" }
```

## Error Codes

All kernels return error codes via specific return types:

| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | Operation succeeded |
| 1 | INVALID_UTF8 | Invalid UTF-8 sequence |
| 2 | INVALID_OFFSET | Offset out of bounds |
| 3 | BUFFER_TOO_SMALL | Output buffer insufficient |
| 4 | INVALID_ESCAPE | Invalid escape sequence |
| 5 | UNTERMINATED_STRING | Unterminated string literal |
| 6 | INVALID_CHARACTER | Invalid character in context |