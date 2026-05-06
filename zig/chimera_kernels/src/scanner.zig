//! String, heredoc, and sigil scanning kernels.
//!
//! These kernels handle string literal scanning with escape sequences,
//! heredoc scanning, and sigil delimiter parsing.

const std = @import("std");
const utf8 = @import("utf8.zig");

/// Error codes for string operations.
pub const StringError = enum(u32) {
    success = 0,
    invalid_utf8 = 1,
    invalid_offset = 2,
    buffer_too_small = 3,
    invalid_escape = 4,
    unterminated_string = 5,
    invalid_delimiter = 6,
};

/// Scan result for string scanning.
pub const StringScanResult = extern struct {
    end_offset: u32,
    success: u32,
    error_code: u32,
};

/// Sigil delimiter types.
pub const SigilDelimiter = enum(u8) {
    angle_bracket = 0,  // < >
    bracket = 1,        // [ ]
    brace = 2,         // { }
    paren = 3,          // ( )
    pipe = 4,           // | |
    slash = 5,          // / /
};

/// Scan a double-quoted string with escape sequences.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `start` must be < len
pub export fn rzx_scan_string(data: [*]const u8, len: usize, start: usize) StringScanResult {
    if (start >= len) {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_offset),
        };
    }

    // Check for opening quote
    if (data[start] != '"') {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_delimiter),
        };
    }

    var pos = start + 1;
    while (pos < len) : (pos += 1) {
        const c = data[pos];

        if (c == '"') {
            // End of string
            return StringScanResult{
                .end_offset = @as(u32, @intCast(pos + 1)),
                .success = 1,
                .error_code = 0,
            };
        }

        if (c == '\\') {
            // Escape sequence
            if (pos + 1 >= len) {
                return StringScanResult{
                    .end_offset = @as(u32, @intCast(pos)),
                    .success = 0,
                    .error_code = @intFromEnum(StringError.unterminated_string),
                };
            }

            const next = data[pos + 1];
            pos += 1;

            switch (next) {
                'n', 't', 'r', '\\', '\'', '"', '0', ' ', '/' => {},
                'x' => {
                    // Hex escape: \xHH (2 hex digits)
                    if (pos + 2 >= len) {
                        return StringScanResult{
                            .end_offset = @as(u32, @intCast(pos)),
                            .success = 0,
                            .error_code = @intFromEnum(StringError.invalid_escape),
                        };
                    }
                    const h1 = data[pos + 1];
                    const h2 = data[pos + 2];
                    if (!isHexDigit(h1) or !isHexDigit(h2)) {
                        return StringScanResult{
                            .end_offset = @as(u32, @intCast(pos)),
                            .success = 0,
                            .error_code = @intFromEnum(StringError.invalid_escape),
                        };
                    }
                    pos += 2;
                },
                else => {
                    // Check for octal escape: \NNN or \NN
                    if (next >= '0' and next <= '7') {
                        var octal_digits: usize = 1;
                        var check_pos = pos + 1;
                        while (check_pos < pos + 3 and check_pos < len) : (check_pos += 1) {
                            const d = data[check_pos];
                            if (d >= '0' and d <= '7') {
                                octal_digits += 1;
                            } else {
                                break;
                            }
                        }
                        // Octal escapes are at most 3 digits
                        if (octal_digits > 3) octal_digits = 3;
                        pos += octal_digits - 1; // -1 because loop will +1
                    } else {
                        return StringScanResult{
                            .end_offset = @as(u32, @intCast(pos)),
                            .success = 0,
                            .error_code = @intFromEnum(StringError.invalid_escape),
                        };
                    }
                },
            }
        } else if (c == '\n') {
            // Unterminated newline in string
            return StringScanResult{
                .end_offset = @as(u32, @intCast(pos)),
                .success = 0,
                .error_code = @intFromEnum(StringError.unterminated_string),
            };
        }
    }

    // Unterminated string
    return StringScanResult{
        .end_offset = @as(u32, @intCast(pos)),
        .success = 0,
        .error_code = @intFromEnum(StringError.unterminated_string),
    };
}

