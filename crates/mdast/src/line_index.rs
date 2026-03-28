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
        let mut offsets = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                offsets.push(i as u32 + 1);
            }
        }
        LineIndex {
            line_offsets: offsets,
        }
    }

    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        match self.line_offsets.binary_search(&offset) {
            Ok(idx) => {
                let line = idx as u32 + 1;
                (line, 1)
            }
            Err(idx) => {
                // idx is the insertion point; the line is the one before it.
                let line_idx = idx - 1;
                let line = line_idx as u32 + 1;
                let col = offset - self.line_offsets[line_idx] + 1;
                (line, col)
            }
        }
    }

    /// Number of lines in the source (including a final unterminated line).
    pub fn line_count(&self) -> usize {
        self.line_offsets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let idx = LineIndex::from_source("hello");
        assert_eq!(idx.offset_to_line_col(0), (1, 1));
        assert_eq!(idx.offset_to_line_col(4), (1, 5));
        assert_eq!(idx.line_count(), 1);
    }

    #[test]
    fn two_lines() {
        let idx = LineIndex::from_source("hi\nbye");
        // "hi\n" = offsets 0,1,2(\n)
        assert_eq!(idx.offset_to_line_col(0), (1, 1));
        assert_eq!(idx.offset_to_line_col(1), (1, 2));
        // byte 3 is 'b' on line 2
        assert_eq!(idx.offset_to_line_col(3), (2, 1));
        assert_eq!(idx.offset_to_line_col(5), (2, 3));
        assert_eq!(idx.line_count(), 2);
    }

    #[test]
    fn trailing_newline() {
        // "abc\n" — the newline pushes offset 4 into line_offsets, but
        // that represents an empty line 2.
        let idx = LineIndex::from_source("abc\n");
        assert_eq!(idx.offset_to_line_col(0), (1, 1));
        assert_eq!(idx.offset_to_line_col(2), (1, 3));
        // offset 4 is the start of the empty trailing line
        assert_eq!(idx.offset_to_line_col(4), (2, 1));
        assert_eq!(idx.line_count(), 2);
    }

    #[test]
    fn multi_line() {
        let source = "line1\nline2\nline3";
        let idx = LineIndex::from_source(source);
        assert_eq!(idx.line_count(), 3);
        // "line1" = bytes 0-4, '\n' at 5
        // "line2" starts at 6
        // "line3" starts at 12
        assert_eq!(idx.offset_to_line_col(6), (2, 1));
        assert_eq!(idx.offset_to_line_col(10), (2, 5));
        assert_eq!(idx.offset_to_line_col(12), (3, 1));
        assert_eq!(idx.offset_to_line_col(16), (3, 5));
    }
}
