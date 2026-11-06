---
cargo/satteri-napi: patch
npm/satteri: patch
---

Added a `byteOffsets` option to `serializeHandle`, for buffers read back in Rust with `Arena::from_raw_buffer`.
