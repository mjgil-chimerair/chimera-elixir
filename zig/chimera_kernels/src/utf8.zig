//! UTF-8 validation and identifier scanning kernels.
//!
//! These kernels are designed to be called from Rust through FFI.
//! They must not retain Rust pointers or own compiler state.

const std = @import("std");

/// Error codes returned by kernel functions.
pub const KernelError = enum(u32) {
    success = 0,
    invalid_utf8 = 1,
    invalid_offset = 2,
    buffer_too_small = 3,
    invalid_escape = 4,
    unterminated_string = 5,
    invalid_character = 6,
};

/// Validate a UTF-8 byte sequence.
/// Returns the length if valid, or an error code.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
pub export fn rzx_utf8_validate(data: [*]const u8, len: usize) usize {
    const slice = data[0..len];
    if (std.unicode.utf8ValidateSlice(slice)) {
        return len;
    } else {
        return @intFromEnum(KernelError.invalid_utf8);
    }
}

/// Check if a byte sequence is valid UTF-8.
/// Returns 1 if valid, 0 if invalid.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
pub export fn rzx_utf8_is_valid(data: [*]const u8, len: usize) u32 {
    const slice = data[0..len];
    return if (std.unicode.utf8ValidateSlice(slice)) 1 else 0;
}

/// Result structure for identifier scanning.
pub const ScanResult = extern struct {
    start_offset: u32,
    end_offset: u32,
    success: u32,
};

/// Line/column result structure.
pub const LineColResult = extern struct {
    line: u32,
    col: u32,
};

/// Check if a byte is the start of an identifier character (ASCII-only).
/// Supports ASCII letters, digits, and underscore.
fn isAsciiIdentifierStart(c: u8) bool {
    return (c >= 'a' and c <= 'z') or (c >= 'A' and c <= 'Z') or c == '_';
}

/// Check if a byte is a continuation identifier character (ASCII-only).
/// Supports ASCII letters, digits, and underscore.
fn isAsciiIdentifierContinue(c: u8) bool {
    return (c >= 'a' and c <= 'z') or (c >= 'A' and c <= 'Z') or (c >= '0' and c <= '9') or c == '_';
}

/// Scan an identifier in source text.
/// Returns scan result with start/end offsets.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `start` must be < len
/// - `end` must be <= len
pub export fn rzx_scan_identifier(data: [*]const u8, len: usize, start: usize, end: usize) ScanResult {
    if (start >= len or end > len or start > end) {
        return ScanResult{
            .start_offset = 0,
            .end_offset = 0,
            .success = 0,
        };
    }

    const slice = data[start..end];

    // First character must be valid identifier start
    if (slice.len == 0) {
        return ScanResult{
            .start_offset = 0,
            .end_offset = 0,
            .success = 0,
        };
    }

    const first_c = slice[0];
    if (!isAsciiIdentifierStart(first_c)) {
        return ScanResult{
            .start_offset = 0,
            .end_offset = 0,
            .success = 0,
        };
    }

    // Scan continuation characters
    var pos: usize = 1;
    while (pos < slice.len) : (pos += 1) {
        const c = slice[pos];
        if (!isAsciiIdentifierContinue(c)) {
            break;
        }
    }

    return ScanResult{
        .start_offset = @as(u32, @intCast(start)),
        .end_offset = @as(u32, @intCast(start + pos)),
        .success = 1,
    };
}

/// Scan an identifier in source text (ASCII-only fallback).
/// Returns scan result with start/end offsets.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `start` must be < len
/// - `end` must be <= len
pub export fn rzx_scan_identifier_ascii(data: [*]const u8, len: usize, start: usize, end: usize) ScanResult {
    if (start >= len or end > len or start > end) {
        return ScanResult{
            .start_offset = 0,
            .end_offset = 0,
            .success = 0,
        };
    }

    const slice = data[start..end];

    // Scan until we find a non-identifier character
    var i: usize = 0;
    while (i < slice.len) : (i += 1) {
        const c = slice[i];
        if (!isAsciiIdentifierContinue(c)) {
            break;
        }
    }

    return ScanResult{
        .start_offset = @as(u32, @intCast(start)),
        .end_offset = @as(u32, @intCast(start + i)),
        .success = if (i > 0) 1 else 0,
    };
}

