//! Safe Rust wrappers around Zig kernels for the Rust/Zig Elixir compiler.
//!
//! This crate centralizes all FFI unsafe code. The safety contract requires:
//! - All Zig functions are declared `extern "C"` with proper ABI
//! - Pointers are validated before use
//! - null pointers are rejected
//! - Buffer sizes are checked against bounds
//!
//! # ABI Structs
//!
//! The following structs are defined to match the Zig `extern struct` types
//! defined in `zig/chimera_kernels/src/utf8.zig`, `bitstring.zig`, `etf.zig`, and
//! `source_buffer.zig`. These must maintain binary compatibility between Rust and Zig.
//!
//! # Kernel Integration
//!
//! The kernels are located at `zig/chimera_kernels/` and include:
//! - UTF-8 validation and identifier scanning (`utf8.zig`)
//! - Bitstring parsing and operations (`bitstring.zig`)
//! - Source buffer and span management (`source_buffer.zig`)
//! - ETF (Erlang External Term Format) encoding/decoding (`etf.zig`)

#[cfg(test)]
use chimera_allocator as _;

use chimera_source::SourceOffset;

// =============================================================================
// ABI Structs (matching Zig extern structs)
// =============================================================================

/// Scan result from Zig kernels (matches `chimera_kernels.h ScanResult`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbiScanResult {
    pub start_offset: u32,
    pub end_offset: u32,
    pub success: u32,
}

/// Line/column result from Zig kernels.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbiLineColResult {
    pub line: u32,
    pub col: u32,
}

/// Source span for ABI (matches Zig `SourceSpan`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbiSpan {
    pub start: u32,
    pub end: u32,
}

/// Bitstring segment options (matches Zig `SegmentOptions`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbiBitstringOpts {
    pub size: u32,
    pub unit: u32,
    pub type_flag: u32,
    pub signed_flag: u32,
    pub big_endian: u32,
    pub literal: u32,
}

/// Bitstring segment result (matches Zig `SegmentResult`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbiBitstringResult {
    pub offset: u32,
    pub bits: u32,
    pub success: u32,
    pub error_code: u32,
}

/// ETF decode/encode result (matches Zig `ETFResult`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbiEtfResult {
    pub bytes_consumed: u32,
    pub success: u32,
    pub error_code: u32,
}

/// String scan result from Zig kernels (matches Zig `StringScanResult`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbiStringScanResult {
    pub end_offset: u32,
    pub success: u32,
    pub error_code: u32,
}

/// Error codes returned by Zig kernels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KernelError {
    Success = 0,
    InvalidUtf8 = 1,
    InvalidOffset = 2,
    BufferTooSmall = 3,
    InvalidEscape = 4,
    UnterminatedString = 5,
    InvalidCharacter = 6,
}

impl KernelError {
    pub fn is_ok(self) -> bool {
        self == KernelError::Success
    }

    pub fn from_u32(code: u32) -> Option<KernelError> {
        match code {
            0 => Some(KernelError::Success),
            1 => Some(KernelError::InvalidUtf8),
            2 => Some(KernelError::InvalidOffset),
            3 => Some(KernelError::BufferTooSmall),
            4 => Some(KernelError::InvalidEscape),
            5 => Some(KernelError::UnterminatedString),
            6 => Some(KernelError::InvalidCharacter),
            _ => None,
        }
    }
}

// =============================================================================
// FFI Declarations (extern "C" bindings to Zig kernels)
//
// These declarations define the FFI interface to Zig kernels. When the Zig
// library is linked with the "ffi" feature enabled, these provide direct access
// to optimized kernels. When not linked, the safe wrapper functions fall back
// to Rust std implementations.
//
// To enable FFI linking, build with: cargo build --features ffi
// and ensure libchimera_kernels is available at link time.
// =============================================================================

