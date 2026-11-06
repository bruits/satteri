---
cargo/satteri-arena: patch
---

Added `Arena::from_raw_buffer` to read a raw buffer back into an arena, and `to_raw_buffer_in` to write one with byte offsets. The raw buffer header now carries a format version, which `from_raw_buffer` checks; earlier versions can't read the new layout.
