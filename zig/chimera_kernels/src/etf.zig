//! ETF (Erlang External Term Format) encoding/decoding kernels.
//!
//! These kernels handle term encoding for crossing the Rust/Zig boundary.
//! They must not retain Rust pointers or own compiler state.

const std = @import("std");

/// ETF term tags as defined in the Erlang external term format.
pub const ETF_TAG = enum(u8) {
    small_integer = 0x61,
    integer = 0x62,
    float = 0x62,
    atom = 0x71,
    small_atom = 0x72,
    nil = 0x6A,
    string = 0x64,
    cons = 0x68,
    small_tuple = 0x68,
    large_tuple = 0x69,
    map = 0x6D,
    small_bigint = 0x6F,
    large_bigint = 0x70,
    binary = 0x6C,
    small_atom_utf8 = 0x73,
    atom_utf8 = 0x74,
    fun = 0x67,
    new_fun = 0x6E,
    local_fun = 0x63,
    external_fun = 0x64,
    port = 0x71,
    pid = 0x71,
    reference = 0x71,
    new_reference = 0x71,
    bit_binary = 0x6A,
};

/// Result of ETF decoding.
pub const ETFResult = extern struct {
    bytes_consumed: u32,
    success: u32,
    error_code: u32,
};

/// Decode a small integer from ETF buffer.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
pub export fn rzx_etf_decode_small_int(buf: [*]const u8, len: usize) ETFResult {
    if (len < 5) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1, // UnexpectedEof
        };
    }

    if (buf[0] != @intFromEnum(ETF_TAG.small_integer)) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 2, // InvalidTag
        };
    }

    const value = std.mem.readInt(u32, buf[1..5], .big);
    return ETFResult{
        .bytes_consumed = 5,
        .success = 1,
        .error_code = 0,
    };
}

/// Decode an atom from ETF buffer.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
pub export fn rzx_etf_decode_atom(buf: [*]const u8, len: usize) ETFResult {
    if (len < 5) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1,
        };
    }

    const tag = buf[0];
    if (tag != @intFromEnum(ETF_TAG.atom) and tag != @intFromEnum(ETF_TAG.small_atom)) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 2,
        };
    }

    return ETFResult{
        .bytes_consumed = 5,
        .success = 1,
        .error_code = 0,
    };
}

/// Decode nil (empty list) from ETF buffer.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
pub export fn rzx_etf_decode_nil(buf: [*]const u8, len: usize) ETFResult {
    if (len < 1) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1,
        };
    }

    if (buf[0] != @intFromEnum(ETF_TAG.nil)) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 2,
        };
    }

    return ETFResult{
        .bytes_consumed = 1,
        .success = 1,
        .error_code = 0,
    };
}

/// Decode a cons cell from ETF buffer.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
pub export fn rzx_etf_decode_cons(buf: [*]const u8, len: usize) ETFResult {
    if (len < 1) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1,
        };
    }

    if (buf[0] != @intFromEnum(ETF_TAG.cons)) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 2,
        };
    }

    // Cons cell: tag + term + term
    // We just validate the tag here; actual decoding requires recursive handling
    return ETFResult{
        .bytes_consumed = 1,
        .success = 1,
        .error_code = 0,
    };
}

/// Decode a string from ETF buffer.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
pub export fn rzx_etf_decode_string(buf: [*]const u8, len: usize) ETFResult {
    if (len < 5) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1,
        };
    }

    if (buf[0] != @intFromEnum(ETF_TAG.string)) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 2,
        };
    }

    const str_len = std.mem.readInt(u32, buf[1..5], .big);
    const total_len = 5 + str_len;

    if (len < total_len) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1,
        };
    }

    return ETFResult{
        .bytes_consumed = @as(u32, @intCast(total_len)),
        .success = 1,
        .error_code = 0,
    };
}

/// Decode a binary from ETF buffer.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
pub export fn rzx_etf_decode_binary(buf: [*]const u8, len: usize) ETFResult {
    if (len < 5) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1,
        };
    }

    if (buf[0] != @intFromEnum(ETF_TAG.binary)) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 2,
        };
    }

    const bin_len = std.mem.readInt(u32, buf[1..5], .big);
    const total_len = 5 + bin_len + 1; // +1 for bits trailer

    if (len < total_len) {
        return ETFResult{
            .bytes_consumed = 0,
            .success = 0,
            .error_code = 1,
        };
    }

    return ETFResult{
        .bytes_consumed = @as(u32, @intCast(total_len)),
        .success = 1,
        .error_code = 0,
    };
}