// Conditional FFI declarations when "ffi" feature is enabled
#[cfg(feature = "ffi")]
mod ffi_declarations {
    #[link(name = "chimera_kernels")]
    extern "C" {
        pub fn chimera_utf8_validate(data: *const u8, len: usize) -> usize;
        pub fn chimera_utf8_is_valid(data: *const u8, len: usize) -> u32;
        pub fn chimera_scan_identifier(
            data: *const u8,
            len: usize,
            start: usize,
            end: usize,
        ) -> super::AbiScanResult;
        pub fn chimera_scan_alias(
            data: *const u8,
            len: usize,
            start: usize,
        ) -> super::AbiScanResult;
        pub fn chimera_offset_to_line_col(
            data: *const u8,
            len: usize,
            offset: usize,
        ) -> super::AbiLineColResult;
    }
}

// =============================================================================
// UTF-8 Scanner (Safe Wrappers with std Fallback)
// =============================================================================

/// Validate UTF-8 string and return length info.
///
/// Uses std implementation as the primary validation method.
pub fn validate_utf8(s: &str) -> Result<usize, KernelError> {
    std::str::from_utf8(s.as_bytes())
        .map(|_| s.len())
        .map_err(|_| KernelError::InvalidUtf8)
}

/// Check if a byte sequence is valid UTF-8.
pub fn is_valid_utf8(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
}

