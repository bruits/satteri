//! Character classes of the
//! [CommonMark CJK-friendly amendments](https://github.com/tats-u/markdown-cjk-friendly/blob/main/specification.md).

use crate::puncttable::is_punctuation;

/// Returns true when `c` is a CJK character: East Asian Width W, F or H and
/// not an emoji by default, or Hangul.
///
/// These ranges are based on Unicode version 18.
/// The source is https://github.com/tats-u/markdown-cjk-friendly/blob/caa37236186e0d294cd6da255a1bdde81dbc0f50/ranges.md.
pub(crate) fn is_cjk_character(c: char) -> bool {
    let cp = c as u32;
    if cp < 0x1100 {
        return false;
    }
    matches!(
        cp,
        0x1100..=0x11ff
            | 0x20a9
            | 0x2329..=0x232a
            | 0x2630..=0x2637
            | 0x268a..=0x268f
            | 0x2e80..=0x2e99
            | 0x2e9b..=0x2ef3
            | 0x2f00..=0x2fd5
            | 0x2ff0..=0x303e
            | 0x3041..=0x3096
            | 0x3099..=0x30ff
            | 0x3105..=0x312f
            | 0x3131..=0x318e
            | 0x3190..=0x31e5
            | 0x31ef..=0x321e
            | 0x3220..=0x3247
            | 0x3250..=0xa48c
            | 0xa490..=0xa4c6
            | 0xa960..=0xa97c
            | 0xac00..=0xd7a3
            | 0xd7b0..=0xd7c6
            | 0xd7cb..=0xd7fb
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe19
            | 0xfe30..=0xfe52
            | 0xfe54..=0xfe66
            | 0xfe68..=0xfe6b
            | 0xff01..=0xffbe
            | 0xffc2..=0xffc7
            | 0xffca..=0xffcf
            | 0xffd2..=0xffd7
            | 0xffda..=0xffdc
            | 0xffe0..=0xffe6
            | 0xffe8..=0xffee
            | 0x16fe0..=0x16fe4
            | 0x16ff0..=0x16ff6
            | 0x17000..=0x18cda
            | 0x18cff..=0x18d20
            | 0x18d80..=0x18df2
            | 0x18e00..=0x19191
            | 0x191a0..=0x191d2
            | 0x1aff0..=0x1aff3
            | 0x1aff5..=0x1affb
            | 0x1affd..=0x1affe
            | 0x1b000..=0x1b128
            | 0x1b132
            | 0x1b150..=0x1b152
            | 0x1b155
            | 0x1b164..=0x1b168
            | 0x1b170..=0x1b2fb
            | 0x1d300..=0x1d356
            | 0x1d360..=0x1d376
            | 0x1f1ae
            | 0x1f200
            | 0x1f202
            | 0x1f210..=0x1f219
            | 0x1f21b..=0x1f22e
            | 0x1f230..=0x1f231
            | 0x1f237
            | 0x1f23b
            | 0x1f240..=0x1f248
            | 0x1f260..=0x1f265
            | 0x1f7da
            | 0x20000..=0x3fffd
    )
}

/// The character before a delimiter run, as the amendments read it.
#[derive(Clone, Copy)]
pub(crate) struct PrecedingSequence {
    /// A Unicode punctuation sequence: punctuation, or punctuation and a
    /// variation selector.
    pub(crate) is_punctuation: bool,
    /// A CJK sequence, or one of the quotation marks U+FE01 asks to render
    /// full width.
    pub(crate) is_cjk: bool,
    /// An ideographic variation selector, which stands for the ideograph
    /// before it.
    pub(crate) is_ideographic_variation_selector: bool,
}

/// Classifies `prev`, the character before a delimiter run. A non-emoji
/// variation selector belongs to the character it follows, which
/// `before_prev` reads only then.
pub(crate) fn classify_preceding_sequence(
    prev: char,
    before_prev: impl FnOnce() -> Option<char>,
) -> PrecedingSequence {
    // A selector after whitespace or at the start of a line belongs to nothing.
    // The specification leaves that case open; it stays a character of its own,
    // as in remark-cjk-friendly.
    if is_non_emoji_general_variation_selector(prev)
        && let Some(base) = before_prev().filter(|base| !base.is_whitespace())
    {
        return PrecedingSequence {
            is_punctuation: is_punctuation(base),
            is_cjk: is_cjk_character(base) || is_cjk_ambiguous_punctuation_sequence(base, prev),
            is_ideographic_variation_selector: false,
        };
    }
    PrecedingSequence {
        is_punctuation: is_punctuation(prev),
        is_cjk: is_cjk_character(prev),
        is_ideographic_variation_selector: is_ideographic_variation_selector(prev),
    }
}

fn is_cjk_ambiguous_punctuation_sequence(c: char, selector: char) -> bool {
    selector == '\u{FE01}' && matches!(c, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}')
}

fn is_non_emoji_general_variation_selector(c: char) -> bool {
    matches!(c, '\u{FE00}'..='\u{FE0E}')
}

fn is_ideographic_variation_selector(c: char) -> bool {
    matches!(c, '\u{E0100}'..='\u{E01EF}')
}
