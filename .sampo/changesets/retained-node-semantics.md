---
npm/satteri: minor
---

Nodes retained past their visitor pass now read as the tree looked during that pass instead of unconditionally throwing. Any in-pass node-content read pins the pass's arena snapshot, and retained nodes resolve against it even after later plugins mutate the tree; with no pinned snapshot, reads still work as long as nothing has mutated or freed the arena yet. The retention error only remains for the genuinely unrecoverable case (an untouched node first read after the tree changed) and now explains how to avoid it. Retained nodes keep their pass snapshot alive until collected, so copying the data you need (`structuredClone(node)`) is still the cheaper way to keep results around.
