//! Source file and span management for the Rust/Zig Elixir compiler.
//!
//! Provides core types for tracking source locations, file contents,
//! and source map management needed by the lexer, parser, and diagnostics.

#[cfg(test)]
use chimera_allocator as _;

use std::ops::Range;
use std::sync::Arc;

/// A byte offset in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteOffset(pub u32);

impl ByteOffset {
    pub fn new(offset: u32) -> Self {
        ByteOffset(offset)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// A line number (0-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineNumber(pub u32);

impl LineNumber {
    pub fn new(line: u32) -> Self {
        LineNumber(line)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// A column offset in a line (byte offset from line start).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnOffset(pub u32);

impl ColumnOffset {
    pub fn new(column: u32) -> Self {
        ColumnOffset(column)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// A source span tracking start and end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    pub start: SourceOffset,
    pub end: SourceOffset,
}

impl SourceSpan {
    pub fn new(start: SourceOffset, end: SourceOffset) -> Self {
        SourceSpan { start, end }
    }

    pub fn merge(self, other: SourceSpan) -> SourceSpan {
        SourceSpan {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }

    pub fn to_range(self) -> Range<usize> {
        self.start.0 as usize..self.end.0 as usize
    }
}

/// A byte offset in source (absolute from file start).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceOffset(pub u32);

impl SourceOffset {
    pub fn new(offset: u32) -> Self {
        SourceOffset(offset)
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    pub fn line_column(self, source: &str) -> (LineNumber, ColumnOffset) {
        let mut line = 0u32;
        let mut last_newline = 0i64;
        for (i, byte) in source[..self.0 as usize].bytes().enumerate() {
            if byte == b'\n' {
                line += 1;
                last_newline = i as i64 + 1;
            }
        }
        let col = self.0.saturating_sub(last_newline as u32);
        (LineNumber::new(line), ColumnOffset::new(col))
    }
}

/// Unique identifier for a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceFileId(pub u32);

impl SourceFileId {
    pub fn new(id: u32) -> Self {
        SourceFileId(id)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A source file with its content and metadata.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: SourceFileId,
    pub path: Arc<str>,
    pub content: Arc<str>,
    pub line_offsets: Vec<u32>,
}

impl SourceFile {
    pub fn new(id: SourceFileId, path: impl Into<Arc<str>>, content: impl Into<Arc<str>>) -> Self {
        let content = content.into();
        let line_offsets = compute_line_offsets(&content);
        SourceFile {
            id,
            path: path.into(),
            content,
            line_offsets,
        }
    }

    pub fn get_line(&self, line: LineNumber) -> Option<&str> {
        let line_idx = line.0 as usize;
        if line_idx >= self.line_offsets.len() {
            return None;
        }
        let start = self.line_offsets[line_idx] as usize;
        let end = if line_idx + 1 < self.line_offsets.len() {
            self.line_offsets[line_idx + 1] as usize
        } else {
            self.content.len()
        };
        Some(self.content[start..end].trim_end_matches('\n'))
    }

    pub fn span_to_location(&self, span: SourceSpan) -> SourceLocation {
        let (start_line, start_col) = self.offset_to_line_col(span.start);
        let (end_line, end_col) = self.offset_to_line_col(span.end);
        SourceLocation {
            file_id: self.id,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    pub fn offset_to_line_col(&self, offset: SourceOffset) -> (LineNumber, ColumnOffset) {
        let target = offset.0;
        let line_idx = self
            .line_offsets
            .binary_search(&target)
            .unwrap_or_else(|idx| idx.saturating_sub(1));
        let line_start = self.line_offsets[line_idx];
        let col = target.saturating_sub(line_start);
        (LineNumber::new(line_idx as u32), ColumnOffset::new(col))
    }
}

fn compute_line_offsets(content: &str) -> Vec<u32> {
    let mut offsets = vec![0];
    for (i, c) in content.char_indices() {
        if c == '\n' {
            offsets.push((i + c.len_utf8()) as u32);
        }
    }
    offsets
}

/// A source location with file, line, and column information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub file_id: SourceFileId,
    pub start_line: LineNumber,
    pub start_col: ColumnOffset,
    pub end_line: LineNumber,
    pub end_col: ColumnOffset,
}

impl SourceLocation {
    pub fn new(
        file_id: SourceFileId,
        start_line: LineNumber,
        start_col: ColumnOffset,
        end_line: LineNumber,
        end_col: ColumnOffset,
    ) -> Self {
        SourceLocation {
            file_id,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}

/// Manages source files and provides source map functionality.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<Arc<SourceFile>>,
}

impl SourceMap {
    pub fn new() -> Self {
        SourceMap { files: Vec::new() }
    }

    pub fn add_file(&mut self, path: impl Into<Arc<str>>, content: impl Into<Arc<str>>) -> SourceFileId {
        let id = SourceFileId(self.files.len() as u32);
        let file = SourceFile::new(id, path, content);
        self.files.push(Arc::new(file));
        id
    }

    pub fn get_file(&self, id: SourceFileId) -> Option<&Arc<SourceFile>> {
        self.files.get(id.0 as usize)
    }

    pub fn get_span_text(&self, file_id: SourceFileId, span: SourceSpan) -> Option<&str> {
        let file = self.get_file(file_id)?;
        let range = span.to_range();
        file.content.get(range)
    }

    /// Get source context around an offset for debugging.
    pub fn get_context(&self, file_id: SourceFileId, offset: SourceOffset, context_lines: usize) -> Option<DebugContext> {
        let file = self.get_file(file_id)?;
        let (line, col) = self.offset_to_line_col(file_id, offset)?;

        let start_line = line.0.saturating_sub(context_lines as u32);
        let end_line = (line.0 + context_lines as u32).min(file.line_offsets.len() as u32 - 1);

        let mut lines = Vec::new();
        for i in start_line..=end_line {
            if let Some(line_text) = file.get_line(LineNumber::new(i)) {
                lines.push(DebugLine {
                    number: i + 1,
                    text: line_text.to_string(),
                    is_target: i == line.0,
                });
            }
        }

        Some(DebugContext {
            file_path: file.path.to_string(),
            offset,
            line,
            column: col,
            lines,
        })
    }

    /// Convert offset to line and column.
    pub fn offset_to_line_col(&self, file_id: SourceFileId, offset: SourceOffset) -> Option<(LineNumber, ColumnOffset)> {
        let file = self.get_file(file_id)?;
        let (line, col) = offset.line_column(&file.content);
        Some((line, col))
    }
}

/// Context for debugging - surrounding source lines around an offset.
#[derive(Debug, Clone)]
pub struct DebugContext {
    pub file_path: String,
    pub offset: SourceOffset,
    pub line: LineNumber,
    pub column: ColumnOffset,
    pub lines: Vec<DebugLine>,
}

/// A single line in debug context.
#[derive(Debug, Clone)]
pub struct DebugLine {
    pub number: u32,
    pub text: String,
    pub is_target: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_file_new() {
        let file = SourceFile::new(SourceFileId::new(0), "test.ex", "defmodule Foo\n  :ok\nend");
        assert_eq!(file.id, SourceFileId::new(0));
        assert_eq!(file.path.as_ref(), "test.ex");
    }

    #[test]
    fn test_source_file_get_line() {
        let file = SourceFile::new(SourceFileId::new(0), "test.ex", "line1\nline2\nline3");
        assert_eq!(file.get_line(LineNumber::new(0)), Some("line1"));
        assert_eq!(file.get_line(LineNumber::new(1)), Some("line2"));
        assert_eq!(file.get_line(LineNumber::new(2)), Some("line3"));
        assert_eq!(file.get_line(LineNumber::new(3)), None);
    }

    #[test]
    fn test_source_span_merge() {
        let span1 = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(5));
        let span2 = SourceSpan::new(SourceOffset::new(3), SourceOffset::new(10));
        let merged = span1.merge(span2);
        assert_eq!(merged.start, SourceOffset::new(0));
        assert_eq!(merged.end, SourceOffset::new(10));
    }

    #[test]
    fn test_source_map_add_file() {
        let mut sm = SourceMap::new();
        let id = sm.add_file("test.ex", "defmodule Foo\nend");
        assert_eq!(id, SourceFileId::new(0));
        assert!(sm.get_file(id).is_some());
    }

    #[test]
    fn test_source_map_get_span_text() {
        let mut sm = SourceMap::new();
        let id = sm.add_file("test.ex", "defmodule Foo\nend");
        let span = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(9));
        assert_eq!(sm.get_span_text(id, span), Some("defmodule"));
    }

    #[test]
    fn test_line_offsets() {
        let content = "line1\nline2\nline3";
        let file = SourceFile::new(SourceFileId::new(0), "test.ex", content);
        assert_eq!(file.line_offsets.len(), 3);
        assert_eq!(file.line_offsets[0], 0);
        assert_eq!(file.line_offsets[1], 6);
        assert_eq!(file.line_offsets[2], 12);
    }

    #[test]
    fn test_source_offset_utf8() {
        // UTF-8 multi-byte character handling
        let content = "αβγ\nline2";  // Greek letters - 2 bytes each
        let file = SourceFile::new(SourceFileId::new(0), "test.ex", content);
        // α is at offset 0, β at offset 2, γ at offset 4, \n at offset 6
        assert_eq!(file.line_offsets.len(), 2);
        assert_eq!(file.line_offsets[0], 0);
        assert_eq!(file.line_offsets[1], 7); // 6 + newline
    }

    #[test]
    fn test_source_offset_crlf() {
        // CRLF line ending handling
        let content = "line1\r\nline2\r\nline3";
        let file = SourceFile::new(SourceFileId::new(0), "test.ex", content);
        // \r\n is 2 bytes but should count as single line break
        assert_eq!(file.line_offsets.len(), 3);
        assert_eq!(file.line_offsets[0], 0);
        assert_eq!(file.line_offsets[1], 7); // "line1" + \r\n = 7
        assert_eq!(file.line_offsets[2], 14);
    }

    #[test]
    fn test_source_file_empty() {
        // Empty file handling
        let file = SourceFile::new(SourceFileId::new(0), "empty.ex", "");
        assert_eq!(file.line_offsets.len(), 1); // Just the start
        assert_eq!(file.line_offsets[0], 0);
        assert_eq!(file.get_line(LineNumber::new(0)), Some(""));
    }

    #[test]
    fn test_source_file_only_newlines() {
        // File with only newlines
        let file = SourceFile::new(SourceFileId::new(0), "nl.ex", "\n\n\n");
        assert_eq!(file.line_offsets.len(), 4); // Start + 3 newlines
    }

    #[test]
    fn test_source_span_empty() {
        let span = SourceSpan::new(SourceOffset::new(5), SourceOffset::new(5));
        assert!(span.is_empty());
    }

    #[test]
    fn test_source_span_invalid_range() {
        let span = SourceSpan::new(SourceOffset::new(10), SourceOffset::new(5));
        assert!(span.is_empty()); // start > end
    }

    #[test]
    fn test_source_map_multi_file() {
        let mut sm = SourceMap::new();
        let id1 = sm.add_file("file1.ex", "defmodule A do end");
        let id2 = sm.add_file("file2.ex", "defmodule B do end");

        assert_ne!(id1, id2);
        assert_eq!(id1, SourceFileId::new(0));
        assert_eq!(id2, SourceFileId::new(1));

        let span1 = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(9));
        let span2 = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(9));

        assert_eq!(sm.get_span_text(id1, span1), Some("defmodule"));
        assert_eq!(sm.get_span_text(id2, span2), Some("defmodule"));
    }

    #[test]
    fn test_source_offset_to_line_col() {
        // "line1\nline2\nline3" = 18 bytes total
        // line_offsets should be [0, 6, 12] for lines 0, 1, 2
        let content = "line1\nline2\nline3";
        let file = SourceFile::new(SourceFileId::new(0), "test.ex", content);

        // Offset 0 -> line 0, col 0 (start of "line1")
        let (line, col) = file.offset_to_line_col(SourceOffset::new(0));
        assert_eq!(line, LineNumber::new(0));
        assert_eq!(col, ColumnOffset::new(0));

        // Offset 5 -> line 0, col 5 (the '1' at end of "line1")
        let (line, col) = file.offset_to_line_col(SourceOffset::new(5));
        assert_eq!(line, LineNumber::new(0));
        assert_eq!(col, ColumnOffset::new(5));

        // Offset 6 -> line 1, col 0 (start of "line2")
        let (line, col) = file.offset_to_line_col(SourceOffset::new(6));
        assert_eq!(line, LineNumber::new(1));
        assert_eq!(col, ColumnOffset::new(0));

        // Offset 11 -> line 1, col 5 (the '2' at end of "line2", before \n)
        let (line, col) = file.offset_to_line_col(SourceOffset::new(11));
        assert_eq!(line, LineNumber::new(1));
        assert_eq!(col, ColumnOffset::new(5));

        // Offset 12 -> line 2, col 0 (start of "line3")
        let (line, col) = file.offset_to_line_col(SourceOffset::new(12));
        assert_eq!(line, LineNumber::new(2));
        assert_eq!(col, ColumnOffset::new(0));
    }

    #[test]
    fn test_source_file_span_to_location() {
        let content = "fn foo do\n  :ok\nend";
        let file = SourceFile::new(SourceFileId::new(0), "test.ex", content);

        let span = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(2));
        let location = file.span_to_location(span);

        assert_eq!(location.file_id, SourceFileId::new(0));
        assert_eq!(location.start_line, LineNumber::new(0));
        assert_eq!(location.start_col, ColumnOffset::new(0));
    }

    #[test]
    fn test_source_span_to_range() {
        let span = SourceSpan::new(SourceOffset::new(5), SourceOffset::new(10));
        let range = span.to_range();
        assert_eq!(range, 5..10);
    }

    #[test]
    fn test_byte_offset() {
        let offset = ByteOffset::new(42);
        assert_eq!(offset.as_u32(), 42);
    }

    #[test]
    fn test_line_number() {
        let line = LineNumber::new(10);
        assert_eq!(line.as_u32(), 10);
    }

    #[test]
    fn test_column_offset() {
        let col = ColumnOffset::new(5);
        assert_eq!(col.as_u32(), 5);
    }

    #[test]
    fn test_source_file_id_index() {
        let id = SourceFileId::new(3);
        assert_eq!(id.index(), 3);
    }

    #[test]
    fn test_source_offset_as_usize() {
        let offset = SourceOffset::new(100);
        assert_eq!(offset.as_usize(), 100);
    }

    #[test]
    fn test_source_offset_line_column() {
        // "abc\ndef\nghi" - positions:
        // a=0, b=1, c=2, \n=3, d=4, e=5, f=6, \n=7, g=8, h=9, i=10
        let content = "abc\ndef\nghi";
        let offset = SourceOffset::new(4); // 'd' in line 2 (second line, 0-indexed)
        let (line, col) = offset.line_column(content);
        // After first \n at index 3, line=1, last_newline=4
        // col = 4 - 4 = 0, so 'd' is at column 0 of line 1
        assert_eq!(line, LineNumber::new(1)); // second line (0-indexed)
        assert_eq!(col, ColumnOffset::new(0)); // first char of line
    }

    #[test]
    fn test_source_location_new() {
        let loc = SourceLocation::new(
            SourceFileId::new(0),
            LineNumber::new(5),
            ColumnOffset::new(3),
            LineNumber::new(5),
            ColumnOffset::new(10),
        );
        assert_eq!(loc.start_line, LineNumber::new(5));
        assert_eq!(loc.start_col, ColumnOffset::new(3));
        assert_eq!(loc.end_line, LineNumber::new(5));
        assert_eq!(loc.end_col, ColumnOffset::new(10));
    }

    #[test]
    fn test_source_map_get_nonexistent_file() {
        let sm = SourceMap::new();
        assert!(sm.get_file(SourceFileId::new(999)).is_none());
    }

    #[test]
    fn test_source_map_get_span_text_out_of_bounds() {
        let mut sm = SourceMap::new();
        let id = sm.add_file("test.ex", "abc");
        let span = SourceSpan::new(SourceOffset::new(0), SourceOffset::new(100));
        assert!(sm.get_span_text(id, span).is_none());
    }

    #[test]
    fn test_source_map_get_context() {
        let mut sm = SourceMap::new();
        let id = sm.add_file("test.ex", "line1\nline2\nline3\nline4\nline5");
        let ctx = sm.get_context(id, SourceOffset::new(12), 1);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.file_path, "test.ex");
        assert_eq!(ctx.lines.len(), 3); // line 3 and 1 line above/below
    }

    #[test]
    fn test_source_map_offset_to_line_col() {
        let mut sm = SourceMap::new();
        let id = sm.add_file("test.ex", "abc\ndef\nghi");
        let result = sm.offset_to_line_col(id, SourceOffset::new(4));
        assert!(result.is_some());
        let (line, col) = result.unwrap();
        assert_eq!(line, LineNumber::new(1));
        assert_eq!(col, ColumnOffset::new(0));
    }

    #[test]
    fn test_debug_context_lines() {
        let ctx = DebugContext {
            file_path: "test.ex".to_string(),
            offset: SourceOffset::new(10),
            line: LineNumber::new(2),
            column: ColumnOffset::new(0),
            lines: vec![
                DebugLine { number: 1, text: "line1".to_string(), is_target: false },
                DebugLine { number: 2, text: "line2".to_string(), is_target: false },
                DebugLine { number: 3, text: "line3".to_string(), is_target: true },
            ],
        };
        assert_eq!(ctx.lines.len(), 3);
        assert!(ctx.lines[2].is_target);
        assert!(!ctx.lines[0].is_target);
    }

    #[test]
    fn test_debug_line_is_target() {
        let line = DebugLine {
            number: 5,
            text: "target line".to_string(),
            is_target: true,
        };
        assert!(line.is_target);
        assert_eq!(line.number, 5);
    }
}