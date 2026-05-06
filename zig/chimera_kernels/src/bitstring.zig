//! Bitstring operation kernels.
//!
//! These kernels handle bit-level parsing, slicing, and construction.
//! They must not retain Rust pointers or own compiler state.

const std = @import("std");

/// Error codes for bitstring operations.
pub const BitstringError = enum(u32) {
    success = 0,
    invalid_size = 1,
    alignment_error = 2,
    invalid_segment = 3,
};

/// Segment options for bitstring parsing.
pub const SegmentOptions = extern struct {
    size: u32,        // Size in bits (0 = variable)
    unit: u32,        // Unit size (default 8)
    type_flag: u32,   // 0 = integer, 1 = float, 2 = binary
    signed: u32,      // 0 = unsigned, 1 = signed
    big_endian: u32,  // 0 = little, 1 = big, 2 = native
    literal: u32,     // 0 = evaluated, 1 = literal
};

/// Result of bitstring segment parsing.
pub const SegmentResult = extern struct {
    offset: u32,      // Byte offset where segment starts
    bits: u32,        // Total bits consumed
    success: u32,     // 1 = success, 0 = failure
    error_code: u32,  // Error code if failed
};

/// Parse a bitstring segment from a buffer.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `offset` must be < len
pub export fn rzx_bitstring_parse_segment(
    data: [*]const u8,
    len: usize,
    offset: usize,
    opts: SegmentOptions,
) SegmentResult {
    if (offset >= len) {
        return SegmentResult{
            .offset = 0,
            .bits = 0,
            .success = 0,
            .error_code = @intFromEnum(BitstringError.invalid_offset),
        };
    }

    const default_unit = 8;
    const unit = if (opts.unit == 0) default_unit else opts.unit;

    var byte_offset = offset;
    var bits_consumed: u32 = 0;

    // Calculate bits based on type
    switch (opts.type_flag) {
        0 => { // integer
            const size = if (opts.size == 0) 8 else opts.size;
            bits_consumed = size;

            // Validate alignment
            if (size % 8 != 0) {
                // Bit-level integer
                const byte_bits = (byte_offset * 8) + bits_consumed;
                _ = byte_bits;
            }
        },
        1 => { // float
            const size = if (opts.size == 0) 64 else opts.size;
            bits_consumed = size;
        },
        2 => { // binary
            if (opts.size == 0) {
                // Variable size - consume to end of buffer
                bits_consumed = @as(u32, @intCast((len - byte_offset) * 8));
            } else {
                bits_consumed = opts.size;
            }
        },
        else => {
            return SegmentResult{
                .offset = @as(u32, @intCast(offset)),
                .bits = 0,
                .success = 0,
                .error_code = @intFromEnum(BitstringError.invalid_segment),
            };
        }
    }

    return SegmentResult{
        .offset = @as(u32, @intCast(byte_offset)),
        .bits = bits_consumed,
        .success = 1,
        .error_code = 0,
    };
}

/// Calculate total bitstring size in bits.
///
/// # Safety
/// - `segment_count` must not overflow
pub export fn rzx_bitstring_calculate_size(
    segment_sizes: [*]const u32,
    segment_count: usize,
) u64 {
    var total: u64 = 0;
    var i: usize = 0;
    while (i < segment_count) : (i += 1) {
        total += segment_sizes[i];
    }
    return total;
}

/// Validate bitstring options.
///
/// # Safety
/// - `opts_str` must be a null-terminated string
pub export fn rzx_bitstring_validate_opts(opts_str: [*]const u8) u32 {
    const valid_opts = [_][]const u8{
        "binary", "bits", "bytes", "size", "unit",
        "big", "little", "native", "signed", "unsigned",
        "big-endian", "little-endian", "unsigned",
    };

    // Scan for null terminator
    var len: usize = 0;
    while (opts_str[len] != 0) : (len += 1) {}

    const opts = opts_str[0..len];

    var start: usize = 0;
    while (start < opts.len) {
        // Skip whitespace
        while (start < opts.len and opts[start] == ' ') : (start += 1) {}

        if (start >= opts.len) break;

        // Find end of option
        var end = start;
        while (end < opts.len and opts[end] != ',' and opts[end] != ' ') : (end += 1) {}

        const opt = opts[start..end];

        // Check if option is valid
        var found = false;
        for (valid_opts) |valid_opt| {
            if (std.mem.eql(u8, opt, valid_opt)) {
                found = true;
                break;
            }
        }

        if (!found) {
            return 0; // Invalid option found
        }

        start = end + 1;
    }

    return 1; // All options valid
}

test "bitstring segment parsing" {
    const data = [_]u8{ 0xDE, 0xAD, 0xBE, 0xEF };
    const opts = SegmentOptions{
        .size = 8,
        .unit = 8,
        .type_flag = 0,
        .signed = 0,
        .big_endian = 0,
        .literal = 0,
    };

    const result = rzx_bitstring_parse_segment(&data, data.len, 0, opts);
    try std.testing.expectEqual(@as(u32, 1), result.success);
}

test "bitstring validate opts" {
    const valid = "binary, big-endian\0";
    try std.testing.expectEqual(@as(u32, 1), rzx_bitstring_validate_opts(valid.ptr));
}

test "bitstring calculate size" {
    const sizes = [_]u32{ 8, 16, 8 };
    const result = rzx_bitstring_calculate_size(&sizes, sizes.len);
    try std.testing.expectEqual(@as(u64, 32), result);
}

test "bitstring segment float" {
    const data = [_]u8{ 0x40, 0x28, 0x5C, 0x28, 0xF5, 0xC2, 0x8F, 0x5B }; // 3.14 as f64
    const opts = SegmentOptions{
        .size = 64,
        .unit = 8,
        .type_flag = 1, // float
        .signed = 0,
        .big_endian = 0,
        .literal = 0,
    };

    const result = rzx_bitstring_parse_segment(&data, data.len, 0, opts);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, 64), result.bits);
}

test "bitstring segment binary variable" {
    const data = [_]u8{ 0xDE, 0xAD, 0xBE, 0xEF };
    const opts = SegmentOptions{
        .size = 0, // variable
        .unit = 8,
        .type_flag = 2, // binary
        .signed = 0,
        .big_endian = 0,
        .literal = 0,
    };

    const result = rzx_bitstring_parse_segment(&data, data.len, 0, opts);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, 32), result.bits); // All 4 bytes
}

test "bitstring segment binary fixed" {
    const data = [_]u8{ 0xDE, 0xAD, 0xBE, 0xEF };
    const opts = SegmentOptions{
        .size = 16, // 2 bytes
        .unit = 8,
        .type_flag = 2, // binary
        .signed = 0,
        .big_endian = 0,
        .literal = 0,
    };

    const result = rzx_bitstring_parse_segment(&data, data.len, 0, opts);
    try std.testing.expectEqual(@as(u32, 1), result.success);
    try std.testing.expectEqual(@as(u32, 16), result.bits);
}