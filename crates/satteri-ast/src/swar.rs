//! Word-at-a-time (SWAR) primitives shared by the crate's byte scanners.
//!
//! Ported from ox-content together with the escape scanner; see
//! [`crate::hast::escape`] for the upstream copyright notice.

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGH: u64 = 0x8080_8080_8080_8080;

pub(crate) const fn splat(byte: u8) -> u64 {
    (byte as u64) * ONES
}

/// Sets `0x80` in every zero byte lane of `word`. Only the *lowest* flagged
/// lane is trustworthy: a lane holding `0x01` also lights up when the lane
/// below it borrowed, so a spurious lane always sits above a genuine zero.
pub(crate) const fn has_zero(word: u64) -> u64 {
    word.wrapping_sub(ONES) & !word & HIGH
}
