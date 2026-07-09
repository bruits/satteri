---
cargo/satteri-pulldown-cmark: minor
cargo/satteri-napi: patch
npm/satteri: patch
---

Extends thread-local arena pooling from the no-plugin fast paths to the whole plugin pipeline. Handle-creating calls parse and convert into recycled arenas (`parse_into` joins `parse_no_positions_into` in `satteri-pulldown-cmark`), the fused apply/render/compile tails return their arenas to the pool, and `dropHandle` recycles instead of freeing. Pools are per-thread (worker threads keep their own), capped at 4 entries, and refuse arenas retaining more than 8 MiB so a large-document burst is returned to the allocator instead of pinned. Small-document plugin pipelines measure 5-7% faster; memory stays flat under sustained mixed workloads.
