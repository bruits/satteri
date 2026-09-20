# satteri-arena

Arena allocator and binary buffer primitives for Sätteri, a high-performance Markdown and MDX processor.

`ArenaRead<K>` provides read-only access to resolved trees without requiring a particular node or string-pool layout. It returns node snapshots by value and also works through references, `Box`, `Arc`, and lock guards. Owned `Arena<K>` storage remains a `Vec<ArenaNode>`; its `get_node` and `get_node_mut` methods still return references.

## Development

Refer to [CONTRIBUTING.md](https://github.com/bruits/satteri/blob/main/CONTRIBUTING.md) for development setup and workflow details.
