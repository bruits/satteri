# satteri-arena

Arena allocator and binary buffer primitives for Sätteri, a high-performance Markdown and MDX processor.

`Document<'a, K>` stores resolved MDAST or HAST nodes with borrowed or owned source text. `Arena<K>` is the owned form of the same type. `into_owned()` copies borrowed source text into the string pool without rebuilding nodes or changing references.

Use `source()` for the original input and `get_str(StringRef)` for values: a borrowed document's `string_pool` contains only computed strings. `into_reusable()` and `rebind(source)` discard the tree and metadata while retaining buffers; use `into_owned()` to preserve the tree beyond the input's lifetime.

`DocumentBuilder` constructs either ownership form; `ArenaBuilder` names its owned form. Position setters and leaf construction take `NodePosition` rather than separate offset, line, and column arguments. The JS API and wire layout are unchanged.

## Development

Refer to [CONTRIBUTING.md](https://github.com/bruits/satteri/blob/main/CONTRIBUTING.md) for development setup and workflow details.
