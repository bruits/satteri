//! Stack headroom for the tree recursions in `convert`, `hast::render`, and `patch`.

/// Levels between headroom checks; `PROBE_INTERVAL * frame_size` must stay inside `RED_ZONE`.
const PROBE_INTERVAL: u32 = 32;

/// Runs `f` on a fresh stack segment when the current one is nearly exhausted.
///
/// Checks only every `PROBE_INTERVAL` levels; stacker's first check reads `/proc/self/maps`.
#[cfg(not(target_family = "wasm"))]
#[inline(always)]
pub(crate) fn with_headroom<R>(depth: u32, f: impl FnOnce() -> R) -> R {
    if depth != 0 && depth.is_multiple_of(PROBE_INTERVAL) {
        grow_if_low(f)
    } else {
        f()
    }
}

/// Outlined so the check inlines into the recursion as a bare compare.
#[cfg(not(target_family = "wasm"))]
#[inline(never)]
fn grow_if_low<R>(f: impl FnOnce() -> R) -> R {
    const RED_ZONE: usize = 1024 * 1024;
    const NEW_SEGMENT: usize = 8 * 1024 * 1024;
    stacker::maybe_grow(RED_ZONE, NEW_SEGMENT, f)
}

/// psm cannot report remaining stack on wasm, so growing there would allocate a segment per call.
#[cfg(target_family = "wasm")]
pub(crate) fn with_headroom<R>(_depth: u32, f: impl FnOnce() -> R) -> R {
    f()
}
