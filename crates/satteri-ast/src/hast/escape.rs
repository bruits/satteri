//! HTML escaping for the HAST renderer.
//!
//! The SWAR scanner, the needle folds, and the overlapping tail read are ported
//! from ox-content and adapted to the needle sets `hast-util-to-html` uses:
//! <https://github.com/ubugeeei-prod/ox-content/blob/30d64f2b5daec595150c58f4a47aa96bf9b2b056/crates/ox_content_renderer/src/html/escape.rs>
//!
//! Copyright (c) 2024 ubugeeei, MIT License. Permission is hereby granted, free
//! of charge, to any person obtaining a copy of this software and associated
//! documentation files (the "Software"), to deal in the Software without
//! restriction, including without limitation the rights to use, copy, modify,
//! merge, publish, distribute, sublicense, and/or sell copies of the Software,
//! and to permit persons to whom the Software is furnished to do so, subject to
//! the following conditions: The above copyright notice and this permission
//! notice shall be included in all copies or substantial portions of the
//! Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
//! EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
//! OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
//! ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! DEALINGS IN THE SOFTWARE.

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGH: u64 = 0x8080_8080_8080_8080;

const fn splat(byte: u8) -> u64 {
    (byte as u64) * ONES
}

/// Sets `0x80` in every zero byte lane of `word`. Only the *lowest* flagged
/// lane is trustworthy: a lane holding `0x01` also lights up when the lane
/// below it borrowed, so a spurious lane always sits above a genuine zero.
const fn has_zero(word: u64) -> u64 {
    word.wrapping_sub(ONES) & !word & HIGH
}

/// Flags lanes holding `&`, `<`, or `>`. The fold admits nothing extra because
/// `y | 0x02 == 0x3E` holds for exactly `<` (0x3C) and `>` (0x3E).
const fn body_text_mask(word: u64) -> u64 {
    has_zero((word | splat(0x02)) ^ splat(b'>')) | has_zero(word ^ splat(b'&'))
}

/// Flags lanes holding `&`, `"`, `'`, or `` ` ``. As above, `y | 0x01 == 0x27`
/// holds for exactly `&` (0x26) and `'` (0x27).
const fn attr_value_mask(word: u64) -> u64 {
    has_zero((word | splat(0x01)) ^ splat(b'\''))
        | has_zero(word ^ splat(b'"'))
        | has_zero(word ^ splat(b'`'))
}

const fn needle_table(needles: &[u8]) -> [bool; 256] {
    let mut table = [false; 256];
    let mut i = 0;
    while i < needles.len() {
        table[needles[i] as usize] = true;
        i += 1;
    }
    table
}

static BODY_TEXT_NEEDLES: [bool; 256] = needle_table(b"&<>");
static ATTR_VALUE_NEEDLES: [bool; 256] = needle_table(b"&\"'`");

const fn body_text_escape(byte: u8) -> Option<&'static str> {
    match byte {
        b'&' => Some("&amp;"),
        b'<' => Some("&lt;"),
        b'>' => Some("&gt;"),
        _ => None,
    }
}

const fn attr_value_escape(byte: u8) -> Option<&'static str> {
    match byte {
        b'&' => Some("&amp;"),
        b'"' => Some("&quot;"),
        b'\'' => Some("&#x27;"),
        b'`' => Some("&#x60;"),
        _ => None,
    }
}

/// Position and replacement of the first byte at or after `from` that needs
/// escaping, or `None` when the rest is clean. `bytes` must hold a whole word.
///
/// A sub-word remainder is covered by re-reading the final eight bytes with the
/// already-cleared lanes masked off, which is one more word test instead of a
/// byte walk. Masking forfeits `has_zero`'s lowest-lane guarantee, hence the
/// `escape_of` confirmation on every flagged lane.
#[inline]
fn next_escape(
    bytes: &[u8],
    from: usize,
    mask_of: impl Fn(u64) -> u64,
    escape_of: impl Fn(u8) -> Option<&'static str>,
) -> Option<(usize, &'static str)> {
    let len = bytes.len();
    debug_assert!(len >= 8, "the tail re-read needs a whole word behind `i`");
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let mut mask = mask_of(u64::from_le_bytes(*chunk));
        while mask != 0 {
            let at = i + (mask.trailing_zeros() / 8) as usize;
            if let Some(escaped) = escape_of(bytes[at]) {
                return Some((at, escaped));
            }
            mask &= mask - 1;
        }
        i += 8;
    }

    if i >= len {
        return None;
    }

    // The loop leaves `len - i` in `1..8`, so the shift below never reaches 64.
    let base = len - 8;
    let mut tail = [0u8; 8];
    tail.copy_from_slice(&bytes[base..]);
    let mut mask = mask_of(u64::from_le_bytes(tail)) & (u64::MAX << ((i - base) * 8));
    while mask != 0 {
        let at = base + (mask.trailing_zeros() / 8) as usize;
        if let Some(escaped) = escape_of(bytes[at]) {
            return Some((at, escaped));
        }
        mask &= mask - 1;
    }
    None
}

#[inline]
fn scan_bytes(
    bytes: &[u8],
    from: usize,
    needles: &[bool; 256],
    escape_of: impl Fn(u8) -> Option<&'static str>,
) -> Option<(usize, &'static str)> {
    let offset = bytes[from..].iter().position(|&b| needles[b as usize])?;
    let at = from + offset;
    escape_of(bytes[at]).map(|escaped| (at, escaped))
}

