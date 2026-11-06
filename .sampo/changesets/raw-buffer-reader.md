---
cargo/satteri-arena: patch
---

Added `Arena::from_raw_buffer` to read a raw buffer back into an arena, and `to_raw_buffer_in` to write one with byte offsets. Buffers now carry a format version, so a buffer from another version is refused instead of misread.
