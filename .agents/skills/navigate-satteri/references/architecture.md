# Satteri Architecture Reference

Deep dive into binary formats, key data structures, and file-level details.

## Binary Formats

### ArenaNode (52 bytes, `#[repr(C)]`)

Defined in `crates/satteri-arena/src/node.rs`. Every node in the arena is exactly this struct:

```
Offset  Size  Field
0       4     id: u32
4       1     node_type: u8
5       3     _padding
8       4     parent: u32 (u32::MAX = no parent)
12      4     start_offset: u32
16      4     end_offset: u32
20      4     start_line: u32
24      4     start_column: u32
28      4     end_line: u32
32      4     end_column: u32
36      4     children_start: u32 (index into Arena::children)
40      4     children_count: u32
44      4     data_offset: u32 (index into Arena::type_data)
48      4     data_len: u32
```

### StringRef (8 bytes, `#[repr(C)]`)

Defined in `crates/satteri-arena/src/node.rs`. A zero-copy reference into the arena's source string:

```
Offset  Size  Field
0       4     offset: u32
4       4     len: u32
```

Resolved via `Arena::get_str(string_ref)`. When a new string is synthesized (e.g., character references, plugin-generated content), it is appended to `Arena::source` via `alloc_string()` and a new `StringRef` is returned.

### Wire Format (Rust to JS)

Produced by `Arena::to_raw_buffer()` in `crates/satteri-arena/src/raw_buffer.rs`. Consumed by `MdastReader` / `HastReader` in TypeScript.

```
[Header: 44 bytes (11 x u32, native endian)]
  0: magic = b"MDAR" (0x5244414D)
  4: kind (1=Mdast, 2=Hast)
  8: node_struct_size (52)
 12: node_count
 16: nodes_offset
 20: children_count
 24: children_offset
 28: type_data_len
 32: type_data_offset
 36: source_len
 40: source_offset

[Nodes section: node_count * 52 bytes]
  Raw ArenaNode structs, cast from Vec<ArenaNode>

[Children section: children_count * 4 bytes]
  Raw u32 child IDs, cast from Vec<u32>

[Type data section: type_data_len bytes]
  Packed per-node binary data

[Source section: source_len bytes]
  UTF-8 source string
```

### Walk Result Format (Rust to JS)

Produced by `walk_mdast()` / `walk_hast()` in `crates/satteri-ast/src/walk.rs`. Read by `visitMdastHandle` / `visitHastHandle` in TypeScript.

```
[match_count: u32]
[match_index: count * 12 bytes]
  Per entry (12 bytes each):
    node_id: u32 (4B)
    sub_index: u8 (1B, which subscription matched)
    pad: u8 (1B)
    data_offset: u32 (4B, byte offset into data section)
    data_len: u16 (2B)

[data section: variable length]
  Per matched node (format depends on node type):
    node_data_len: u32  (JSON blob length, 0 if none)
    node_data: [u8; node_data_len]
    position: 6 x u32 (start_offset, end_offset, start_line, start_col, end_line, end_col)
    child_count: u16
    child_ids: child_count x u32
    ...type-specific data (heading depth, text value as StringRef, etc.)
```

The inline serialization avoids per-node NAPI round-trips. JS reads everything from the flat buffer via `DataView`.

### Command Buffer Format (JS to Rust)

Encoded by `CommandBuffer` class in `packages/satteri/src/command-buffer.ts`. Parsed by `apply_mdast_commands()` / `apply_hast_commands()` in `crates/satteri-plugin-api/src/js_commands.rs`.

```
Repeated command frames:
  command_type: u8
    0x01 = REMOVE
    0x05 = INSERT_BEFORE
    0x06 = INSERT_AFTER
    0x07 = PREPEND_CHILD
    0x08 = APPEND_CHILD
    0x09 = WRAP
    0x0B = REPLACE
    0x0C = SET_PROPERTY

For structural commands (INSERT/REPLACE/WRAP/CHILD):
  node_id: u32
  payload_type: u8
    0x10 = RAW_MARKDOWN (Rust re-parses the string)
    0x11 = RAW_HTML (inserted as Raw node)
    0x12 = SERDE_JSON (structured JsNode, deserialized in Rust)
  payload_len: u32
  payload: [u8; payload_len]

For REMOVE:
  node_id: u32

For SET_PROPERTY:
  node_id: u32
  field_name_len: u32
  field_name: UTF-8 string
  value_type: u8
    0 = PROP_STRING
    1 = PROP_BOOL_TRUE
    2 = PROP_BOOL_FALSE
    3 = PROP_SPACE_SEP (space-separated list, for HAST element properties)
    5 = PROP_INT
    6 = PROP_NULL
  For PROP_STRING/PROP_SPACE_SEP: value_len: u32, value: UTF-8 string
  For PROP_INT: value: u32
  For PROP_BOOL_TRUE/FALSE/NULL: no additional data
```

