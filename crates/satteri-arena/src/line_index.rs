/// Per-line UTF-16 offset and ASCII flag, folded into one record so a lookup
/// reads both from a single cache line rather than two parallel arrays.
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
/// code units (the unit JS strings index by), so `position` values slice the
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
    /// "Skip positions" mode for HTML/JS output paths that never read them: no
    /// per-line scan, and every lookup returns the all-zero sentinel.
    disabled: bool,
}

impl<'a> LineIndex<'a> {
    /// Construct a no-op index: no line scan, and `cursor()` returns trivial
    /// values. The source slice is still held so debug helpers keep working.
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
        // Real markdown averages ~21-27 bytes per line.
        let line_count_estimate = bytes.len() / 24 + 1;
        let mut offsets = Vec::with_capacity(line_count_estimate);
        offsets.push(0u32);
        if all_ascii {
            push_line_starts(bytes, &mut offsets);
            return LineIndex {
                source: bytes,
                line_offsets: offsets,
                line_meta: Vec::new(),
                all_ascii: true,
                disabled: false,
            };
        }
        push_line_starts(bytes, &mut offsets);
        let mut line_meta = Vec::with_capacity(offsets.len());
        let mut utf16_count = 0u32;
        for i in 0..offsets.len() {
            let start = offsets[i] as usize;
            let end = offsets.get(i + 1).map_or(bytes.len(), |&e| e as usize);
            let line = &bytes[start..end];
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
        }
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
        let mut cursor = LineIndexCursor {
            index: self,
            disabled: self.disabled,
            line_idx: 0,
            line_start: 0,
            line_len: u32::MAX,
            line_is_ascii: true,
            col_byte: 0,
            col_units: 0,
        };
        if !self.disabled {
            cursor.set_line(0);
        }
        cursor
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

/// A cursor over a `LineIndex` that remembers the line it last resolved, so
/// any lookup landing on the same line costs two compares and a subtract.
///
/// When offsets arrive in roughly ascending order (as they do from a parser),
/// line changes scan forward from the current line instead of binary-searching.
pub struct LineIndexCursor<'idx, 'src> {
    index: &'idx LineIndex<'src>,
    /// Mirrored off the index so the skip-positions early-out is one local load.
    disabled: bool,
    line_idx: usize,
    /// `line_len` saturates on the last line so any tail offset stays in the window.
    line_start: u32,
    line_len: u32,
    line_is_ascii: bool,
    /// Invariant: `col_units` is the UTF-16 length of `line_start..col_byte`.
    col_byte: u32,
    col_units: u32,
}

impl LineIndexCursor<'_, '_> {
    #[inline(always)]
    pub fn offset_to_line_col(&mut self, offset: u32) -> (u32, u32) {
        if self.disabled {
            return (0, 0);
        }
        let mut rel = offset.wrapping_sub(self.line_start);
        if rel >= self.line_len {
            self.move_to(offset);
            rel = offset - self.line_start;
        }
        let col = if self.line_is_ascii {
            rel + 1
        } else {
            self.utf16_col(offset) + 1
        };
        (self.line_idx as u32 + 1, col)
    }

    /// Convert a byte offset into the source to a UTF-16 offset;
    /// `position.start.offset` / `position.end.offset` are JS string
    /// indices, not bytes.
    #[inline]
    pub fn byte_to_utf16_offset(&mut self, byte_offset: u32) -> u32 {
        if self.index.all_ascii || self.disabled {
            return byte_offset;
        }
        if byte_offset.wrapping_sub(self.line_start) >= self.line_len {
            self.move_to(byte_offset);
        }
        let meta = self.index.line_meta[self.line_idx];
        if self.line_is_ascii {
            meta.utf16_offset + (byte_offset - self.line_start)
        } else {
            meta.utf16_offset + self.utf16_col(byte_offset)
        }
    }

    #[inline(never)]
    fn move_to(&mut self, offset: u32) {
        let offsets = &self.index.line_offsets;
        let len = offsets.len();
        let mut idx = self.line_idx;
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
        self.set_line(idx);
    }

    fn set_line(&mut self, idx: usize) {
        self.line_idx = idx;
        self.line_start = self.index.line_offsets[idx];
        self.line_len = match self.index.line_offsets.get(idx + 1) {
            Some(&next) => next - self.line_start,
            None => u32::MAX - self.line_start,
        };
        self.line_is_ascii = match self.index.line_meta.get(idx) {
            Some(meta) => meta.is_ascii,
            None => true,
        };
        self.col_byte = self.line_start;
        self.col_units = 0;
    }

