//! Internal ASCII byte-set searching.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    _mm_loadu_si128, _mm256_and_si256, _mm256_broadcastsi128_si256, _mm256_cmpeq_epi8,
    _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_or_si256, _mm256_set1_epi8,
    _mm256_setzero_si256, _mm256_shuffle_epi8, _mm256_srli_epi16,
};

const AVX2_VECTOR_BYTES: usize = 32;
// Amortize dispatch and table setup over the four vectors processed per iteration.
const AVX2_BATCH_BYTES: usize = 4 * AVX2_VECTOR_BYTES;

/// Test a nibble-encoded ASCII set, or return `None` when acceleration is unavailable.
/// Each low-nibble entry holds matching ASCII high nibbles as bits.
#[doc(hidden)]
#[inline]
pub fn contains_ascii_byte_accelerated(bytes: &[u8], low: &[u8; 16]) -> Option<bool> {
    if bytes.len() < AVX2_BATCH_BYTES {
        return None;
    }
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: AVX2 is available and a full vector batch is readable.
        return Some(unsafe { contains_avx2(bytes, low) });
    }
    let _ = (bytes, low);
    None
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn contains_avx2(bytes: &[u8], low: &[u8; 16]) -> bool {
    // High nibbles 0..=7 map to their membership bit; 8..=15 reject non-ASCII bytes.
    const HIGH_NIBBLE_BITS: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0];
    // SAFETY: Both tables contain sixteen readable bytes.
    let low = _mm256_broadcastsi128_si256(unsafe { _mm_loadu_si128(low.as_ptr().cast()) });
    let high =
        _mm256_broadcastsi128_si256(unsafe { _mm_loadu_si128(HIGH_NIBBLE_BITS.as_ptr().cast()) });
    let nibble = _mm256_set1_epi8(0x0f);
    let zero = _mm256_setzero_si256();
    let matches = |at| {
        // SAFETY: Both loops below leave a full vector readable at `at`.
        let chunk = unsafe { _mm256_loadu_si256(bytes.as_ptr().add(at).cast()) };
        let lows = _mm256_shuffle_epi8(low, _mm256_and_si256(chunk, nibble));
        let highs =
            _mm256_shuffle_epi8(high, _mm256_and_si256(_mm256_srli_epi16(chunk, 4), nibble));
        _mm256_and_si256(lows, highs)
    };
    let mut at = 0;
    // Only existence matters: combine four independent vectors before testing
    // their mask, rather than branching and advancing once per vector.
    while bytes.len() - at >= AVX2_BATCH_BYTES {
        let first = _mm256_or_si256(matches(at), matches(at + AVX2_VECTOR_BYTES));
        let second = _mm256_or_si256(
            matches(at + 2 * AVX2_VECTOR_BYTES),
            matches(at + 3 * AVX2_VECTOR_BYTES),
        );
        let absent = _mm256_cmpeq_epi8(_mm256_or_si256(first, second), zero);
        if _mm256_movemask_epi8(absent) != -1 {
            return true;
        }
        at += AVX2_BATCH_BYTES;
    }
    while at < bytes.len() {
        // Rechecking a suffix avoids an out-of-bounds load or a scalar tail.
        let absent = _mm256_cmpeq_epi8(matches(at.min(bytes.len() - AVX2_VECTOR_BYTES)), zero);
        if _mm256_movemask_epi8(absent) != -1 {
            return true;
        }
        at += AVX2_VECTOR_BYTES;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{AVX2_BATCH_BYTES, contains_ascii_byte_accelerated};

    #[test]
    fn short_inputs_decline_acceleration() {
        for len in 0..AVX2_BATCH_BYTES {
            assert_eq!(
                contains_ascii_byte_accelerated(&vec![0; len], &[255; 16]),
                None
            );
        }
    }

    #[test]
    fn accelerated_search_matches_every_byte_at_vector_boundaries() {
        for low in [[0; 16], [255; 16], [0x55; 16], [0xaa; 16]] {
            for len in 128..193 {
                let mut bytes = vec![255; len + 31];
                for offset in [0, 1, 15, 31] {
                    let slice = &mut bytes[offset..offset + len];
                    for at in [
                        0,
                        1,
                        15,
                        16,
                        31,
                        32,
                        63,
                        64,
                        95,
                        96,
                        len - 33,
                        len - 32,
                        len - 17,
                        len - 16,
                        len - 1,
                    ] {
                        for byte in 0..=255u8 {
                            slice[at] = byte;
                            let expected =
                                byte < 128 && low[(byte & 15) as usize] & (1 << (byte >> 4)) != 0;
                            if let Some(found) = contains_ascii_byte_accelerated(slice, &low) {
                                assert_eq!(
                                    found, expected,
                                    "len={len}, offset={offset}, at={at}, byte={byte}"
                                );
                            }
                        }
                        slice[at] = 255;
                    }
                }
            }
        }
    }

    #[test]
    fn singleton_sets_distinguish_every_ascii_byte_and_reject_non_ascii() {
        for needle in 0..128u8 {
            let mut low = [0; 16];
            low[(needle & 15) as usize] = 1 << (needle >> 4);
            for len in [128, 129, 159, 160, 161, 255, 256, 257] {
                // Exact-sized allocations also make tail overreads visible to memory checkers.
                let mut bytes = vec![255; len].into_boxed_slice();
                for at in [0, 15, 31, len - 33, len - 32, len - 1] {
                    for byte in 0..=255u8 {
                        bytes[at] = byte;
                        if let Some(found) = contains_ascii_byte_accelerated(&bytes, &low) {
                            assert_eq!(
                                found,
                                byte == needle,
                                "needle={needle}, byte={byte}, len={len}, at={at}"
                            );
                        }
                    }
                    bytes[at] = 255;
                }
            }
        }
    }

    #[test]
    fn arbitrary_ascii_sets_match_scalar_search() {
        let mut state = 0x18273645u32;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        };
        for _ in 0..1024 {
            let low = std::array::from_fn(|_| next());
            let bytes: [u8; 321] = std::array::from_fn(|_| next());
            for start in 0..32 {
                let slice = &bytes[start..];
                let expected = slice
                    .iter()
                    .any(|&b| b < 128 && low[(b & 15) as usize] & (1 << (b >> 4)) != 0);
                if let Some(found) = contains_ascii_byte_accelerated(slice, &low) {
                    assert_eq!(found, expected);
                }
            }
        }
    }
}
