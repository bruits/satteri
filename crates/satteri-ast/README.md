# satteri-ast

MDAST and HAST node types, codecs, tree operations, and conversion for Sätteri, a high-performance Markdown and MDX processor.

For subtree serialization, `hast::render_node_with_options` accepts `RenderOptions` to track SVG attribute casing separately from the content namespace. Use `RenderOptions::for_children(tag)` when descending through elements; it preserves HTML text and void-element rules inside SVG `foreignObject`, `desc`, and `title` elements.

## Development

Refer to [CONTRIBUTING.md](https://github.com/bruits/satteri/blob/main/CONTRIBUTING.md) for development setup and workflow details.