## MDAST Type Data Layouts

Each MDAST node type stores its data as packed bytes in `Arena::type_data`. Defined in `crates/satteri-ast/src/mdast/codec.rs`.

| Node Type | Layout | Size |
|-----------|--------|------|
| Heading | `depth: u8` | 1B |
| Text, InlineCode, Html, Yaml, Toml, InlineMath | `value: StringRef` | 8B |
| Link | `url: StringRef, title: StringRef` | 16B |
| Image | `url: StringRef, alt: StringRef, title: StringRef` | 24B |
| Code | `lang: StringRef, meta: StringRef, value: StringRef, fence_char: u8` | 28B |
| Definition | `url: StringRef, title: StringRef, identifier: StringRef, label: StringRef` | 32B |
| List | `start: u32, ordered: bool, spread: bool` | 8B |
| ListItem | `checked: u8 (0=unchecked, 1=checked, 2=not-task), spread: bool` | 2B |
| Table | `align_count: u32` + `align_count` x `ColumnAlign` (1B each) | 4B + N |
| LinkReference, ImageReference, FootnoteReference | `identifier: StringRef, label: StringRef, reference_kind: u8` | 20B |
| FootnoteDefinition | `identifier: StringRef, label: StringRef` | 16B |
| Math | `meta: StringRef, value: StringRef` | 16B |
| Expression (MdxFlow/TextExpression, MdxjsEsm) | `value: StringRef` | 8B |
| MdxJsxFlowElement, MdxJsxTextElement | `name: StringRef` + 16B header + N x 20B attrs | 24B + 20N |
| Directive (Container/Leaf/Text) | `name: StringRef` + N x 16B attrs (key: StringRef, value: StringRef) | 8B + 16N |
| Root, Paragraph, ThematicBreak, Blockquote, Emphasis, Strong, Break, Delete, TableRow, TableCell | (no type data) | 0B |

MDX JSX attribute entry (20 bytes):
```
kind: u8 (0=boolean, 1=literal, 2=expression, 3=spread)
_pad: 3 bytes
name: StringRef (8B)
value: StringRef (8B)
```

## HAST Type Data Layouts

Defined in `crates/satteri-ast/src/hast/codec.rs`.

| Node Type | Layout | Size |
|-----------|--------|------|
| Element | 16B header: `tag_name: StringRef, prop_count: u32, _pad: u32` + N x 20B props | 16B + 20N |
| Text, Comment, Raw | `value: StringRef` | 8B |
| MdxJsxElement, MdxJsxTextElement | Same as MDAST MDX JSX encoding | variable |
| MdxFlowExpression, MdxTextExpression, MdxEsm | `value: StringRef` | 8B |
| Root, Doctype | (no type data) | 0B |

HAST Element property entry (20 bytes):
```
name: StringRef (8B)
value_type: u8 (0=string, 1=bool_true, 2=bool_false, 3=space_sep, 4=comma_sep, 5=int, 6=null)
_pad: 3 bytes
value: StringRef (8B)
```

## Arena<K> Fields

Defined in `crates/satteri-arena/src/arena.rs`:

| Field | Type | Description |
|-------|------|-------------|
| `nodes` | `Vec<ArenaNode>` | Flat node storage. Index = node ID. |
| `children` | `Vec<u32>` | Flat child ID array. Nodes reference ranges via `children_start`/`children_count`. |
| `type_data` | `Vec<u8>` | Packed variable-length per-node data. Nodes reference slices via `data_offset`/`data_len`. |
| `source` | `String` | Original source text (+ any strings appended by `alloc_string`). StringRefs point into this. |
| `node_data` | `FxHashMap<u32, Vec<u8>>` | Optional per-node JSON blobs set by JS plugins (for `data` property / `hProperties` / etc.). |
| `mdx` | `bool` | Whether the arena was parsed with MDX extensions. |
| `parse_options` | `u32` | Bitflags of the parser options used. |

## ArenaBuilder<K> Stack Model

Defined in `crates/satteri-arena/src/builder.rs`. The builder maintains:

- `arena: Arena<K>` -- the arena being built
- `stack: Vec<(u32, u32)>` -- `(node_id, children_start)` pairs
- `pending_children: Vec<u32>` -- children collected for the current open node