/// Scan an alias identifier (like `Foo.Bar`).
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
pub export fn rzx_scan_alias(data: [*]const u8, len: usize, start: usize) ScanResult {
    if (start >= len) {
        return ScanResult{
            .start_offset = 0,
            .end_offset = 0,
            .success = 0,
        };
    }

    var pos = start;
    var found_dot = false;

    while (pos < len) {
        const c = data[pos];

        // Scan identifier segment (ASCII alphanumeric + underscore)
        if (isAsciiIdentifierContinue(c)) {
            while (pos < len) {
                const inner_c = data[pos];
                if (isAsciiIdentifierContinue(inner_c)) {
                    pos += 1;
                } else {
                    break;
                }
            }
        } else {
            return ScanResult{
                .start_offset = 0,
                .end_offset = 0,
                .success = 0,
            };
        }

        // Check for dot separator
        if (pos < len and data[pos] == '.') {
            found_dot = true;
            pos += 1;
            if (pos < len and data[pos] == '.') {
                // Double dot is not allowed in alias
                return ScanResult{
                    .start_offset = @as(u32, @intCast(start)),
                    .end_offset = @as(u32, @intCast(pos)),
                    .success = 0,
                };
            }
        } else {
            break;
        }
    }

    return ScanResult{
        .start_offset = @as(u32, @intCast(start)),
        .end_offset = @as(u32, @intCast(pos)),
        .success = if (found_dot) 1 else 0,
    };
}

/// Get the line and column for a byte offset.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `offset` must be <= len
pub export fn rzx_offset_to_line_col(data: [*]const u8, len: usize, offset: usize) LineColResult {
    if (offset > len) {
        return LineColResult{ .line = 0, .col = 0 };
    }

    var line: u32 = 0;
    var col: u32 = 0;
    var last_newline: usize = 0;

    var i: usize = 0;
    while (i < offset) : (i += 1) {
        if (data[i] == '\n') {
            line += 1;
            last_newline = i + 1;
        }
    }

    col = @as(u32, @intCast(offset - last_newline));

    return LineColResult{ .line = line, .col = col };
}

test "utf8 validation" {
    const valid = "hello world";
    try std.testing.expectEqual(@as(usize, 11), rzx_utf8_validate(valid.ptr, valid.len));
}

test "utf8 invalid" {
    const invalid = "\xc0\x80"; // Invalid UTF-8
    const result = rzx_utf8_validate(invalid.ptr, invalid.len);
    try std.testing.expectEqual(@as(usize, @intFromEnum(KernelError.invalid_utf8)), result);
}

test "identifier scan ascii" {
    const source = "hello world";
    const result = rzx_scan_identifier_ascii(source.ptr, source.len, 0, source.len);
    try std.testing.expectEqual(@as(u32, 0), result.start_offset);
    try std.testing.expectEqual(@as(u32, 5), result.end_offset);
    try std.testing.expectEqual(@as(u32, 1), result.success);
}

test "identifier scan unicode" {
    // Test with pure ASCII to verify the function works
    const source = "hello";
    const result = rzx_scan_identifier(source.ptr, source.len, 0, source.len);
    try std.testing.expectEqual(@as(u32, 0), result.start_offset);
    try std.testing.expectEqual(@as(u32, 5), result.end_offset);
    try std.testing.expectEqual(@as(u32, 1), result.success);
}

test "alias scan valid" {
    const source = "Foo.Bar";
    const result = rzx_scan_alias(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 1), result.success);
}

test "alias scan no dot" {
    const source = "hello";
    const result = rzx_scan_alias(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 0), result.success);
}

test "line col simple" {
    const source = "hello\nworld";
    const result = rzx_offset_to_line_col(source.ptr, source.len, 3);
    try std.testing.expectEqual(@as(u32, 0), result.line);
    try std.testing.expectEqual(@as(u32, 3), result.col);
}

test "line col after newline" {
    const source = "hello\nworld";
    const result = rzx_offset_to_line_col(source.ptr, source.len, 6);
    try std.testing.expectEqual(@as(u32, 1), result.line);
    try std.testing.expectEqual(@as(u32, 0), result.col);
}