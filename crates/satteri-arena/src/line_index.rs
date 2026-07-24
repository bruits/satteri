/// Per-line UTF-16 offset and ASCII flag for non-ASCII sources, indexed
/// like `line_offsets`. Folded into one record so a lookup reads both from a
/// single cache line rather than two parallel arrays.
#[derive(Clone, Copy)]
struct LineMeta {
    /// UTF-16 code-unit offset where the line starts (the UTF-16 analogue of
    /// `line_offsets`). Equal to the byte offset until a multi-byte char
    /// appears earlier in the source.
    utf16_offset: u32,
    /// Whether the line is pure ASCII. Lets a lookup on the line skip the
    /// per-byte continuation scan and use byte arithmetic.
    is_ascii: bool,
}

/// Maps byte offsets to 1-based (line, column) pairs and 0-based UTF-16
/// offsets. Built once; lookups are O(log n). Columns and offsets count UTF-16
/// code units — the unit JS strings index by — so `position` values slice the
/// source string even for astral characters (two units each).
pub struct LineIndex<'a> {
    source: &'a [u8],
    /// `line_offsets[i]` is the byte offset where line `i+1` starts.
    /// `line_offsets[0]` is always 0.
    line_offsets: Vec<u32>,
    /// Per-line UTF-16 offset + ASCII flag, indexed the same as
    /// `line_offsets`. Empty when `all_ascii` is true (the byte offset is the
    /// UTF-16 offset everywhere, so no lookup needs it).
    line_meta: Vec<LineMeta>,
    /// True when the entire source is ASCII — every lookup short-circuits
    /// without consulting `line_meta`.
    all_ascii: bool,
    /// "Skip positions" mode: every lookup returns the all-zero sentinel and
    /// the per-line scan at construction is skipped. Used by HTML/JS output
    /// paths that never read positions.
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
        let mut utf16_count: u32 = 0;
        let mut last_byte: usize = 0;
        for nl_idx in memchr::memchr_iter(b'\n', bytes) {
            let line = &bytes[last_byte..=nl_idx];
            let is_ascii = line.is_ascii();
            line_meta.push(LineMeta {
                utf16_offset: utf16_count,
                is_ascii,
            });
            utf16_count += if is_ascii {
                line.len() as u32
            } else {
                utf16_len_bytes(line)
            };
            offsets.push(nl_idx as u32 + 1);
            last_byte = nl_idx + 1;
        }
        // Final line (no trailing newline): describe whether it is ASCII so
        // lookups falling on it can fast-path too.
        line_meta.push(LineMeta {
            utf16_offset: utf16_count,
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
            last_line_col: (u32::MAX, (0, 0)),
        }
    }

    /// UTF-16 offset for a 1-based `(line, col)` pair from this index;
    /// columns already count UTF-16 code units, so no source rescan is needed.
    pub fn utf16_offset_at(&self, line: u32, col: u32) -> u32 {
        if line == 0 {
            return 0;
        }
        let idx = (line - 1) as usize;
        match self.line_meta.get(idx) {
            Some(meta) => meta.utf16_offset + (col - 1),
            // No per-line meta means byte offsets equal UTF-16 offsets.
            None => self.line_offsets.get(idx).copied().unwrap_or(0) + (col - 1),
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
    /// One-entry memo; a sibling's end offset is usually the next one's start.
    last_line_col: (u32, (u32, u32)),
}

impl LineIndexCursor<'_, '_> {
    #[inline]
    pub fn offset_to_line_col(&mut self, offset: u32) -> (u32, u32) {
        if self.index.disabled {
            return (0, 0);
        }
        if offset == self.last_line_col.0 {
            return self.last_line_col.1;
        }
        let (idx, line_start) = self.find_line_idx(offset);
        let col = if self.index.all_ascii || self.index.line_meta[idx].is_ascii {
            offset - line_start + 1
        } else {
            utf16_len_bytes(&self.index.source[line_start as usize..offset as usize]) + 1
        };
        self.last_line_col = (offset, (idx as u32 + 1, col));
        self.last_line_col.1
    }

    /// Convert a byte offset into the source to a UTF-16 offset;
    /// `position.start.offset` / `position.end.offset` are JS string
    /// indices, not bytes.
    #[inline]
    pub fn byte_to_utf16_offset(&mut self, byte_offset: u32) -> u32 {
        if self.index.all_ascii || self.index.disabled {
            return byte_offset;
        }
        let (idx, line_start) = self.find_line_idx(byte_offset);
        let meta = self.index.line_meta[idx];
        if meta.is_ascii {
            meta.utf16_offset + (byte_offset - line_start)
        } else {
            meta.utf16_offset
                + utf16_len_bytes(&self.index.source[line_start as usize..byte_offset as usize])
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
        // Nearby offsets are the common case; far jumps binary-search instead.
        const LINEAR_STEPS: usize = 4;
        if offset >= offsets[idx] {
            let mut steps = 0;
            while idx + 1 < len && offsets[idx + 1] <= offset {
                idx += 1;
                steps += 1;
                if steps == LINEAR_STEPS {
                    idx += offsets[idx + 1..].partition_point(|&o| o <= offset);
                    break;
                }
            }
        } else {
            let mut steps = 0;
            while idx > 0 && offsets[idx] > offset {
                idx -= 1;
                steps += 1;
                if steps == LINEAR_STEPS {
                    idx = offsets[..idx + 1].partition_point(|&o| o <= offset) - 1;
                    break;
                }
            }
        }
        self.last_line_idx = idx;
        (idx, offsets[idx])
    }
}

/// UTF-16 length of a UTF-8 byte slice. Continuation bytes (`0b10xxxxxx`)
/// don't count; a 4-byte sequence (lead byte ≥ `0xF0`) is an astral code
/// point — a surrogate pair, two units.
fn utf16_len_bytes(bytes: &[u8]) -> u32 {
    let mut count: u32 = 0;
    for &b in bytes {
        if (b & 0xC0) != 0x80 {
            count += if b >= 0xF0 { 2 } else { 1 };
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
        assert_eq!(c.offset_to_line_col(9), (2, 4)); // y (🪐 is 4 bytes, 2 UTF-16 units)
        assert_eq!(c.offset_to_line_col(11), (3, 1)); // d
        assert_eq!(c.offset_to_line_col(13), (3, 3)); // f
    }

    #[test]
    fn byte_to_utf16_offset_multibyte() {
        // "❤️" is U+2764 U+FE0F: 6 bytes, 2 units. "😀" is U+1F600: 4 bytes,
        // 2 units (surrogate pair).
        let idx = LineIndex::from_source("❤️a\n😀b");
        let mut c = idx.cursor();
        assert_eq!(c.byte_to_utf16_offset(0), 0); // ❤️
        assert_eq!(c.byte_to_utf16_offset(6), 2); // a
        assert_eq!(c.byte_to_utf16_offset(8), 4); // 😀 (the \n counts too)
        assert_eq!(c.byte_to_utf16_offset(12), 6); // b
    }

    #[test]
    fn utf16_offset_at_agrees_with_line_col() {
        let idx = LineIndex::from_source("❤️a\n😀b");
        let mut c = idx.cursor();
        for byte_offset in [0u32, 6, 8, 12, 13] {
            let (line, col) = c.offset_to_line_col(byte_offset);
            assert_eq!(
                idx.utf16_offset_at(line, col),
                c.byte_to_utf16_offset(byte_offset)
            );
        }
    }
}
