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
    /// Empty when `all_ascii` is true (saves an allocation since
    /// `line_cp_offsets[i] == line_offsets[i]` everywhere).
    line_cp_offsets: Vec<u32>,
    /// Per-line ASCII flag (`line_is_ascii[i]` covers line `i+1`). Lets a
    /// lookup on an ASCII line skip the per-byte continuation scan and
    /// fall back to byte arithmetic. Empty when `all_ascii` is true.
    line_is_ascii: Vec<bool>,
    /// True when the entire source is ASCII — every lookup short-circuits
    /// without consulting `line_cp_offsets` / `line_is_ascii`.
    all_ascii: bool,
}

impl<'a> LineIndex<'a> {
    pub fn from_source(source: &'a str) -> Self {
        let bytes = source.as_bytes();
        let all_ascii = bytes.is_ascii();
        let line_count_estimate = bytes.len() / 40 + 1;
        let mut offsets = Vec::with_capacity(line_count_estimate);
        offsets.push(0u32);
        if all_ascii {
            for nl_idx in memchr::memchr_iter(b'\n', bytes) {
                offsets.push(nl_idx as u32 + 1);
            }
            return LineIndex {
                source: bytes,
                line_offsets: offsets,
                line_cp_offsets: Vec::new(),
                line_is_ascii: Vec::new(),
                all_ascii: true,
            };
        }
        let mut cp_offsets = Vec::with_capacity(line_count_estimate);
        let mut line_is_ascii = Vec::with_capacity(line_count_estimate);
        cp_offsets.push(0u32);
        let mut cp_count: u32 = 0;
        let mut last_byte: usize = 0;
        for nl_idx in memchr::memchr_iter(b'\n', bytes) {
            let line = &bytes[last_byte..=nl_idx];
            let is_ascii = line.is_ascii();
            line_is_ascii.push(is_ascii);
            cp_count += if is_ascii {
                line.len() as u32
            } else {
                code_point_count_bytes(line)
            };
            offsets.push(nl_idx as u32 + 1);
            cp_offsets.push(cp_count);
            last_byte = nl_idx + 1;
        }
        // Final line (no trailing newline): describe whether it is ASCII so
        // lookups falling on it can fast-path too.
        line_is_ascii.push(bytes[last_byte..].is_ascii());
        LineIndex {
            source: bytes,
            line_offsets: offsets,
            line_cp_offsets: cp_offsets,
            line_is_ascii,
            all_ascii: false,
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
    #[inline]
    pub fn offset_to_line_col(&mut self, offset: u32) -> (u32, u32) {
        let idx = self.find_line_idx(offset);
        let line_start = self.index.line_offsets[idx];
        let col = if self.index.all_ascii || self.index.line_is_ascii[idx] {
            offset - line_start + 1
        } else {
            code_point_count_bytes(&self.index.source[line_start as usize..offset as usize]) + 1
        };
        (idx as u32 + 1, col)
    }

    /// Convert a byte offset into the source to a code-point offset. Used
    /// for `position.start.offset` / `position.end.offset` which remark
    /// reports in code points, not bytes.
    #[inline]
    pub fn byte_to_cp_offset(&mut self, byte_offset: u32) -> u32 {
        if self.index.all_ascii {
            return byte_offset;
        }
        let idx = self.find_line_idx(byte_offset);
        let line_start = self.index.line_offsets[idx];
        let line_cp = self.index.line_cp_offsets[idx];
        if self.index.line_is_ascii[idx] {
            line_cp + (byte_offset - line_start)
        } else {
            line_cp
                + code_point_count_bytes(
                    &self.index.source[line_start as usize..byte_offset as usize],
                )
        }
    }

    #[inline]
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

/// Count Unicode code points in a byte slice. UTF-8 continuation bytes
/// match `0b10xxxxxx`; every other byte starts a code point.
fn code_point_count_bytes(bytes: &[u8]) -> u32 {
    let mut count: u32 = 0;
    for &b in bytes {
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

    #[test]
    fn ascii_lines_in_mixed_source() {
        let idx = LineIndex::from_source("abc\nx🪐y\ndef");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1)); // a
        assert_eq!(c.offset_to_line_col(2), (1, 3)); // c
        assert_eq!(c.offset_to_line_col(4), (2, 1)); // x
        assert_eq!(c.offset_to_line_col(9), (2, 3)); // y (🪐 is 4 bytes, 1 cp)
        assert_eq!(c.offset_to_line_col(11), (3, 1)); // d
        assert_eq!(c.offset_to_line_col(13), (3, 3)); // f
    }
}