#[inline]
fn copy_around_escapes(
    out: &mut String,
    s: &str,
    next: impl Fn(usize) -> Option<(usize, &'static str)>,
) {
    let mut pending = 0usize;
    while let Some((at, escaped)) = next(pending) {
        if pending < at {
            out.push_str(&s[pending..at]);
        }
        out.push_str(escaped);
        pending = at + 1;
    }
    if pending < s.len() {
        out.push_str(&s[pending..]);
    }
}

#[inline]
fn escape_into(
    out: &mut String,
    s: &str,
    mask_of: impl Fn(u64) -> u64,
    needles: &[bool; 256],
    escape_of: impl Fn(u8) -> Option<&'static str>,
) {
    let bytes = s.as_bytes();
    if bytes.len() < 8 {
        copy_around_escapes(out, s, |from| scan_bytes(bytes, from, needles, &escape_of));
    } else {
        copy_around_escapes(out, s, |from| {
            next_escape(bytes, from, &mask_of, &escape_of)
        });
    }
}

/// Append `text` to `out`, escaped for HTML body text.
///
/// Encodes `&`, `<`, and `>`, matching `hast-util-to-html`'s default
/// serialization of text nodes.
pub fn escape_html_body_text(out: &mut String, text: &str) {
    escape_into(
        out,
        text,
        body_text_mask,
        &BODY_TEXT_NEEDLES,
        body_text_escape,
    );
}

/// Append `value` to `out`, escaped to appear inside a double-quoted HTML
/// attribute value, matching `hast-util-to-html`'s default "safe"
/// serialization.
///
/// Encodes `&`, `"`, `'`, and `` ` `` (backtick is escaped because some legacy
/// browsers treat it as an attribute-value delimiter). Unlike body-text
/// escaping, `<` and `>` are kept as-is since they're valid inside attribute
/// values.
pub fn escape_html_attr_value(out: &mut String, value: &str) {
    escape_into(
        out,
        value,
        attr_value_mask,
        &ATTR_VALUE_NEEDLES,
        attr_value_escape,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn char_by_char_attr(value: &str) -> String {
        let mut out = String::new();
        for c in value.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&#x27;"),
                '`' => out.push_str("&#x60;"),
                _ => out.push(c),
            }
        }
        out
    }

    fn check(s: &str) {
        let mut body = String::new();
        escape_html_body_text(&mut body, s);
        let mut expected_body = String::new();
        pulldown_cmark_escape::escape_html_body_text(&mut expected_body, s).unwrap();
        assert_eq!(body, expected_body, "body text: {s:?}");

        let mut attr = String::new();
        escape_html_attr_value(&mut attr, s);
        assert_eq!(attr, char_by_char_attr(s), "attr value: {s:?}");
    }

    #[test]
    fn appends_without_clobbering_the_buffer() {
        let mut out = String::from("prefix:");
        escape_html_body_text(&mut out, "a<b");
        escape_html_attr_value(&mut out, "|c\"d");
        assert_eq!(out, "prefix:a&lt;b|c&quot;d");
    }

    #[test]
    fn matches_reference_on_every_byte_alone_and_in_runs() {
        for b in 0..=255u8 {
            let unit = String::from_utf8_lossy(&[b]).into_owned();
            check(&unit);
            check(&unit.repeat(2));
            check(&unit.repeat(7));
            check(&unit.repeat(8));
            check(&unit.repeat(9));
            check(&unit.repeat(17));
            check(&format!("xxxx{unit}xxxx"));
        }
    }

    #[test]
    fn matches_reference_for_needles_at_every_offset_and_length() {
        for needle in ['&', '<', '>', '"', '\'', '`'] {
            for len in 0..=17usize {
                for at in 0..len {
                    let mut s: Vec<char> = std::iter::repeat_n('x', len).collect();
                    s[at] = needle;
                    check(&s.into_iter().collect::<String>());
                }
            }
        }
    }

    #[test]
    fn matches_reference_across_borrow_propagation_shapes() {
        // A folded needle leaves a 0x01 lane on the byte above it for exactly these pairs.
        for pair in ["<=", "<?", ">=", ">?", "\"#", "&'", "''", "`a"] {
            for len in 2..40usize {
                for at in 0..len - 1 {
                    let mut s = "x".repeat(len);
                    s.replace_range(at..at + 2, pair);
                    check(&s);
                }
            }
        }
    }

    #[test]
    fn matches_reference_on_multibyte_utf8_straddling_word_boundaries() {
        let pieces = ["é", "€", "🎉", "日", "a", "&", "<", "\"", "`"];
        for lead in 0..12usize {
            for a in pieces {
                for b in pieces {
                    let mut s = "z".repeat(lead);
                    s.push_str(a);
                    s.push_str(b);
                    s.push_str(a);
                    check(&s);
                    s.push_str("tail");
                    check(&s);
                }
            }
        }
    }

    #[test]
    fn matches_reference_on_pseudorandom_bytes() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let alphabet: Vec<u8> = (0x20u8..0x7f).chain(*b"&<>\"'`").collect();
        for len in 0..300usize {
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                s.push(alphabet[(state % alphabet.len() as u64) as usize] as char);
            }
            check(&s);
        }
    }
}
