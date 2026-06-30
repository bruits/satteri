/// Maps byte offsets in the source to 1-based (line, column) pairs and
/// 0-based code-point offsets.
///
/// Built once from the source text; lookups are O(log n) via binary search.
/// Columns and offsets are counted as Unicode code points (matching the
/// CommonMark `position` convention used by remark/micromark), not bytes —
/// necessary for multi-byte chars to land at the positions the reference
/// parsers report.
/// Per-line metadata for non-ASCII sources, indexed in parallel with
/// `line_offsets`. The code-point offset and ASCII flag are folded into one
/// record so a lookup reads both with a single bounds-checked access landing
/// on one cache line, instead of indexing two parallel arrays.
#[derive(Clone, Copy)]
struct LineMeta {
    /// Code-point offset where the line starts (the code-point analogue of
    /// `line_offsets`). Equal to the byte offset until a multi-byte char
    /// appears earlier in the source.
    cp_offset: u32,
    /// Whether the line is pure ASCII. Lets a lookup on the line skip the
    /// per-byte continuation scan and use byte arithmetic.
    is_ascii: bool,
}

pub struct LineIndex<'a> {
    source: &'a [u8],
    /// `line_offsets[i]` is the byte offset where line `i+1` starts.
    /// `line_offsets[0]` is always 0.
    line_offsets: Vec<u32>,
    /// Per-line code-point offset + ASCII flag, indexed the same as
    /// `line_offsets`. Empty when `all_ascii` is true (the byte offset is the
    /// code-point offset everywhere, so no lookup needs it).
    line_meta: Vec<LineMeta>,
    /// True when the entire source is ASCII — every lookup short-circuits
    /// without consulting `line_meta`.
    all_ascii: bool,
    /// "Skip positions" mode: every lookup returns the all-zero sentinel so
    /// downstream code records no useful position. Used by HTML/JS output
    /// paths where the consumer never reads positions; skips the per-line
    /// `memchr` scan at construction and ~5 cursor lookups per MDAST node.
    /// `cursor()` on a disabled index produces a cursor whose `offset_to_line_col`
    /// and `byte_to_cp_offset` return `(0,0)` / `0` without consulting any
    /// state, and downstream `convert.rs::copy_position` already short-circuits
    /// on the zero sentinel — so HAST node positions stay unset for free.
    disabled: bool,
}

impl<'a> LineIndex<'a> {
    /// Construct a no-op index: `cursor()` returns trivial values without
    /// inspecting the source. The source slice is still held so debug helpers
    /// keep working, but no line scan happens.
    pub fn disabled_for(source: &'a str) -> Self {
        LineIndex {
            source: source.as_bytes(),
            line_offsets: Vec::new(),
            line_meta: Vec::new(),
            all_ascii: true,
            disabled: true,
        }
    }

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
                line_meta: Vec::new(),
                all_ascii: true,
                disabled: false,
            };
        }
        let mut line_meta = Vec::with_capacity(line_count_estimate);
        let mut cp_count: u32 = 0;
        let mut last_byte: usize = 0;
        for nl_idx in memchr::memchr_iter(b'\n', bytes) {
            let line = &bytes[last_byte..=nl_idx];
            let is_ascii = line.is_ascii();
            line_meta.push(LineMeta {
                cp_offset: cp_count,
                is_ascii,
            });
            cp_count += if is_ascii {
                line.len() as u32
            } else {
                code_point_count_bytes(line)
            };
            offsets.push(nl_idx as u32 + 1);
            last_byte = nl_idx + 1;
        }
        // Final line (no trailing newline): describe whether it is ASCII so
        // lookups falling on it can fast-path too.
        line_meta.push(LineMeta {
            cp_offset: cp_count,
            is_ascii: bytes[last_byte..].is_ascii(),
        });
        LineIndex {
            source: bytes,
            line_offsets: offsets,
            line_meta,
            all_ascii: false,
            disabled: false,
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
        if self.index.disabled {
            return (0, 0);
        }
        let (idx, line_start) = self.find_line_idx(offset);
        let col = if self.index.all_ascii || self.index.line_meta[idx].is_ascii {
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
        if self.index.all_ascii || self.index.disabled {
            return byte_offset;
        }
        let (idx, line_start) = self.find_line_idx(byte_offset);
        let meta = self.index.line_meta[idx];
        if meta.is_ascii {
            meta.cp_offset + (byte_offset - line_start)
        } else {
            meta.cp_offset
                + code_point_count_bytes(
                    &self.index.source[line_start as usize..byte_offset as usize],
                )
        }
    }

    /// Returns the line index containing `offset` and that line's start byte
    /// offset, so callers don't re-index `line_offsets` (and pay a second
    /// bounds check) for the start they already located.
    #[inline]
    fn find_line_idx(&mut self, offset: u32) -> (usize, u32) {
        let offsets = &self.index.line_offsets;
        let len = offsets.len();
        let mut idx = self.last_line_idx;
        if offset >= offsets[idx] {
            while idx + 1 < len && offsets[idx + 1] <= offset {
                idx += 1;
            }
        } else {
            while idx > 0 && offsets[idx] > offset {
                idx -= 1;
            }
        }
        self.last_line_idx = idx;
        (idx, offsets[idx])
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