/// Scan a single-quoted string (charlist in Elixir).
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `start` must be < len
pub export fn rzx_scan_charlist(data: [*]const u8, len: usize, start: usize) StringScanResult {
    if (start >= len) {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_offset),
        };
    }

    // Check for opening quote
    if (data[start] != '\'') {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_delimiter),
        };
    }

    var pos = start + 1;
    while (pos < len) : (pos += 1) {
        const c = data[pos];

        if (c == '\'') {
            // End of charlist (but check for escapes)
            if (pos + 1 < len and data[pos + 1] == '\'') {
                // Escaped single quote - continue scanning
                pos += 1;
                continue;
            }
            return StringScanResult{
                .end_offset = @as(u32, @intCast(pos + 1)),
                .success = 1,
                .error_code = 0,
            };
        }

        if (c == '\\') {
            // Escape sequence
            if (pos + 1 >= len) {
                return StringScanResult{
                    .end_offset = @as(u32, @intCast(pos)),
                    .success = 0,
                    .error_code = @intFromEnum(StringError.unterminated_string),
                };
            }
            pos += 1; // Skip escaped character
        } else if (c == '\n') {
            // Unterminated newline
            return StringScanResult{
                .end_offset = @as(u32, @intCast(pos)),
                .success = 0,
                .error_code = @intFromEnum(StringError.unterminated_string),
            };
        }
    }

    return StringScanResult{
        .end_offset = @as(u32, @intCast(pos)),
        .success = 0,
        .error_code = @intFromEnum(StringError.unterminated_string),
    };
}

/// Scan a heredoc string (triple double quotes).
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `start` must be < len
pub export fn rzx_scan_heredoc(data: [*]const u8, len: usize, start: usize) StringScanResult {
    if (start + 2 >= len) {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_offset),
        };
    }

    // Check for opening """
    if (data[start] != '"' or data[start + 1] != '"' or data[start + 2] != '"') {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_delimiter),
        };
    }

    var pos = start + 3;

    // Scan until we find closing """
    while (pos < len) : (pos += 1) {
        if (data[pos] == '"') {
            if (pos + 2 < len and data[pos + 1] == '"' and data[pos + 2] == '"') {
                // Found closing """
                return StringScanResult{
                    .end_offset = @as(u32, @intCast(pos + 3)),
                    .success = 1,
                    .error_code = 0,
                };
            }
        }
    }

    return StringScanResult{
        .end_offset = @as(u32, @intCast(pos)),
        .success = 0,
        .error_code = @intFromEnum(StringError.unterminated_string),
    };
}

/// Find the matching closing sigil delimiter.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `start` must be < len
pub export fn rzx_scan_sigil(
    data: [*]const u8,
    len: usize,
    start: usize,
    delimiter: SigilDelimiter,
) StringScanResult {
    if (start >= len) {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_offset),
        };
    }

    const open_delim = getSigilOpenChar(delimiter);
    const close_delim = getSigilCloseChar(delimiter);

    // Check for opening delimiter
    if (data[start] != open_delim) {
        return StringScanResult{
            .end_offset = 0,
            .success = 0,
            .error_code = @intFromEnum(StringError.invalid_delimiter),
        };
    }

    var pos = start + 1;
    var depth: u32 = 1;

    while (pos < len) : (pos += 1) {
        const c = data[pos];

        if (c == close_delim) {
            depth -= 1;
            if (depth == 0) {
                return StringScanResult{
                    .end_offset = @as(u32, @intCast(pos + 1)),
                    .success = 1,
                    .error_code = 0,
                };
            }
        } else if (c == open_delim) {
            depth += 1;
        } else if (c == '\\') {
            // Skip escaped character
            if (pos + 1 < len) {
                pos += 1;
            }
        } else if (c == '\n' and close_delim == '|') {
            // Sigils with | delimiter can't span lines
            return StringScanResult{
                .end_offset = @as(u32, @intCast(pos)),
                .success = 0,
                .error_code = @intFromEnum(StringError.unterminated_string),
            };
        }
    }

    return StringScanResult{
        .end_offset = @as(u32, @intCast(pos)),
        .success = 0,
        .error_code = @intFromEnum(StringError.unterminated_string),
    };
}