/// Encode a nil term to ETF.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
pub export fn rzx_etf_encode_nil(buf: [*]u8, len: usize) u32 {
    if (len < 1) return 0;
    buf[0] = @intFromEnum(ETF_TAG.nil);
    return 1;
}

/// Encode a small integer term to ETF.
///
/// # Safety
/// - `buf` must point to valid memory of at least `len` bytes
/// - `value` must be in range 0-255
pub export fn rzx_etf_encode_small_int(buf: [*]u8, len: usize, value: u32) u32 {
    if (len < 5) return 0;
    buf[0] = @intFromEnum(ETF_TAG.small_integer);
    std.mem.writeInt(u32, buf[1..5], value, .big);
    return 5;
}

/// Get the ETF version byte.
pub export fn rzx_etf_version() u8 {
    return 0x83; // ETF format version
}

/// Calculate the encoded size of a term (simplified estimation).
///
/// # Safety
/// - `term_type` must be a valid ETF tag
pub export fn rzx_etf_estimate_size(term_type: u32) u32 {
    switch (term_type) {
        0 => return 5, // small_integer
        1 => return 9, // float
        2 => return 5, // atom
        3 => return 1, // nil
        else => return 0,
    }
}

test "etf version" {
    try std.testing.expectEqual(@as(u8, 0x83), rzx_etf_version());
}

test "etf encode nil" {
    var buf: [10]u8 = undefined;
    const len = rzx_etf_encode_nil(&buf, 10);
    try std.testing.expectEqual(@as(u32, 1), len);
    try std.testing.expectEqual(@as(u8, 0x6A), buf[0]);
}

test "etf encode small int" {
    var buf: [10]u8 = undefined;
    const len = rzx_etf_encode_small_int(&buf, 10, 42);
    try std.testing.expectEqual(@as(u32, 5), len);
    try std.testing.expectEqual(@as(u8, 0x61), buf[0]);
}

test "etf decode small int" {
    var buf: [10]u8 = undefined;
    buf[0] = 0x61; // small_integer tag
    buf[1] = 0x00;
    buf[2] = 0x00;
    buf[3] = 0x00;
    buf[4] = 0x2A; // 42 in big endian
    const result = rzx_etf_decode_small_int(&buf, 5);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, 5), result.bytes_consumed);
}

test "etf decode small int wrong tag" {
    var buf: [10]u8 = undefined;
    buf[0] = 0x62; // wrong tag (integer, not small_integer)
    buf[1] = 0x00;
    buf[2] = 0x00;
    buf[3] = 0x00;
    buf[4] = 0x2A;
    const result = rzx_etf_decode_small_int(&buf, 5);
    try std.testing.expectEqual(@as(u32, 0), result.success);
}

test "etf decode nil" {
    var buf: [10]u8 = undefined;
    buf[0] = 0x6A; // nil tag
    const result = rzx_etf_decode_nil(&buf, 1);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, 1), result.bytes_consumed);
}

test "etf decode cons" {
    var buf: [10]u8 = undefined;
    buf[0] = 0x68; // cons tag
    const result = rzx_etf_decode_cons(&buf, 1);
    try std.testing.expectEqual(@as(u32, 1), result.success);
}

test "etf decode string" {
    var buf: [10]u8 = undefined;
    buf[0] = 0x64; // string tag
    buf[1] = 0x00;
    buf[2] = 0x00;
    buf[3] = 0x00;
    buf[4] = 0x05; // 5 bytes of string data
    // Total: 5 (header) + 5 (data) = 10 bytes
    const result = rzx_etf_decode_string(&buf, 10);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, 10), result.bytes_consumed);
}

test "etf decode binary" {
    var buf: [11]u8 = undefined;
    buf[0] = 0x6C; // binary tag
    buf[1] = 0x00;
    buf[2] = 0x00;
    buf[3] = 0x00;
    buf[4] = 0x05; // 5 bytes of binary data
    // Total: 5 (header) + 5 (data) + 1 (bits trailer) = 11 bytes
    const result = rzx_etf_decode_binary(&buf, 11);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, 11), result.bytes_consumed);
}

test "etf estimate size" {
    try std.testing.expectEqual(@as(u32, 5), rzx_etf_estimate_size(0)); // small_integer
    try std.testing.expectEqual(@as(u32, 9), rzx_etf_estimate_size(1)); // float
    try std.testing.expectEqual(@as(u32, 5), rzx_etf_estimate_size(2)); // atom
    try std.testing.expectEqual(@as(u32, 1), rzx_etf_estimate_size(3)); // nil
    try std.testing.expectEqual(@as(u32, 0), rzx_etf_estimate_size(99)); // unknown
}