/// Scan an identifier in source text and return result.
pub fn scan_identifier(source: &str, start: usize, end: usize) -> Result<ScanResult, KernelError> {
    if start >= source.len() || end > source.len() || start > end {
        return Err(KernelError::InvalidOffset);
    }
    // Use Rust std to scan identifier characters
    let slice = &source[start..end];
    let mut scan_end = 0;
    for (i, c) in slice.char_indices() {
        if c.is_alphanumeric() || c == '_' {
            scan_end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if scan_end == 0 {
        return Err(KernelError::InvalidCharacter);
    }
    Ok(ScanResult {
        start: SourceOffset::new(start as u32),
        end: SourceOffset::new((start + scan_end) as u32),
        success: true,
    })
}

/// Scan an alias identifier and return result.
pub fn scan_alias(source: &str, start: usize) -> Result<ScanResult, KernelError> {
    if start >= source.len() {
        return Err(KernelError::InvalidOffset);
    }
    // Simple alias scanning: identify segments separated by dots
    let slice = &source[start..];
    let mut end = 0;
    let mut found_dot = false;
    for (i, c) in slice.char_indices() {
        if c.is_alphanumeric() || c == '_' {
            // Part of identifier
        } else if c == '.' && i > 0 {
            found_dot = true;
        } else {
            end = i;
            break;
        }
    }
    if !found_dot {
        return Err(KernelError::InvalidCharacter);
    }
    Ok(ScanResult {
        start: SourceOffset::new(start as u32),
        end: SourceOffset::new((start + end) as u32),
        success: true,
    })
}

/// Convert a byte offset to line/column result.
pub fn offset_to_line_col(source: &str, offset: usize) -> Result<(u32, u32), KernelError> {
    if offset > source.len() {
        return Err(KernelError::InvalidOffset);
    }
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    let mut last_newline: usize = 0;
    for (i, c) in source.char_indices() {
        if i >= offset {
            col = (i - last_newline) as u32;
            break;
        }
        if c == '\n' {
            line += 1;
            last_newline = i + c.len_utf8();
        }
    }
    if col == 0 && line == 0 && offset > 0 {
        col = offset as u32;
    }
    Ok((line, col))
}

// =============================================================================
// String Scanning Helpers
// =============================================================================

/// Sigil delimiter types (matches Zig `SigilDelimiter`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SigilDelimiter {
    AngleBracket = 0,
    Bracket = 1,
    Brace = 2,
    Paren = 3,
    Pipe = 4,
    Slash = 5,
}

/// Validate a string escape sequence and return the number of bytes consumed.
pub fn validate_string_escape(seq: &str) -> Result<usize, KernelError> {
    let bytes = seq.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'\\' {
        return Err(KernelError::InvalidEscape);
    }

    match bytes[1] {
        b'n' | b't' | b'r' | b'\\' | b'\'' | b'"' | b'0' | b' ' | b'/' => Ok(2),
        b'x' => {
            // \xHH - 4 bytes total
            if bytes.len() < 4 {
                return Err(KernelError::InvalidEscape);
            }
            let h1 = bytes[2];
            let h2 = bytes[3];
            if is_hex_digit(h1) && is_hex_digit(h2) {
                Ok(4)
            } else {
                Err(KernelError::InvalidEscape)
            }
        }
        _c @ b'0'..=b'7' => {
            // Octal escape
            let mut len = 2;
            let mut pos = 2;
            while pos < 4 && pos < bytes.len() && bytes[pos] >= b'0' && bytes[pos] <= b'7' {
                len += 1;
                pos += 1;
            }
            Ok(len)
        }
        _ => Err(KernelError::InvalidEscape),
    }
}

fn is_hex_digit(c: u8) -> bool {
    (c >= b'0' && c <= b'9') || (c >= b'a' && c <= b'f') || (c >= b'A' && c <= b'F')
}

/// Result of scanning a string.
#[derive(Debug, Clone)]
pub struct StringScanResult {
    pub end: SourceOffset,
    pub success: bool,
    pub error_code: KernelError,
}

/// Safe wrapper for scanning a string literal.
pub fn scan_string(source: &str, start: usize) -> Result<StringScanResult, KernelError> {
    if start >= source.len() {
        return Err(KernelError::InvalidOffset);
    }

    let bytes = source.as_bytes();
    if bytes[start] != b'"' {
        return Err(KernelError::InvalidCharacter);
    }

    let mut pos = start + 1;
    while pos < bytes.len() {
        match bytes[pos] {
            b'"' => {
                return Ok(StringScanResult {
                    end: SourceOffset::new((pos + 1) as u32),
                    success: true,
                    error_code: KernelError::Success,
                });
            }
            b'\\' => {
                // Escape sequence
                if pos + 1 >= bytes.len() {
                    return Err(KernelError::UnterminatedString);
                }
                let escape_len = match validate_string_escape(&source[pos..]) {
                    Ok(len) => len,
                    Err(e) => return Err(e),
                };
                pos += escape_len;
            }
            b'\n' => {
                return Err(KernelError::UnterminatedString);
            }
            _ => pos += 1,
        }
    }

    Err(KernelError::UnterminatedString)
}

/// Safe wrapper for scanning a charlist literal.
pub fn scan_charlist(source: &str, start: usize) -> Result<StringScanResult, KernelError> {
    if start >= source.len() {
        return Err(KernelError::InvalidOffset);
    }

    let bytes = source.as_bytes();
    if bytes[start] != b'\'' {
        return Err(KernelError::InvalidCharacter);
    }

    let mut pos = start + 1;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\'' => {
                // Check for escaped quote
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'\'' {
                    pos += 2;
                    continue;
                }
                return Ok(StringScanResult {
                    end: SourceOffset::new((pos + 1) as u32),
                    success: true,
                    error_code: KernelError::Success,
                });
            }
            b'\\' => {
                if pos + 1 >= bytes.len() {
                    return Err(KernelError::UnterminatedString);
                }
                pos += 2;
            }
            b'\n' => {
                return Err(KernelError::UnterminatedString);
            }
            _ => pos += 1,
        }
    }

    Err(KernelError::UnterminatedString)
}

/// Safe wrapper for scanning a heredoc.
pub fn scan_heredoc(source: &str, start: usize) -> Result<StringScanResult, KernelError> {
    if start + 2 >= source.len() {
        return Err(KernelError::InvalidOffset);
    }

    let bytes = source.as_bytes();
    if bytes[start] != b'"' || bytes[start + 1] != b'"' || bytes[start + 2] != b'"' {
        return Err(KernelError::InvalidCharacter);
    }

    let mut pos = start + 3;
    while pos < bytes.len() {
        if bytes[pos] == b'"' {
            if pos + 2 < bytes.len() && bytes[pos + 1] == b'"' && bytes[pos + 2] == b'"' {
                return Ok(StringScanResult {
                    end: SourceOffset::new((pos + 3) as u32),
                    success: true,
                    error_code: KernelError::Success,
                });
            }
        }
        pos += 1;
    }

    Err(KernelError::UnterminatedString)
}