    /// 0-based UTF-16 column of `offset` inside the current (non-ASCII) line.
    fn utf16_col(&mut self, offset: u32) -> u32 {
        if offset < self.col_byte {
            self.col_byte = self.line_start;
            self.col_units = 0;
        }
        self.col_units +=
            utf16_len_bytes(&self.index.source[self.col_byte as usize..offset as usize]);
        self.col_byte = offset;
        self.col_units
    }
}

/// Byte index of the last byte of every line ending in `bytes`. CommonMark
/// counts `\n`, `\r` and `\r\n` alike, so the `\r` of a CRLF is skipped and
/// the pair is reported once, at its `\n`.
pub fn line_ending_iter(bytes: &[u8]) -> impl Iterator<Item = usize> + '_ {
    memchr::memchr2_iter(b'\n', b'\r', bytes)
        .filter(move |&i| bytes[i] == b'\n' || bytes.get(i + 1) != Some(&b'\n'))
}

const SWAR_LO: u64 = 0x0101_0101_0101_0101;
const SWAR_HI: u64 = 0x8080_8080_8080_8080;

/// High bit set on every byte equal to `b` (exact, no false positives).
#[inline]
fn eq_mask(w: u64, b: u8) -> u64 {
    let x = w ^ (SWAR_LO * b as u64);
    x.wrapping_sub(SWAR_LO) & !x & SWAR_HI
}

/// Must agree with `line_ending_iter` on which bytes end a line (CRLF counts once).
fn push_line_starts(bytes: &[u8], offsets: &mut Vec<u32>) {
    // Word-at-a-time beats memchr2 here: markdown's line density makes memchr re-pay its SIMD setup per line.
    if memchr::memchr(b'\r', bytes).is_none() {
        push_newline_starts(bytes, offsets);
    } else {
        push_line_starts_cr(bytes, offsets);
    }
}

/// `\r`-free fast path: every `\n` ends a line, no CRLF pairing to check.
fn push_newline_starts(bytes: &[u8], offsets: &mut Vec<u32>) {
    let mut base = 0usize;
    for block in bytes.chunks_exact(32) {
        let m0 = eq_mask(u64::from_le_bytes(block[0..8].try_into().unwrap()), b'\n');
        let m1 = eq_mask(u64::from_le_bytes(block[8..16].try_into().unwrap()), b'\n');
        let m2 = eq_mask(u64::from_le_bytes(block[16..24].try_into().unwrap()), b'\n');
        let m3 = eq_mask(u64::from_le_bytes(block[24..32].try_into().unwrap()), b'\n');
        if m0 | m1 | m2 | m3 != 0 {
            for (k, m) in [m0, m1, m2, m3].into_iter().enumerate() {
                let mut hits = m;
                while hits != 0 {
                    let j = base + k * 8 + (hits.trailing_zeros() as usize >> 3);
                    hits &= hits - 1;
                    offsets.push(j as u32 + 1);
                }
            }
        }
        base += 32;
    }
    for (j, &b) in bytes.iter().enumerate().skip(base) {
        if b == b'\n' {
            offsets.push(j as u32 + 1);
        }
    }
}

fn push_line_starts_cr(bytes: &[u8], offsets: &mut Vec<u32>) {
    let mut base = 0usize;
    for chunk in bytes.chunks_exact(8) {
        let w = u64::from_le_bytes(chunk.try_into().unwrap());
        let mut hits = eq_mask(w, b'\n') | eq_mask(w, b'\r');
        while hits != 0 {
            let j = base + (hits.trailing_zeros() as usize >> 3);
            hits &= hits - 1;
            if bytes[j] == b'\n' || bytes.get(j + 1) != Some(&b'\n') {
                offsets.push(j as u32 + 1);
            }
        }
        base += 8;
    }
    for j in base..bytes.len() {
        let b = bytes[j];
        if b == b'\n' || (b == b'\r' && bytes.get(j + 1) != Some(&b'\n')) {
            offsets.push(j as u32 + 1);
        }
    }
}

