//! Source buffer management kernels.
//!
//! These kernels handle source text scanning and span operations.
//! They must not retain Rust pointers or own compiler state.

const std = @import("std");

/// A source span with start and end offsets.
pub const SourceSpan = extern struct {
    start: u32,
    end: u32,
};

/// Scan result for source operations.
pub const SourceScanResult = extern struct {
    span: SourceSpan,
    success: u32,
    error_code: u32,
};

/// Create a source span from start and end offsets.
///
/// # Safety
/// - `start` and `end` must be valid offsets
/// - `start` must be <= `end`
pub export fn rzx_span_create(start: u32, end: u32) SourceSpan {
    return SourceSpan{
        .start = start,
        .end = end,
    };
}

/// Check if a span is valid within a buffer.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
pub export fn rzx_span_is_valid(data: [*]const u8, len: usize, start: u32, end: u32) u32 {
    if (start > end) return 0;
    if (end > len) return 0;
    return 1;
}

/// Merge two adjacent or overlapping spans.
///
/// # Safety
/// - `span1` and `span2` must be valid spans
pub export fn rzx_span_merge(span1: SourceSpan, span2: SourceSpan) SourceSpan {
    return SourceSpan{
        .start = if (span1.start < span2.start) span1.start else span2.start,
        .end = if (span1.end > span2.end) span1.end else span2.end,
    };
}

/// Extract text from a source span.
///
/// # Safety
/// - `data` must point to valid memory
/// - `start` and `end` must be valid offsets with start <= end
pub export fn rzx_span_text(
    data: [*]const u8,
    start: u32,
    end: u32,
    out_buf: [*]u8,
    out_len: usize,
) u32 {
    const text_len = end - start;
    if (text_len > out_len) {
        return 0; // Buffer too small
    }

    @memcpy(out_buf, data + start, text_len);
    return text_len;
}

/// Find the next newline in a source buffer starting from offset.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
/// - `offset` must be <= len
pub export fn rzx_find_next_newline(data: [*]const u8, len: usize, offset: usize) u32 {
    if (offset >= len) {
        return @as(u32, @intCast(len));
    }

    var pos = offset;
    while (pos < len) : (pos += 1) {
        if (data[pos] == '\n') {
            return @as(u32, @intCast(pos + 1));
        }
    }

    return @as(u32, @intCast(len));
}

/// Find the previous newline in a source buffer before offset.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
pub export fn rzx_find_prev_newline(data: [*]const u8, offset: usize) u32 {
    if (offset == 0) return 0;

    var pos = offset;
    while (pos > 0) : (pos -= 1) {
        if (data[pos - 1] == '\n') {
            return @as(u32, @intCast(pos));
        }
    }

    return 0;
}

/// Count newlines in a buffer up to offset.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
pub export fn rzx_count_newlines(data: [*]const u8, offset: usize) u32 {
    var count: u32 = 0;
    var pos: usize = 0;
    while (pos < offset) : (pos += 1) {
        if (data[pos] == '\n') {
            count += 1;
        }
    }
    return count;
}

/// Get the byte offset for a line number.
///
/// # Safety
/// - `data` must point to valid memory of at least `len` bytes
pub export fn rzx_line_offset(data: [*]const u8, len: usize, line: u32) u32 {
    if (line == 0) return 0;

    var current_line: u32 = 0;
    var pos: usize = 0;

    while (pos < len) {
        if (data[pos] == '\n') {
            current_line += 1;
            if (current_line == line) {
                return @as(u32, @intCast(pos + 1));
            }
        }
        pos += 1;
    }

    // Line beyond buffer
    return @as(u32, @intCast(len));
}

/// Check if a span represents an empty region.
pub export fn rzx_span_is_empty(span: SourceSpan) u32 {
    return if (span.start >= span.end) 1 else 0;
}

/// Get the length of a span.
pub export fn rzx_span_length(span: SourceSpan) u32 {
    return if (span.end > span.start) span.end - span.start else 0;
}

test "span create" {
    const span = rzx_span_create(5, 10);
    try std.testing.expectEqual(@as(u32, 5), span.start);
    try std.testing.expectEqual(@as(u32, 10), span.end);
}

test "span merge" {
    const span1 = SourceSpan{ .start = 0, .end = 5 };
    const span2 = SourceSpan{ .start = 3, .end = 10 };
    const merged = rzx_span_merge(span1, span2);
    try std.testing.expectEqual(@as(u32, 0), merged.start);
    try std.testing.expectEqual(@as(u32, 10), merged.end);
}

test "span is empty" {
    const empty = SourceSpan{ .start = 5, .end = 5 };
    try std.testing.expectEqual(@as(u32, 1), rzx_span_is_empty(empty));

    const non_empty = SourceSpan{ .start = 5, .end = 6 };
    try std.testing.expectEqual(@as(u32, 0), rzx_span_is_empty(non_empty));
}