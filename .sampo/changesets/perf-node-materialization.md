---
npm/satteri: patch
---

Makes reading node content inside plugins substantially cheaper. Arena snapshots and the nodes materialized from them are now cached per handle and mutation epoch, so consecutive plugin passes over an unchanged tree share one serialized reader and one set of materialized nodes instead of rebuilding both per pass. Materialized nodes store their scalar fields (tag name, properties, value, position, attributes) eagerly as plain properties, keeping only `children` as a lazy getter shared per reader, which cuts the per-node accessor-install overhead that dominated deep reads. Child lists of matched nodes skip stub construction entirely once a same-epoch snapshot exists, since materializing the real node is then cheaper than deferring through per-field stub getters. Deep-reading plugin chains measure 40-75% faster wall-time depending on chain length; walk-only and mutation-only pipelines are unchanged.