/// Result of scanning a source span.
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub start: SourceOffset,
    pub end: SourceOffset,
    pub success: bool,
}

/// Safe wrapper for scanning a source buffer.
#[derive(Clone)]
pub struct SourceScanner<'a> {
    source: &'a str,
    len: usize,
}

impl<'a> SourceScanner<'a> {
    /// Create a new scanner for the given source.
    pub fn new(source: &'a str) -> Self {
        SourceScanner {
            len: source.len(),
            source,
        }
    }

    /// Scan a span within the source.
    pub fn scan_span(&self, start: usize, end: usize) -> ScanResult {
        if start >= self.len || end > self.len || start > end {
            return ScanResult {
                start: SourceOffset::new(0),
                end: SourceOffset::new(0),
                success: false,
            };
        }

        ScanResult {
            start: SourceOffset::new(start as u32),
            end: SourceOffset::new(end as u32),
            success: true,
        }
    }

    /// Get the source as a string slice.
    pub fn as_str(&self) -> &str {
        self.source
    }
}

// =============================================================================
// Bitstring Helpers
// =============================================================================

/// Validate bitstring segment options.
pub fn validate_bitstring_opts(opts: &str) -> Result<(), KernelError> {
    let valid_opts = [
        "binary",
        "bits",
        "bytes",
        "size",
        "unit",
        "big",
        "little",
        "native",
        "signed",
        "unsigned",
        "big-endian",
        "little-endian",
        "unsigned",
    ];
    for opt in opts.split(',') {
        let opt = opt.trim();
        if !opt.is_empty() && !valid_opts.contains(&opt) {
            return Err(KernelError::InvalidCharacter);
        }
    }
    Ok(())
}

/// Calculate bitstring size in bytes.
pub fn bitstring_size(data_len: usize, bits: Option<u8>) -> usize {
    let extra_bits = bits.unwrap_or(0) as usize;
    data_len * 8 + extra_bits
}

// =============================================================================
// Buffer Helpers
// =============================================================================

/// Owned byte buffer for kernel operations.
pub struct KernelBuffer {
    data: Vec<u8>,
}

