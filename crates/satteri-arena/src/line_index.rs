/// Maps byte offsets in the source to 1-based (line, column) pairs.
///
/// Built once from the source text; lookups are O(log n) via binary search.
pub struct LineIndex {
    /// `line_offsets[i]` is the byte offset where line `i+1` starts
    /// (0-indexed internally, but we expose 1-based lines).
    /// `line_offsets[0]` is always 0 (start of line 1).
    line_offsets: Vec<u32>,
}

impl LineIndex {
    pub fn from_source(source: &str) -> Self {
        let bytes = source.as_bytes();
        let line_count_estimate = bytes.len() / 40 + 1;
        let mut offsets = Vec::with_capacity(line_count_estimate);
        offsets.push(0u32);
        for nl_idx in memchr::memchr_iter(b'\n', bytes) {
            offsets.push(nl_idx as u32 + 1);
        }
        LineIndex {
            line_offsets: offsets,
        }
    }

    /// Create a cursor for O(1) amortized lookups when offsets are roughly ascending.
    pub fn cursor(&self) -> LineIndexCursor<'_> {
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
pub struct LineIndexCursor<'a> {
    index: &'a LineIndex,
    last_line_idx: usize,
}

impl LineIndexCursor<'_> {
    pub fn offset_to_line_col(&mut self, offset: u32) -> (u32, u32) {
        let offsets = &self.index.line_offsets;
        let len = offsets.len();

        // Fast path: check if offset is still on the same line as last lookup.
        let mut idx = self.last_line_idx;
        let line_start = offsets[idx];
        if offset >= line_start {
            // Scan forward from current position.
            while idx + 1 < len && offsets[idx + 1] <= offset {
                idx += 1;
            }
        } else {
            // Offset went backwards, scan backwards from current position.
            while idx > 0 && offsets[idx] > offset {
                idx -= 1;
            }
        }

        self.last_line_idx = idx;
        let line = idx as u32 + 1;
        let col = offset - offsets[idx] + 1;
        (line, col)
    }
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
}