`open_node(type)` pushes onto the stack. `close_node()` pops, copies pending children to `arena.children`, sets parent backlinks, and registers the closed node as a child of the new stack top. `add_leaf(type)` is a shortcut for nodes with no children.

## The Rebuild System

Defined in `crates/satteri-ast/src/rebuild.rs`. After plugin mutations are collected as `Patch` objects, `rebuild()` produces a new arena:

1. Build a `patch_map: FxHashMap<u32, Vec<Patch>>` from all patches
2. Validate: Wrap+Remove on same node is an error; Child patches on removed nodes error
3. DFS via `copy_node()`: for each node, apply patches in order:
   - InsertBefore patches first (emit sub-arenas as siblings)
   - If Remove: skip the node entirely
   - If Replace: emit the replacement sub-arena (optionally keeping original children)
   - If Wrap: emit wrapper root, then the original node as its child
   - Normal copy: emit the node, handle PrependChild/AppendChild
   - InsertAfter patches last
4. `emit_subtree()` copies sub-arenas into the new builder with StringRef remapping (source strings are concatenated, offsets adjusted)
5. Verify no patches were stranded on removed subtrees

`remap_string_refs()` dispatches to `remap_mdast_string_refs()` or `remap_hast_string_refs()` based on `K::KIND_TAG`. These functions know the byte layout of every node type's type_data to find and offset StringRef fields.

## The JS Plugin Execution Model

When a JS plugin runs (e.g., an MDAST plugin in `visitMdastHandle`):

1. **Subscription resolution**: Plugin method names (e.g., `heading`, `text`, `code`) are mapped to numeric node type IDs
2. **Rust walk**: `walkMdastHandle()` calls `walk_mdast()` which does a DFS, filtering by subscriptions. For HAST element visitors, tag name filters are applied in Rust -- only matching elements cross NAPI.
3. **Binary match buffer**: Matched nodes are serialized inline with all their type-specific data (avoiding per-node NAPI round-trips).
4. **JS dispatch**: For each match, `readMdastMatchedNode()` decodes the binary data into a lightweight JS object. Element nodes use `WalkElement` with prototype-based lazy getters (V8-optimized, ~16x faster than per-instance `defineProperty`). Children use `LazyChildResolver` -- serializes the handle once, then materializes via reader + materializer.
5. **Visitor call**: The visitor function receives `(Readonly<Node>, Context)`. It can:
   - Return `void` (no change)
   - Return a replacement `MdastNode` / `HastNode` (replace)
   - Return `{ raw: string }` (Rust re-parses as Markdown)
   - Return `{ rawHtml: string }` (Rust inserts as Raw node)
   - Call context methods: `ctx.setProperty()`, `ctx.removeNode()`, `ctx.insertBefore()`, etc.
6. **Command buffer**: Both return values and context methods write to `CommandBuffer` instances. These are merged and sent to Rust as a single `Uint8Array`.
7. **Rust apply**: `apply_mdast_commands()` / `apply_hast_commands()` parses the buffer. SET_PROPERTY mutations happen in-place. Structural mutations become `Patch` objects and trigger `rebuild()`.

## NAPI Handle Types

Defined in `crates/satteri-napi-binding/src/lib.rs`:

- `MdastHandle` = `External<Mutex<Arena<Mdast>>>` -- created by `create_mdast_handle`, consumed by `convert_mdast_to_hast_handle`
- `HastHandle` = `External<Mutex<Arena<Hast>>>` -- created by `convert_mdast_to_hast_handle` or `create_hast_handle`
- `AnyHandle` = `Either<&MdastHandle, &HastHandle>` -- used by kind-agnostic operations like `serialize_handle`

Handles are reference-counted by NAPI. `drop_handle()` explicitly releases the arena memory. Forgetting to drop causes a memory leak (the Rust memory stays alive until the JS garbage collector finalizes the External, which is non-deterministic).

## Key Crate Dependencies

```
satteri-arena (standalone, depends on: memchr, rustc-hash)
    |
    v
satteri-ast (depends on: satteri-arena, rustc-hash, serde, serde_json, pulldown-cmark-escape)
    |
    +---> satteri-pulldown-cmark (depends on: satteri-arena, satteri-ast, memchr, unicode-id-start)
    +---> satteri-mdxjs-rs (depends on: satteri-arena, satteri-ast, satteri-pulldown-cmark, oxc_*)
    +---> satteri-plugin-api (depends on: satteri-arena, satteri-ast, rustc-hash, serde, serde_json)
              |
              v
         satteri-napi-binding (depends on: all of the above, napi, napi-derive)
              |
              v
         satteri (facade, depends on: satteri-ast, satteri-pulldown-cmark, satteri-mdxjs-rs)
```
