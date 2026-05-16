/// Maps byte offsets in the source to 1-based (line, column) pairs and
/// 0-based code-point offsets.
///
/// Built once from the source text; lookups are O(log n) via binary search.
/// Columns and offsets are counted as Unicode code points (matching the
/// CommonMark `position` convention used by remark/micromark), not bytes —
/// necessary for multi-byte chars to land at the positions the reference
/// parsers report.
pub struct LineIndex<'a> {
    source: &'a [u8],
    /// `line_offsets[i]` is the byte offset where line `i+1` starts.
    /// `line_offsets[0]` is always 0.
    line_offsets: Vec<u32>,
    /// `line_cp_offsets[i]` is the *code-point* offset where line `i+1`
    /// starts. Same indexing as `line_offsets`. Equal to `line_offsets[i]`
    /// for ASCII-only sources; differs once a multi-byte char appears.
    line_cp_offsets: Vec<u32>,
}

impl<'a> LineIndex<'a> {
    pub fn from_source(source: &'a str) -> Self {
        let bytes = source.as_bytes();
        let line_count_estimate = bytes.len() / 40 + 1;
        let mut offsets = Vec::with_capacity(line_count_estimate);
        let mut cp_offsets = Vec::with_capacity(line_count_estimate);
        offsets.push(0u32);
        cp_offsets.push(0u32);
        let mut cp_count: u32 = 0;
        let mut last_byte: usize = 0;
        for nl_idx in memchr::memchr_iter(b'\n', bytes) {
            // Code points between previous line start and this newline + 1
            // (the newline itself is one code point too).
            for &b in &bytes[last_byte..=nl_idx] {
                if (b & 0xC0) != 0x80 {
                    cp_count += 1;
                }
            }
            offsets.push(nl_idx as u32 + 1);
            cp_offsets.push(cp_count);
            last_byte = nl_idx + 1;
        }
        LineIndex {
            source: bytes,
            line_offsets: offsets,
            line_cp_offsets: cp_offsets,
        }
    }

    /// Create a cursor for O(1) amortized lookups when offsets are roughly ascending.
    pub fn cursor(&self) -> LineIndexCursor<'_, 'a> {
        LineIndexCursor {
            index: self,
            last_line_idx: 0,
        }
    }
}

/// A cursor over a `LineIndex` that remembers its last position for O(1) amortized lookups.
///
/// When offsets arrive in roughly ascending order (as they do from a parser),
/// the cursor scans forward from the last known line instead of binary-searching.
pub struct LineIndexCursor<'idx, 'src> {
    index: &'idx LineIndex<'src>,
    last_line_idx: usize,
}

impl LineIndexCursor<'_, '_> {
    pub fn offset_to_line_col(&mut self, offset: u32) -> (u32, u32) {
        let idx = self.find_line_idx(offset);
        let line = idx as u32 + 1;
        let col = code_point_count(self.index.source, self.index.line_offsets[idx], offset) + 1;
        (line, col)
    }

    /// Convert a byte offset into the source to a code-point offset. Used
    /// for `position.start.offset` / `position.end.offset` which remark
    /// reports in code points, not bytes.
    pub fn byte_to_cp_offset(&mut self, byte_offset: u32) -> u32 {
        let idx = self.find_line_idx(byte_offset);
        self.index.line_cp_offsets[idx]
            + code_point_count(self.index.source, self.index.line_offsets[idx], byte_offset)
    }

    fn find_line_idx(&mut self, offset: u32) -> usize {
        let offsets = &self.index.line_offsets;
        let len = offsets.len();
        let mut idx = self.last_line_idx;
        let line_start = offsets[idx];
        if offset >= line_start {
            while idx + 1 < len && offsets[idx + 1] <= offset {
                idx += 1;
            }
        } else {
            while idx > 0 && offsets[idx] > offset {
                idx -= 1;
            }
        }
        self.last_line_idx = idx;
        idx
    }
}

/// Count Unicode code points in `source[start..end]`. UTF-8 continuation
/// bytes match `0b10xxxxxx`; every other byte is the start of a code point.
fn code_point_count(source: &[u8], start: u32, end: u32) -> u32 {
    let s = start as usize;
    let e = (end as usize).min(source.len());
    if e <= s {
        return 0;
    }
    let mut count: u32 = 0;
    for &b in &source[s..e] {
        if (b & 0xC0) != 0x80 {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let idx = LineIndex::from_source("hello");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1));
        assert_eq!(c.offset_to_line_col(4), (1, 5));
    }

    #[test]
    fn two_lines() {
        let idx = LineIndex::from_source("hi\nbye");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1));
        assert_eq!(c.offset_to_line_col(1), (1, 2));
        assert_eq!(c.offset_to_line_col(3), (2, 1));
        assert_eq!(c.offset_to_line_col(5), (2, 3));
    }

    #[test]
    fn trailing_newline() {
        let idx = LineIndex::from_source("abc\n");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1));
        assert_eq!(c.offset_to_line_col(2), (1, 3));
        assert_eq!(c.offset_to_line_col(4), (2, 1));
    }

    #[test]
    fn multi_line() {
        let idx = LineIndex::from_source("line1\nline2\nline3");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(6), (2, 1));
        assert_eq!(c.offset_to_line_col(10), (2, 5));
        assert_eq!(c.offset_to_line_col(12), (3, 1));
        assert_eq!(c.offset_to_line_col(16), (3, 5));
    }

    #[test]
    fn multi_byte_unicode_columns() {
        // ὐ is 3 bytes in UTF-8 but counts as 1 column.
        let idx = LineIndex::from_source("aὐb");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1)); // a
        assert_eq!(c.offset_to_line_col(1), (1, 2)); // ὐ start
        assert_eq!(c.offset_to_line_col(4), (1, 3)); // b (ὐ ate 3 bytes, +1 col)
    }

    #[test]
    fn unicode_after_newline() {
        // Column counts reset at line start.
        let idx = LineIndex::from_source("ab\nὐcd");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(3), (2, 1)); // ὐ
        assert_eq!(c.offset_to_line_col(6), (2, 2)); // c (3 bytes after line start = col 2)
        assert_eq!(c.offset_to_line_col(7), (2, 3)); // d
    }
}