/// UTF-16 length of a UTF-8 byte slice. Continuation bytes (`0b10xxxxxx`)
/// don't count; a 4-byte sequence (lead byte ≥ `0xF0`) is an astral code
/// point (a surrogate pair, two units).
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
    fn lone_carriage_return_ends_a_line() {
        let idx = LineIndex::from_source("a\rb");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1)); // a
        assert_eq!(c.offset_to_line_col(1), (1, 2)); // \r
        assert_eq!(c.offset_to_line_col(2), (2, 1)); // b
        assert_eq!(c.offset_to_line_col(3), (2, 2)); // end
    }

    #[test]
    fn crlf_is_one_line_ending() {
        let idx = LineIndex::from_source("a\r\nb");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(1), (1, 2)); // \r
        assert_eq!(c.offset_to_line_col(2), (1, 3)); // \n
        assert_eq!(c.offset_to_line_col(3), (2, 1)); // b
    }

    #[test]
    fn mixed_line_endings() {
        let idx = LineIndex::from_source("a\r\nb\rc\nd");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1)); // a
        assert_eq!(c.offset_to_line_col(3), (2, 1)); // b
        assert_eq!(c.offset_to_line_col(5), (3, 1)); // c
        assert_eq!(c.offset_to_line_col(7), (4, 1)); // d
    }

    #[test]
    fn line_ending_iter_counts_crlf_once() {
        let collect = |s: &[u8]| line_ending_iter(s).collect::<Vec<_>>();
        assert_eq!(collect(b"a\nb\nc"), [1, 3]);
        assert_eq!(collect(b"a\rb\rc"), [1, 3]);
        assert_eq!(collect(b"a\r\nb\r\nc"), [2, 5]);
        assert_eq!(collect(b"a\r\rb"), [1, 2]);
        assert_eq!(collect(b"a\n\rb"), [1, 2]);
        assert!(collect(b"abc").is_empty());
    }

    #[test]
    fn carriage_return_at_document_edges() {
        let idx = LineIndex::from_source("\ra\r");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(0), (1, 1)); // leading \r
        assert_eq!(c.offset_to_line_col(1), (2, 1)); // a
        assert_eq!(c.offset_to_line_col(3), (3, 1)); // past the trailing \r
    }

    #[test]
    fn consecutive_carriage_returns() {
        let idx = LineIndex::from_source("a\r\r\rb");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(2), (2, 1));
        assert_eq!(c.offset_to_line_col(3), (3, 1));
        assert_eq!(c.offset_to_line_col(4), (4, 1)); // b
    }

    #[test]
    fn carriage_return_in_non_ascii_source() {
        // "ð" is 2 bytes, 1 unit; the per-line UTF-16 bookkeeping has to split
        // on the lone \r too, not just on \n.
        let idx = LineIndex::from_source("a\rð\rb");
        let mut c = idx.cursor();
        assert_eq!(c.offset_to_line_col(2), (2, 1)); // ð
        assert_eq!(c.offset_to_line_col(5), (3, 1)); // b
        assert_eq!(c.byte_to_utf16_offset(2), 2); // ð
        assert_eq!(c.byte_to_utf16_offset(5), 4); // b
        for byte_offset in [0u32, 1, 2, 4, 5, 6] {
            let (line, col) = c.offset_to_line_col(byte_offset);
            assert_eq!(
                idx.utf16_offset_at(line, col),
                c.byte_to_utf16_offset(byte_offset)
            );
        }
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

    #[test]
    fn backward_lookups_rescan_correctly() {
        // Descending offsets on a multibyte line force the incremental
        // column progress to reset; both directions must agree.
        let idx = LineIndex::from_source("aあbいc\nxyz");
        let mut c = idx.cursor();
        let forward: Vec<_> = [0u32, 1, 4, 5, 8, 9]
            .iter()
            .map(|&o| c.offset_to_line_col(o))
            .collect();
        let backward: Vec<_> = [9u32, 8, 5, 4, 1, 0]
            .iter()
            .map(|&o| c.offset_to_line_col(o))
            .collect();
        let mut reversed = backward.clone();
        reversed.reverse();
        assert_eq!(forward, reversed);
    }

    #[test]
    fn fused_scan_matches_line_ending_iter() {
        let cases = [
            "",
            "a",
            "\n",
            "\r",
            "\r\n",
            "\n\r",
            "aaaaaaa\nbbbbbbbb\r\ncc",
            "ð12345\n1234567ð\nxxxxxxxx\ryyyyyyyy\r\n😀abcdefgh\nz",
            "abcdefg\r\nhijklmn\r\nðpqrst\nuvwxyz",
            "é\né\né\né\né\né\né\né\n",
            "seven77\rñ\r\nlong ascii line that spans several words before its end\nð",
            "ascii tail line with the multibyte character é hiding near the end\n1234567",
            "a long pure ascii line that comfortably crosses one 32-byte block boundary\nsecond line with é multibyte content also crossing a block boundary here\nand a trailing ascii line long enough to cross yet another block boundary\n",
            "0123456789012345678901234567890\n0123456789012345678901234567890\nx😀\n0123456789012345678901234567890\ntail-é",
        ];
        for src in cases {
            let idx = LineIndex::from_source(src);
            let mut expect = vec![0u32];
            for nl in line_ending_iter(src.as_bytes()) {
                expect.push(nl as u32 + 1);
            }
            assert_eq!(idx.line_offsets, expect, "offsets for {src:?}");
            if idx.all_ascii {
                assert!(idx.line_meta.is_empty());
                continue;
            }
            assert_eq!(idx.line_meta.len(), idx.line_offsets.len());
            let bytes = src.as_bytes();
            let mut utf16_count = 0u32;
            for (i, &start) in idx.line_offsets.iter().enumerate() {
                let end = idx
                    .line_offsets
                    .get(i + 1)
                    .map_or(bytes.len(), |&e| e as usize);
                let line = &bytes[start as usize..end];
                assert_eq!(idx.line_meta[i].is_ascii, line.is_ascii(), "line {i} of {src:?}");
                assert_eq!(idx.line_meta[i].utf16_offset, utf16_count, "line {i} of {src:?}");
                utf16_count += utf16_len_bytes(line);
            }
        }
    }
}
