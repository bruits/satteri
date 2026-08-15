//! Stack headroom for the tree recursions in `convert`, `hast::render`, and `patch`.

/// Runs `f` on a fresh stack segment when the current one is nearly exhausted.
#[cfg(not(target_family = "wasm"))]
#[inline(always)]
pub(crate) fn with_headroom<R>(f: impl FnOnce() -> R) -> R {
    const RED_ZONE: usize = 128 * 1024;
    const NEW_SEGMENT: usize = 1024 * 1024;
    stacker::maybe_grow(RED_ZONE, NEW_SEGMENT, f)
}

/// psm cannot report remaining stack on wasm, so growing there would allocate a segment per call.
#[cfg(target_family = "wasm")]
pub(crate) fn with_headroom<R>(f: impl FnOnce() -> R) -> R {
    f()
}