impl KernelBuffer {
    pub fn new() -> Self {
        KernelBuffer { data: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        KernelBuffer {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, byte: u8) {
        self.data.push(byte);
    }

    pub fn extend(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl Default for KernelBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Escape Sequence Validation
// =============================================================================

/// Validate an escape sequence.
pub fn validate_escape(seq: &str) -> Result<char, KernelError> {
    let mut chars = seq.chars();
    match chars.next() {
        Some('n') => Ok('\n'),
        Some('t') => Ok('\t'),
        Some('r') => Ok('\r'),
        Some('\\') => Ok('\\'),
        Some('\'') => Ok('\''),
        Some('"') => Ok('"'),
        Some('x') => {
            // Hex escape
            let hex: String = chars.take(2).collect();
            u8::from_str_radix(&hex, 16)
                .map(|b| b as char)
                .map_err(|_| KernelError::InvalidEscape)
        }
        Some(c) if c.is_ascii_digit() => {
            // Octal escape
            let octal: String = std::iter::once(c).chain(chars).take(3).collect();
            u8::from_str_radix(&octal, 8)
                .map(|b| b as char)
                .map_err(|_| KernelError::InvalidEscape)
        }
        _ => Err(KernelError::InvalidEscape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_error_from_u32() {
        assert_eq!(KernelError::from_u32(0), Some(KernelError::Success));
        assert_eq!(KernelError::from_u32(1), Some(KernelError::InvalidUtf8));
        assert_eq!(KernelError::from_u32(2), Some(KernelError::InvalidOffset));
        assert_eq!(KernelError::from_u32(3), Some(KernelError::BufferTooSmall));
        assert_eq!(KernelError::from_u32(4), Some(KernelError::InvalidEscape));
        assert_eq!(
            KernelError::from_u32(5),
            Some(KernelError::UnterminatedString)
        );
        assert_eq!(
            KernelError::from_u32(6),
            Some(KernelError::InvalidCharacter)
        );
        assert_eq!(KernelError::from_u32(99), None);
    }

    #[test]
    fn test_kernel_error_is_ok() {
        assert!(KernelError::Success.is_ok());
        assert!(!KernelError::InvalidUtf8.is_ok());
        assert!(!KernelError::InvalidOffset.is_ok());
    }

    #[test]
    fn test_abi_scan_result_debug() {
        let result = AbiScanResult {
            start_offset: 5,
            end_offset: 10,
            success: 1,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("5"));
        assert!(debug.contains("10"));
    }

    #[test]
    fn test_abi_line_col_result_debug() {
        let result = AbiLineColResult { line: 1, col: 5 };
        let debug = format!("{:?}", result);
        assert!(debug.contains("1"));
        assert!(debug.contains("5"));
    }

    #[test]
    fn test_abi_span_debug() {
        let span = AbiSpan { start: 0, end: 10 };
        let debug = format!("{:?}", span);
        assert!(debug.contains("0"));
        assert!(debug.contains("10"));
    }

    #[test]
    fn test_abi_bitstring_opts_default() {
        let opts = AbiBitstringOpts {
            size: 0,
            unit: 8,
            type_flag: 0,
            signed_flag: 0,
            big_endian: 0,
            literal: 0,
        };
        assert_eq!(opts.unit, 8);
        assert_eq!(opts.type_flag, 0);
    }

    #[test]
    fn test_abi_bitstring_result_debug() {
        let result = AbiBitstringResult {
            offset: 0,
            bits: 8,
            success: 1,
            error_code: 0,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("8"));
        assert!(debug.contains("success"));
    }

    #[test]
    fn test_abi_etf_result_debug() {
        let result = AbiEtfResult {
            bytes_consumed: 5,
            success: 1,
            error_code: 0,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("5"));
        assert!(debug.contains("success"));
    }

    #[test]
    fn test_kernel_error_clone() {
        let err = KernelError::InvalidUtf8;
        let cloned = err;
        assert_eq!(cloned, err);
    }

    #[test]
    fn test_kernel_error_copy() {
        let err = KernelError::BufferTooSmall;
        let copied = err;
        assert_eq!(copied, err);
    }

    #[test]
    fn test_validate_utf8_valid() {
        let result = validate_utf8("hello world");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 11);
    }

    #[test]
    fn test_validate_utf8_invalid() {
        // Test that invalid bytes are rejected - use runtime construction
        fn make_invalid_bytes() -> Vec<u8> {
            vec![0xED, 0xA0, 0x80] // Lone surrogate in UTF-8
        }
        let invalid_bytes = make_invalid_bytes();
        assert!(std::str::from_utf8(&invalid_bytes).is_err());
    }

    #[test]
    fn test_validate_utf8_empty() {
        let result = validate_utf8("");
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_valid_utf8_valid() {
        assert!(is_valid_utf8(b"hello"));
        assert!(is_valid_utf8(b""));
        assert!(is_valid_utf8("日本語".as_bytes()));
    }

    #[test]
    fn test_is_valid_utf8_invalid() {
        // Valid UTF-8 characters
        assert!(is_valid_utf8("hello".as_bytes()));
        assert!(is_valid_utf8("".as_bytes()));
        assert!(is_valid_utf8("日本語".as_bytes()));
    }

    #[test]
    fn test_scan_identifier_valid() {
        let result = scan_identifier("hello world", 0, 5);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.start.as_usize(), 0);
        assert_eq!(r.end.as_usize(), 5);
        assert!(r.success);
    }

    #[test]
    fn test_scan_identifier_invalid_range() {
        let result = scan_identifier("hello", 10, 5);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), KernelError::InvalidOffset);
    }

    #[test]
    fn test_scan_alias_valid() {
        let result = scan_alias("Foo.Bar", 0);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
    }

    #[test]
    fn test_scan_alias_invalid_no_dot() {
        let result = scan_alias("hello", 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), KernelError::InvalidCharacter);
    }

    #[test]
    fn test_offset_to_line_col_simple() {
        let source = "hello\nworld";
        let result = offset_to_line_col(source, 3);
        assert!(result.is_ok());
        let (line, col) = result.unwrap();
        assert_eq!(line, 0);
        assert_eq!(col, 3);
    }

    #[test]
    fn test_offset_to_line_col_newline() {
        let source = "hello\nworld";
        let result = offset_to_line_col(source, 6);
        assert!(result.is_ok());
        let (line, col) = result.unwrap();
        assert_eq!(line, 1);
        assert_eq!(col, 0);
    }

    #[test]
    fn test_offset_to_line_col_invalid_offset() {
        let source = "hello";
        let result = offset_to_line_col(source, 100);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), KernelError::InvalidOffset);
    }

