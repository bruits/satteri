---
npm/satteri: patch
---

Reading node content inside plugins is substantially cheaper: pass snapshots and materialized nodes are cached and shared across plugin passes, and node fields are plain eager properties instead of lazy getters. Deep-reading plugin chains are 40-75% faster.