/// Validate an escape sequence and return the number of bytes consumed.
pub export fn rzx_validate_escape(data: [*]const u8, len: usize, start: usize) u32 {
    if (start >= len) return 0;

    if (data[start] != '\\') return 0;

    if (start + 1 >= len) return 0;

    const next = data[start + 1];

    switch (next) {
        'n', 't', 'r', '\\', '\'', '"', '0', ' ', '/' => return 2,
        'x' => {
            // \xHH - 4 bytes total
            if (start + 3 >= len) return 0;
            const h1 = data[start + 2];
            const h2 = data[start + 3];
            if (isHexDigit(h1) and isHexDigit(h2)) return 4;
            return 0;
        },
        else => {
            // Octal escape
            if (next >= '0' and next <= '7') {
                var octal_len: u32 = 2;
                var check_pos = start + 2;
                while (check_pos < start + 4 and check_pos < len) : (check_pos += 1) {
                    const d = data[check_pos];
                    if (d >= '0' and d <= '7') {
                        octal_len += 1;
                    } else {
                        break;
                    }
                }
                return octal_len;
            }
            return 0;
        },
    }
}

// =============================================================================
// Helper functions
// =============================================================================

fn isHexDigit(c: u8) bool {
    return (c >= '0' and c <= '9') or
           (c >= 'a' and c <= 'f') or
           (c >= 'A' and c <= 'F');
}

fn getSigilOpenChar(delimiter: SigilDelimiter) u8 {
    return switch (delimiter) {
        .angle_bracket => '<',
        .bracket => '[',
        .brace => '{',
        .paren => '(',
        .pipe => '|',
        .slash => '/',
    };
}

fn getSigilCloseChar(delimiter: SigilDelimiter) u8 {
    return switch (delimiter) {
        .angle_bracket => '>',
        .bracket => ']',
        .brace => '}',
        .paren => ')',
        .pipe => '|',
        .slash => '/',
    };
}

// =============================================================================
// Tests
// =============================================================================

test "scan simple string" {
    const source = "\"hello\"";
    const result = rzx_scan_string(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, source.len), result.end_offset);
}

test "scan string with escapes" {
    const source = "\"hello\\nworld\"";
    const result = rzx_scan_string(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, source.len), result.end_offset);
}

test "scan string unterminated" {
    const source = "\"hello";
    const result = rzx_scan_string(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 0), result.success);
    try std.testing.expectEqual(@as(u32, @intFromEnum(StringError.unterminated_string)), result.error_code);
}

test "scan charlist" {
    const source = "'hello'";
    const result = rzx_scan_charlist(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, source.len), result.end_offset);
}

test "scan heredoc" {
    const source = "\"\"\"hello\"\"\"";
    const result = rzx_scan_heredoc(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, source.len), result.end_offset);
}

test "scan sigil with pipe" {
    const source = "|hello|";
    const result = rzx_scan_sigil(source.ptr, source.len, 0, .pipe);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, source.len), result.end_offset);
}

test "scan sigil with brackets" {
    const source = "[hello]";
    const result = rzx_scan_sigil(source.ptr, source.len, 0, .bracket);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, source.len), result.end_offset);
}

test "validate escape simple" {
    const source = "\\n";
    const result = rzx_validate_escape(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 2), result);
}

test "validate escape hex" {
    const source = "\\xFF";
    const result = rzx_validate_escape(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 4), result);
}

test "validate escape octal" {
    const source = "\\377";
    const result = rzx_validate_escape(source.ptr, source.len, 0);
    try std.testing.expectEqual(@as(u32, 4), result);
}