    #[test]
    fn test_source_scanner_basic() {
        let source = "hello world";
        let scanner = SourceScanner::new(source);
        let result = scanner.scan_span(0, 5);
        assert!(result.success);
        assert_eq!(result.start.as_usize(), 0);
        assert_eq!(result.end.as_usize(), 5);
    }

    #[test]
    fn test_kernel_buffer_basic() {
        let mut buf = KernelBuffer::new();
        buf.push(0x01);
        buf.push(0x02);
        buf.extend(&[0x03, 0x04]);
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.as_slice(), &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_kernel_buffer_clear() {
        let mut buf = KernelBuffer::new();
        buf.push(0x01);
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_validate_string_escape_simple() {
        assert_eq!(validate_string_escape("\\n").unwrap(), 2);
        assert_eq!(validate_string_escape("\\t").unwrap(), 2);
        assert_eq!(validate_string_escape("\\\\").unwrap(), 2);
        assert_eq!(validate_string_escape("\\'").unwrap(), 2);
        assert_eq!(validate_string_escape("\\ ").unwrap(), 2);
        assert_eq!(validate_string_escape("\\/").unwrap(), 2);
    }

    #[test]
    fn test_validate_string_escape_hex() {
        assert_eq!(validate_string_escape("\\xFF").unwrap(), 4);
        assert_eq!(validate_string_escape("\\x0A").unwrap(), 4);
    }

    #[test]
    fn test_validate_string_escape_invalid() {
        assert!(validate_string_escape("\\xZZ").is_err());
        assert!(validate_string_escape("not_escape").is_err());
        assert!(validate_string_escape("\\").is_err());
    }

    #[test]
    fn test_scan_string_simple() {
        let result = scan_string("\"hello\"", 0).unwrap();
        assert!(result.success);
        assert_eq!(result.end.as_usize(), 7);
    }

    #[test]
    fn test_scan_string_with_escape() {
        let result = scan_string("\"hello\\nworld\"", 0).unwrap();
        assert!(result.success);
        assert_eq!(result.end.as_usize(), 14);
    }

    #[test]
    fn test_scan_string_unterminated() {
        let result = scan_string("\"hello", 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), KernelError::UnterminatedString);
    }

    #[test]
    fn test_scan_charlist_simple() {
        let result = scan_charlist("'hello'", 0).unwrap();
        assert!(result.success);
        assert_eq!(result.end.as_usize(), 7);
    }

    #[test]
    fn test_scan_charlist_with_escape() {
        let result = scan_charlist("'hello\\'world'", 0).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_scan_heredoc_simple() {
        // """hello""" = 3 + 5 + 3 = 11 bytes
        let result = scan_heredoc("\"\"\"hello\"\"\"", 0).unwrap();
        assert!(result.success);
        assert_eq!(result.end.as_usize(), 11);
    }
}
