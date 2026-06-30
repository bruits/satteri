//! The node registry: the single source of truth for every MDAST and HAST node
//! type. Each [`Node`] declares its tree, tag, Rust enum variant, AST name, and
//! (for fixed-field leaf types) its `type_data` layout.
//!
//! Everything downstream is generated from this one table:
//!   * the `MdastNodeType` / `HastNodeType` enums (`generated/node_types.rs`),
//!   * the TS name maps and visitor keys (`generated/node-types.ts`),
//!   * the walk serializers + layout decoders (`walk_type_data.rs`, `layout.ts`),
//!   * the MDAST set-property slot dispatch (`prop_slots.rs`),
//!   * compile-time layout assertions (`assert_layouts.rs`).
//!
//! Add a node here once; every downstream list is regenerated from it.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tree {
    Mdast,
    Hast,
}

/// How a field crosses the walk wire (Rust -> JS).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// `StringRef` resolved to an inline string with a `u16` length prefix.
    Str16,
    /// `StringRef` resolved to an inline string with a `u32` length prefix
    /// (for `value` fields, which can be large).
    Str32,
    /// A single stored byte, copied verbatim.
    U8,
}

/// How the decoded value is surfaced to JS.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Js {
    /// String; an empty string stays `""`.
    Str,
    /// String; an empty string becomes `null` (e.g. `title`, `lang`).
    StrNull,
    /// Numeric byte (e.g. `depth`).
    Num,
    /// Byte mapped through an enum value list (e.g. `referenceType`).
    Enum(&'static [&'static str]),
    /// Present on the wire but not assigned to the JS node (e.g. the kind byte
    /// on `footnoteReference`, which the mdast spec does not expose).
    Skip,
    /// A constant byte written on encode, absent from the wire and JS (e.g. the
    /// `fence_char` on `code`, which only affects rendering).
    Const(u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// JS property name (ignored when `js_kind` is [`Js::Skip`]).
    pub js: &'static str,
    /// Byte offset of the field within the node's `type_data`.
    pub offset: usize,
    pub wire: Wire,
    pub js_kind: Js,
    /// Fallback byte when `type_data` is shorter than `offset` (U8 fields only).
    pub u8_default: u8,
    /// Restore MDX phantom-space sentinels on decode (expression/JSX values).
    pub phantom: bool,
}

/// How the set-property dispatch writes a [`SetSlot`]. Fixed [`Field`]s derive
/// their slot kind from `wire`/`js_kind`; these cover the stored scalars that
/// never cross the walk wire as fields.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// 8-byte `StringRef` written in place.
    Str,
    /// Single numeric byte (derived from [`Js::Num`] fields; no declarations).
    U8,
    /// u32 little-endian scalar.
    U32,
    /// bool byte (0/1).
    Bool,
    /// Tri-state checked byte: true=1, false=0, null=2.
    CheckedTri,
    /// Byte mapped through an enum value list.
    Enum8(&'static [&'static str]),
}

/// A settable `type_data` slot consumed only by the set-property dispatch
/// (`prop_slots.rs`) — never part of the walk/op-stream layouts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SetSlot {
    /// JS property name.
    pub js: &'static str,
    /// Byte offset within the node's stored `type_data`.
    pub offset: usize,
    pub kind: Slot,
}

/// How a tail's decoded head + items assemble into JS node fields. `None` keeps
/// the type's decode hand-written — used where the item shape isn't a plain
/// string map (MDX JSX's attribute-kind dispatch, HAST element properties).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TailJs {
    /// Items become a `{ [key]: value }` object on `node[attrs_key]`, keyed by
    /// the item fields named `key` and `value` (directive attributes). `codec`
    /// is the arena encoder the generated op-stream `finalize` calls with the
    /// head string and the interned `(key, value)` pairs.
    Map {
        attrs_key: &'static str,
        key: &'static str,
        value: &'static str,
        codec: &'static str,
    },
    /// Items carry a `kind` byte (`MDX_ATTR_*`) and assemble into a typed
    /// attribute array on `node[attrs_key]`, with a nullable head `name`. The
    /// per-item dispatch is the shared `decodeMdxJsxAttr` (decode) /
    /// `intern_mdx_jsx_attrs` (encode); `codec` is the arena encoder. MDX JSX
    /// only, so the generated encode arm and its imports are `mdx`-gated.
    JsxAttrs {
        attrs_key: &'static str,
        codec: &'static str,
    },
    /// HAST element properties: items `(name, kind, value)` keyed by `name`,
    /// where the bool kinds (`PROP_BOOL_TRUE`/`FALSE`) carry no value. Decodes
    /// to a `properties` object via the shared `decodeElementProp`; `codec` is
    /// the `hast::codec` arena encoder. The head is the element `tagName`.
    ElementProps {
        attrs_key: &'static str,
        codec: &'static str,
    },
    /// A head-less list of single enum bytes (table column alignment). Decodes
    /// to an array on `node[attrs_key]` via the shared `decodeColumnAlign`;
    /// encodes from the collector's `align` bytes via `codec`.
    ByteList {
        attrs_key: &'static str,
        codec: &'static str,
    },
}

/// A variable-length list tail on the walk wire: `head` fields, then a `u16`
/// item count (the stored u32 at `count_offset`, clamped to `u16::MAX`) and
/// `count` fixed-stride items starting at `items_offset`. Item field offsets
/// are relative to the item base and pinned against `entry_struct` by
/// [`check_struct_layouts`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Tail {
    /// Fields written before the count (the element/directive name).
    pub head: &'static [Field],
    pub count_offset: usize,
    pub items_offset: usize,
    /// Stored item stride; must equal the entry struct's size.
    pub stride: usize,
    pub item: &'static [Field],
    /// The codec entry struct ([`MDAST_STRUCTS`] / [`HAST_STRUCTS`]) backing one
    /// stored item, or `None` for a raw single-byte item list (table align) with
    /// no struct — pinned only by `stride == 1`.
    pub entry_struct: Option<&'static str>,
    /// The codec header struct backing the stored bytes before the items:
    /// `count_offset` must land on a declared struct offset, `items_offset`
    /// must equal the struct's size, and head fields must land on declared
    /// offsets. `None` only for the struct-less table tail.
    pub head_struct: Option<&'static str>,
    /// JS assembly for the generated decoder; `None` leaves decode hand-written.
    pub js: Option<TailJs>,
}

pub struct Node {
    /// Kept for readability of the registry; the two tree tables are already
    /// split, so the generators don't read it back.
    #[allow(dead_code)]
    pub tree: Tree,
    pub tag: u8,
    /// Rust enum variant identifier (may differ from `name`, e.g. HAST
    /// `MdxJsxElement` whose name is `"mdxJsxFlowElement"`).
    pub variant: &'static str,
    /// Canonical AST/JS name.
    pub name: &'static str,
    /// Fixed `type_data` fields. Empty for container / no-data nodes and for
    /// `custom` nodes (whose variable-length codec stays hand-written).
    pub fields: &'static [Field],
    /// Variable-length `type_data` handled by hand-written code on both sides.
    pub custom: bool,
    /// Extra settable slots for the set-property dispatch: the custom nodes'
    /// fixed-offset scalars, plus stored bytes the walk wire hides
    /// (footnoteReference's kind byte). Non-skip/const [`Field`]s are settable
    /// already and don't repeat here.
    pub set_slots: &'static [SetSlot],
    /// Generated head+count+items walk tail of a `custom` node. The Rust walk
    /// serializer is always generated from this; when `tail.js` is set, the
    /// op-stream encode (`finalize`) and the walk decode are too. The stored
    /// arena codec (`encode_*_data`) stays hand-written, pinned by the asserts.
    pub tail: Option<Tail>,
}

/// One fixed-field wire layout shared by every tag with the same field list.
pub struct Layout {
    pub tags: Vec<u8>,
    pub fields: &'static [Field],
}

/// A codec struct whose memory layout the generated assertions pin down.
pub struct ArenaStruct {
    pub rust: &'static str,
    pub size: usize,
    pub offsets: &'static [(&'static str, usize)],
}

const fn s16(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str16,
        js_kind: Js::Str,
        u8_default: 0,
        phantom: false,
    }
}
const fn s16n(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str16,
        js_kind: Js::StrNull,
        u8_default: 0,
        phantom: false,
    }
}
const fn s32(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str32,
        js_kind: Js::Str,
        u8_default: 0,
        phantom: false,
    }
}
/// A `value` field that carries MDX phantom-space sentinels.
const fn s32p(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str32,
        js_kind: Js::Str,
        u8_default: 0,
        phantom: true,
    }
}
const fn num(js: &'static str, offset: usize, default: u8) -> Field {
    Field {
        js,
        offset,
        wire: Wire::U8,
        js_kind: Js::Num,
        u8_default: default,
        phantom: false,
    }
}
const fn enum8(js: &'static str, offset: usize, values: &'static [&'static str]) -> Field {
    Field {
        js,
        offset,
        wire: Wire::U8,
        js_kind: Js::Enum(values),
        u8_default: 0,
        phantom: false,
    }
}
const fn skip8(offset: usize) -> Field {
    Field {
        js: "",
        offset,
        wire: Wire::U8,
        js_kind: Js::Skip,
        u8_default: 0,
        phantom: false,
    }
}
/// A constant byte at `offset`, written only on encode (absent from wire/JS).
const fn konst(js: &'static str, offset: usize, value: u8) -> Field {
    Field {
        js,
        offset,
        wire: Wire::U8,
        js_kind: Js::Const(value),
        u8_default: 0,
        phantom: false,
    }
}

const fn sl(js: &'static str, offset: usize, kind: Slot) -> SetSlot {
    SetSlot { js, offset, kind }
}

const REF_KINDS: &[&str] = &["shortcut", "collapsed", "full"];

/// `ListData`: start u32 @0, ordered @4, spread @5.
const LIST_SLOTS: &[SetSlot] = &[
    sl("start", 0, Slot::U32),
    sl("ordered", 4, Slot::Bool),
    sl("spread", 5, Slot::Bool),
];
/// `ListItemData`: checked tri-state @0, spread @1.
const LIST_ITEM_SLOTS: &[SetSlot] = &[
    sl("checked", 0, Slot::CheckedTri),
    sl("spread", 1, Slot::Bool),
];
/// `MdxJsxElementData`: name `StringRef` @0.
const MDX_JSX_SLOTS: &[SetSlot] = &[sl("name", 0, Slot::Str)];
/// `ReferenceData.reference_kind` stays settable on footnoteReference even
/// though the walk wire skips it there (the mdast spec hides it).
const FOOTNOTE_REF_SLOTS: &[SetSlot] = &[sl("referenceType", 16, Slot::Enum8(REF_KINDS))];

const VALUE: &[Field] = &[s32("value", 0)];
const EXPR_VALUE: &[Field] = &[s32p("value", 0)];
const MATH: &[Field] = &[s16n("meta", 0), s32("value", 8)];
const NONE: &[Field] = &[];

/// The head shared by every list tail: a name `StringRef` at offset 0.
const NAME_HEAD: &[Field] = &[s16("name", 0)];

/// MDX JSX elements: 16-byte header, then 20-byte attribute entries
/// (`encode_mdx_jsx_element_data`, shared by the MDAST and HAST codecs).
const MDX_JSX_TAIL: Tail = Tail {
    head: NAME_HEAD,
    count_offset: 8,
    items_offset: 16,
    stride: 20,
    item: &[num("kind", 0, 0), s16("name", 4), s32("value", 12)],
    entry_struct: Some("MdxJsxAttributeEntry"),
    head_struct: Some("MdxJsxElementData"),
    js: Some(TailJs::JsxAttrs {
        attrs_key: "attributes",
        codec: "encode_mdx_jsx_element_data",
    }),
};

/// Directives: 16-byte header, then 16-byte key/value attribute entries
/// (`encode_directive_data`).
const DIRECTIVE_TAIL: Tail = Tail {
    head: NAME_HEAD,
    count_offset: 8,
    items_offset: 16,
    stride: 16,
    item: &[s16("key", 0), s16("value", 8)],
    entry_struct: Some("DirectiveAttributeEntry"),
    head_struct: Some("DirectiveData"),
    js: Some(TailJs::Map {
        attrs_key: "attributes",
        key: "key",
        value: "value",
        codec: "encode_directive_data",
    }),
};

/// HAST elements: 16-byte header, then 20-byte property entries
/// (`encode_element_data`).
const ELEMENT_TAIL: Tail = Tail {
    head: &[s16("tagName", 0)],
    count_offset: 8,
    items_offset: 16,
    stride: 20,
    item: &[s16("name", 0), num("kind", 8, 0), s16("value", 12)],
    entry_struct: Some("PropertyEntry"),
    head_struct: Some("ElementData"),
    js: Some(TailJs::ElementProps {
        attrs_key: "properties",
        codec: "encode_element_data",
    }),
};

/// Table column alignment: a head-less list of single `ColumnAlign` bytes
/// (`align_count` u32 @0, then `count` bytes @4; `encode_table_data`).
const TABLE_TAIL: Tail = Tail {
    head: &[],
    count_offset: 0,
    items_offset: 4,
    stride: 1,
    item: &[num("align", 0, 0)],
    entry_struct: None,
    head_struct: None,
    js: Some(TailJs::ByteList {
        attrs_key: "align",
        codec: "encode_table_data",
    }),
};

use Tree::{Hast, Mdast};

/// A container or no-`type_data` node (no fields, not custom).
const fn c(tree: Tree, tag: u8, variant: &'static str, name: &'static str) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields: NONE,
        custom: false,
        set_slots: &[],
        tail: None,
    }
}
/// A leaf node with a fixed-field layout.
const fn n(
    tree: Tree,
    tag: u8,
    variant: &'static str,
    name: &'static str,
    fields: &'static [Field],
) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields,
        custom: false,
        set_slots: &[],
        tail: None,
    }
}
/// A fixed-field leaf node with extra walk-hidden settable slots.
const fn ns(
    tree: Tree,
    tag: u8,
    variant: &'static str,
    name: &'static str,
    fields: &'static [Field],
    set_slots: &'static [SetSlot],
) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields,
        custom: false,
        set_slots,
        tail: None,
    }
}
/// A custom node with settable fixed-offset `type_data` slots.
const fn xs(
    tree: Tree,
    tag: u8,
    variant: &'static str,
    name: &'static str,
    set_slots: &'static [SetSlot],
) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields: NONE,
        custom: true,
        set_slots,
        tail: None,
    }
}
/// A custom node whose walk tail is a generated head+count+items list.
const fn xt(tree: Tree, tag: u8, variant: &'static str, name: &'static str, tail: Tail) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields: NONE,
        custom: true,
        set_slots: &[],
        tail: Some(tail),
    }
}
/// [`xt`] with settable fixed-offset `type_data` slots.
const fn xts(
    tree: Tree,
    tag: u8,
    variant: &'static str,
    name: &'static str,
    tail: Tail,
    set_slots: &'static [SetSlot],
) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields: NONE,
        custom: true,
        set_slots,
        tail: Some(tail),
    }
}

pub const MDAST_NODES: &[Node] = &[
    c(Mdast, 0, "Root", "root"),
    c(Mdast, 1, "Paragraph", "paragraph"),
    n(Mdast, 2, "Heading", "heading", &[num("depth", 0, 1)]),
    c(Mdast, 3, "ThematicBreak", "thematicBreak"),
    c(Mdast, 4, "Blockquote", "blockquote"),
    xs(Mdast, 5, "List", "list", LIST_SLOTS),
    xs(Mdast, 6, "ListItem", "listItem", LIST_ITEM_SLOTS),
    n(Mdast, 7, "Html", "html", VALUE),
    n(
        Mdast,
        8,
        "Code",
        "code",
        &[
            s16n("lang", 0),
            s16n("meta", 8),
            s32("value", 16),
            konst("fence", 24, b'`'),
        ],
    ),
    n(
        Mdast,
        9,
        "Definition",
        "definition",
        &[
            s16("url", 0),
            s16n("title", 8),
            s16("identifier", 16),
            s16("label", 24),
        ],
    ),
    n(Mdast, 10, "Text", "text", VALUE),
    c(Mdast, 11, "Emphasis", "emphasis"),
    c(Mdast, 12, "Strong", "strong"),
    n(Mdast, 13, "InlineCode", "inlineCode", VALUE),
    c(Mdast, 14, "Break", "break"),
    n(
        Mdast,
        15,
        "Link",
        "link",
        &[s16("url", 0), s16n("title", 8)],
    ),
    n(
        Mdast,
        16,
        "Image",
        "image",
        &[s16("url", 0), s16("alt", 8), s16n("title", 16)],
    ),
    n(
        Mdast,
        17,
        "LinkReference",
        "linkReference",
        &[
            s16("identifier", 0),
            s16("label", 8),
            enum8("referenceType", 16, REF_KINDS),
        ],
    ),
    n(
        Mdast,
        18,
        "ImageReference",
        "imageReference",
        &[
            s16("identifier", 0),
            s16("label", 8),
            enum8("referenceType", 16, REF_KINDS),
            s16("alt", 20),
        ],
    ),
    n(
        Mdast,
        19,
        "FootnoteDefinition",
        "footnoteDefinition",
        &[s16("identifier", 0), s16("label", 8)],
    ),
    ns(
        Mdast,
        20,
        "FootnoteReference",
        "footnoteReference",
        &[s16("identifier", 0), s16("label", 8), skip8(16)],
        FOOTNOTE_REF_SLOTS,
    ),
    xt(Mdast, 21, "Table", "table", TABLE_TAIL),
    c(Mdast, 22, "TableRow", "tableRow"),
    c(Mdast, 23, "TableCell", "tableCell"),
    c(Mdast, 24, "Delete", "delete"),
    n(Mdast, 25, "Yaml", "yaml", VALUE),
    n(Mdast, 26, "Toml", "toml", VALUE),
    n(Mdast, 27, "Math", "math", MATH),
    // InlineMath shares Math's stored `MathData` (meta@0, value@8) but the mdast
    // spec gives it no `meta`, so only `value` is surfaced.
    n(Mdast, 28, "InlineMath", "inlineMath", &[s32("value", 8)]),
    xt(
        Mdast,
        30,
        "ContainerDirective",
        "containerDirective",
        DIRECTIVE_TAIL,
    ),
    xt(Mdast, 31, "LeafDirective", "leafDirective", DIRECTIVE_TAIL),
    xt(Mdast, 32, "TextDirective", "textDirective", DIRECTIVE_TAIL),
    c(Mdast, 33, "Superscript", "superscript"),
    c(Mdast, 34, "Subscript", "subscript"),
    xts(
        Mdast,
        100,
        "MdxJsxFlowElement",
        "mdxJsxFlowElement",
        MDX_JSX_TAIL,
        MDX_JSX_SLOTS,
    ),
    xts(
        Mdast,
        101,
        "MdxJsxTextElement",
        "mdxJsxTextElement",
        MDX_JSX_TAIL,
        MDX_JSX_SLOTS,
    ),
    n(
        Mdast,
        102,
        "MdxFlowExpression",
        "mdxFlowExpression",
        EXPR_VALUE,
    ),
    n(
        Mdast,
        103,
        "MdxTextExpression",
        "mdxTextExpression",
        EXPR_VALUE,
    ),
    n(Mdast, 104, "MdxjsEsm", "mdxjsEsm", EXPR_VALUE),
];

pub const HAST_NODES: &[Node] = &[
    c(Hast, 0, "Root", "root"),
    // The HAST fields/tails below drive only the generated Rust walk
    // serializer (`hast/generated/walk_type_data.rs`); the stored codecs and
    // the TS decoders (hast-visitor.ts) stay hand-written.
    xt(Hast, 1, "Element", "element", ELEMENT_TAIL),
    // text/comment/raw store a single value StringRef (`encode_text_data`).
    n(Hast, 2, "Text", "text", VALUE),
    n(Hast, 3, "Comment", "comment", VALUE),
    c(Hast, 4, "Doctype", "doctype"),
    n(Hast, 5, "Raw", "raw", VALUE),
    xt(Hast, 10, "MdxJsxElement", "mdxJsxFlowElement", MDX_JSX_TAIL),
    xt(
        Hast,
        11,
        "MdxJsxTextElement",
        "mdxJsxTextElement",
        MDX_JSX_TAIL,
    ),
    // The s32p/s32 split mirrors the TS side: expression values get MDX
    // phantom-space restoration, ESM does not. Both emit `write_str32`.
    n(
        Hast,
        12,
        "MdxFlowExpression",
        "mdxFlowExpression",
        EXPR_VALUE,
    ),
    n(Hast, 13, "MdxEsm", "mdxjsEsm", VALUE),
    n(
        Hast,
        14,
        "MdxTextExpression",
        "mdxTextExpression",
        EXPR_VALUE,
    ),
];

/// AST names that can't be handed in as op-stream replacement content (the
/// op-stream is the only structural encoding, so the visitor throws on these
/// rather than falling back). `root` is a document container, never a
/// replacement; `finalize_collector` (js_commands.rs) also has no arm for it.
/// A NEW node type must gain a finalize/generated encode arm or be listed here.
pub const MDAST_OPSTREAM_EXCLUDED: &[&str] = &["root"];
/// HAST twin (`finalize_hast_collector`); `doctype` has no finalize arm either.
pub const HAST_OPSTREAM_EXCLUDED: &[&str] = &["root", "doctype"];

/// Total stored `type_data` size for a field list: the max field extent,
/// rounded up to 4 when it holds any `StringRef` (matching the codec structs'
/// alignment).
pub fn layout_size(fields: &[Field]) -> usize {
    let mut max = 0usize;
    let mut has_ref = false;
    for f in fields {
        let size = match f.wire {
            Wire::Str16 | Wire::Str32 => {
                has_ref = true;
                8
            }
            Wire::U8 => 1,
        };
        max = max.max(f.offset + size);
    }
    if has_ref { max.div_ceil(4) * 4 } else { max }
}

/// Group a tree's fixed-field nodes into shared wire layouts (tags with an
/// identical field list collapse to one [`Layout`], in first-seen order).
pub fn layouts(nodes: &[Node]) -> Vec<Layout> {
    let mut out: Vec<Layout> = Vec::new();
    for node in nodes {
        if node.custom || node.fields.is_empty() {
            continue;
        }
        match out.iter_mut().find(|l| l.fields == node.fields) {
            Some(layout) => layout.tags.push(node.tag),
            None => out.push(Layout {
                tags: vec![node.tag],
                fields: node.fields,
            }),
        }
    }
    out
}

/// One generated walk tail shared by every tag with an identical [`Tail`].
pub struct TailLayout {
    pub tags: Vec<u8>,
    pub tail: Tail,
}

/// Group a tree's tail nodes the same way [`layouts`] groups fixed fields.
pub fn tail_layouts(nodes: &[Node]) -> Vec<TailLayout> {
    let mut out: Vec<TailLayout> = Vec::new();
    for node in nodes {
        let Some(tail) = node.tail else { continue };
        match out.iter_mut().find(|t| t.tail == tail) {
            Some(t) => t.tags.push(node.tag),
            None => out.push(TailLayout {
                tags: vec![node.tag],
                tail,
            }),
        }
    }
    out
}

pub const MDAST_STRUCTS: &[ArenaStruct] = &[
    ArenaStruct {
        rust: "HeadingData",
        size: 1,
        offsets: &[("depth", 0)],
    },
    ArenaStruct {
        rust: "ListData",
        size: 8,
        offsets: &[("start", 0), ("ordered", 4), ("spread", 5)],
    },
    ArenaStruct {
        rust: "ListItemData",
        size: 2,
        offsets: &[("checked", 0), ("spread", 1)],
    },
    ArenaStruct {
        rust: "CodeData",
        size: 28,
        offsets: &[("lang", 0), ("meta", 8), ("value", 16), ("fence_char", 24)],
    },
    ArenaStruct {
        rust: "MathData",
        size: 16,
        offsets: &[("meta", 0), ("value", 8)],
    },
    ArenaStruct {
        rust: "LinkData",
        size: 16,
        offsets: &[("url", 0), ("title", 8)],
    },
    ArenaStruct {
        rust: "ImageData",
        size: 24,
        offsets: &[("url", 0), ("alt", 8), ("title", 16)],
    },
    ArenaStruct {
        rust: "DefinitionData",
        size: 32,
        offsets: &[("url", 0), ("title", 8), ("identifier", 16), ("label", 24)],
    },
    ArenaStruct {
        rust: "ReferenceData",
        size: 20,
        offsets: &[("identifier", 0), ("label", 8), ("reference_kind", 16)],
    },
    ArenaStruct {
        rust: "FootnoteDefinitionData",
        size: 16,
        offsets: &[("identifier", 0), ("label", 8)],
    },
    ArenaStruct {
        rust: "DirectiveData",
        size: 16,
        offsets: &[("name", 0), ("attr_count", 8)],
    },
    ArenaStruct {
        rust: "DirectiveAttributeEntry",
        size: 16,
        offsets: &[("key", 0), ("value", 8)],
    },
    ArenaStruct {
        rust: "MdxJsxElementData",
        size: 16,
        offsets: &[("name", 0), ("attr_count", 8), ("explicit_jsx", 12)],
    },
    ArenaStruct {
        rust: "MdxJsxAttributeEntry",
        size: 20,
        offsets: &[("kind", 0), ("name", 4), ("value", 12)],
    },
];

/// HAST twin of [`MDAST_STRUCTS`], pinned by `hast/generated/assert_layouts.rs`.
pub const HAST_STRUCTS: &[ArenaStruct] = &[
    ArenaStruct {
        rust: "ElementData",
        size: 16,
        offsets: &[("tag_name", 0), ("prop_count", 8)],
    },
    ArenaStruct {
        rust: "PropertyEntry",
        size: 20,
        offsets: &[("name", 0), ("value_type", 8), ("value", 12)],
    },
];

/// Which [`MDAST_STRUCTS`] entry backs each fixed-field node's stored
/// `type_data`. Nodes absent here store a bare `StringRef` (`VALUE` /
/// `EXPR_VALUE`), pinned by the `size_of::<StringRef>() == 8` assertion.
const STRUCT_BY_NODE: &[(&str, &str)] = &[
    ("heading", "HeadingData"),
    ("code", "CodeData"),
    ("math", "MathData"),
    ("inlineMath", "MathData"),
    ("link", "LinkData"),
    ("image", "ImageData"),
    ("definition", "DefinitionData"),
    ("linkReference", "ReferenceData"),
    ("imageReference", "ReferenceData"),
    ("footnoteReference", "ReferenceData"),
    ("footnoteDefinition", "FootnoteDefinitionData"),
];

/// Which struct backs the set-property slots of `custom` nodes whose stored
/// layout has no `fields` and no tail head (list / listItem). Other slotted
/// nodes resolve through [`STRUCT_BY_NODE`] or their tail's `head_struct`.
const SET_SLOT_STRUCTS: &[(&str, &str)] = &[("list", "ListData"), ("listItem", "ListItemData")];

/// Stored byte width of one settable slot.
fn slot_width(kind: Slot) -> usize {
    match kind {
        Slot::Str => 8,
        Slot::U32 => 4,
        Slot::U8 | Slot::Bool | Slot::CheckedTri | Slot::Enum8(_) => 1,
    }
}

/// Cross-check the per-node field lists against [`MDAST_STRUCTS`], so the two
/// parallel tables can't drift apart silently. Field *names* aren't comparable
/// (JS camelCase vs Rust snake_case, e.g. `referenceType` / `reference_kind`),
/// so the check compares offsets and sizes:
///   * every node field inside the struct must start on a declared struct
///     offset and fit within the struct;
///   * sizes must match, unless the node has suffix fields past the struct
///     (`imageReference.alt` at 20 behind the 20-byte `ReferenceData`);
///   * struct-only fields need no node twin (`MathData.meta` on `inlineMath`,
///     which the mdast spec hides).
pub fn check_struct_layouts() {
    for (node_name, struct_name) in STRUCT_BY_NODE {
        let node = MDAST_NODES
            .iter()
            .find(|n| n.name == *node_name)
            .unwrap_or_else(|| panic!("STRUCT_BY_NODE: unknown node {node_name:?}"));
        let st = MDAST_STRUCTS
            .iter()
            .find(|s| s.rust == *struct_name)
            .unwrap_or_else(|| panic!("STRUCT_BY_NODE: unknown struct {struct_name:?}"));
        let size = layout_size(node.fields);
        let has_suffix = node.fields.iter().any(|f| f.offset >= st.size);
        if has_suffix {
            assert!(
                st.size <= size,
                "{node_name}: field layout size {size} is smaller than {struct_name} size {}",
                st.size
            );
        } else {
            assert_eq!(
                size, st.size,
                "{node_name}: field layout size {size} != {struct_name} size {}",
                st.size
            );
        }
        for f in node.fields {
            if f.offset >= st.size {
                continue;
            }
            let extent = f.offset
                + match f.wire {
                    Wire::Str16 | Wire::Str32 => 8,
                    Wire::U8 => 1,
                };
            assert!(
                extent <= st.size,
                "{node_name}: field {:?} (offset {}) straddles the end of {struct_name} (size {})",
                f.js,
                f.offset,
                st.size
            );
            assert!(
                st.offsets.iter().any(|&(_, off)| off == f.offset),
                "{node_name}: field {:?} at offset {} matches no {struct_name} field",
                f.js,
                f.offset
            );
        }
    }
    // A fixed-field node without a struct mapping must be a bare StringRef;
    // anything bigger needs an MDAST_STRUCTS pin and a STRUCT_BY_NODE entry.
    for node in MDAST_NODES {
        if node.custom
            || node.fields.is_empty()
            || STRUCT_BY_NODE.iter().any(|&(n, _)| n == node.name)
        {
            continue;
        }
        assert!(
            node.fields.len() == 1 && layout_size(node.fields) == 8,
            "{}: fixed-field node has no STRUCT_BY_NODE entry",
            node.name
        );
    }
    // Settable slots are pinned like fields: every slot must start on a
    // declared offset of the struct backing the node's stored layout and fit
    // within it, so reordering the codec struct can't silently corrupt
    // set-property writes.
    for node in MDAST_NODES.iter().chain(HAST_NODES) {
        if node.set_slots.is_empty() {
            continue;
        }
        let struct_name = STRUCT_BY_NODE
            .iter()
            .chain(SET_SLOT_STRUCTS)
            .find(|&&(n, _)| n == node.name)
            .map(|&(_, st)| st)
            .or_else(|| node.tail.and_then(|t| t.head_struct))
            .unwrap_or_else(|| panic!("{}: set slots are backed by no struct", node.name));
        let st = MDAST_STRUCTS
            .iter()
            .chain(HAST_STRUCTS)
            .find(|s| s.rust == struct_name)
            .unwrap_or_else(|| panic!("{}: unknown slot struct {struct_name:?}", node.name));
        for slot in node.set_slots {
            assert!(
                slot.offset + slot_width(slot.kind) <= st.size,
                "{}: set slot {:?} (offset {}) straddles the end of {} (size {})",
                node.name,
                slot.js,
                slot.offset,
                st.rust,
                st.size
            );
            assert!(
                st.offsets.iter().any(|&(_, off)| off == slot.offset),
                "{}: set slot {:?} at offset {} matches no {} field",
                node.name,
                slot.js,
                slot.offset,
                st.rust
            );
        }
    }
    // Tail items are pinned the same way: the stride must equal the entry
    // struct's size and every item field must start on a declared offset.
    // Tail heads are pinned against `head_struct`: the count must land on a
    // declared offset, the items must start exactly past the header, and head
    // fields must land on declared offsets.
    for node in MDAST_NODES.iter().chain(HAST_NODES) {
        let Some(tail) = node.tail else { continue };
        assert!(node.custom, "{}: tails require a custom codec", node.name);
        if let Some(head_struct) = tail.head_struct {
            let st = MDAST_STRUCTS
                .iter()
                .chain(HAST_STRUCTS)
                .find(|s| s.rust == head_struct)
                .unwrap_or_else(|| {
                    panic!("{}: unknown tail head struct {head_struct:?}", node.name)
                });
            assert_eq!(
                tail.items_offset, st.size,
                "{}: tail items offset {} != {} size {}",
                node.name, tail.items_offset, st.rust, st.size
            );
            assert!(
                st.offsets.iter().any(|&(_, off)| off == tail.count_offset),
                "{}: tail count offset {} matches no {} field",
                node.name,
                tail.count_offset,
                st.rust
            );
            for f in tail.head {
                assert!(
                    st.offsets.iter().any(|&(_, off)| off == f.offset),
                    "{}: tail head field {:?} at offset {} matches no {} field",
                    node.name,
                    f.js,
                    f.offset,
                    st.rust
                );
            }
        } else {
            assert!(
                tail.head.is_empty(),
                "{}: tail head fields require a head struct",
                node.name
            );
        }
        // A struct-less tail (table align) is a raw single-byte item list, pinned
        // only by `stride == 1`; it has no entry struct to cross-check.
        let Some(entry_struct) = tail.entry_struct else {
            assert_eq!(
                tail.stride, 1,
                "{}: struct-less tail must have stride 1",
                node.name
            );
            continue;
        };
        let st = MDAST_STRUCTS
            .iter()
            .chain(HAST_STRUCTS)
            .find(|s| s.rust == entry_struct)
            .unwrap_or_else(|| panic!("{}: unknown tail entry struct {entry_struct:?}", node.name));
        assert_eq!(
            tail.stride, st.size,
            "{}: tail stride {} != {} size {}",
            node.name, tail.stride, st.rust, st.size
        );
        for f in tail.item {
            let extent = f.offset
                + match f.wire {
                    Wire::Str16 | Wire::Str32 => 8,
                    Wire::U8 => 1,
                };
            assert!(
                extent <= st.size,
                "{}: tail item field {:?} (offset {}) straddles the end of {} (size {})",
                node.name,
                f.js,
                f.offset,
                st.rust,
                st.size
            );
            assert!(
                st.offsets.iter().any(|&(_, off)| off == f.offset),
                "{}: tail item field {:?} at offset {} matches no {} field",
                node.name,
                f.js,
                f.offset,
                st.rust
            );
        }
    }
    // Every pinned struct must be reachable from a node, or it drifted loose.
    for s in MDAST_STRUCTS.iter().chain(HAST_STRUCTS) {
        let in_tail = MDAST_NODES.iter().chain(HAST_NODES).any(|n| {
            n.tail
                .is_some_and(|t| t.entry_struct == Some(s.rust) || t.head_struct == Some(s.rust))
        });
        assert!(
            STRUCT_BY_NODE
                .iter()
                .chain(SET_SLOT_STRUCTS)
                .any(|&(_, st)| st == s.rust)
                || in_tail,
            "struct table entry {} is mapped to no node",
            s.rust
        );
    }
}

/// One wire constant; `doc` (operand layout or meaning) is emitted as a
/// trailing comment on both sides.
pub struct WireConst {
    pub name: &'static str,
    pub value: u8,
    pub doc: &'static str,
}

/// A table of wire constants, emitted into the Rust and TS `wire-constants`
/// modules.
pub struct WireTable {
    /// Table-level comment lines.
    pub doc: &'static [&'static str],
    /// `cfg` attribute for the Rust consts (the TS emit ignores it).
    pub cfg: Option<&'static str>,
    /// Render values as two-digit hex.
    pub hex: bool,
    pub consts: &'static [WireConst],
}

const fn wc(name: &'static str, value: u8, doc: &'static str) -> WireConst {
    WireConst { name, value, doc }
}

/// Op codes of the OPEN/CLOSE/field/REF/KEEP_CHILDREN/PROP stream the JS
/// `OpWriter` emits and `replay_opstream` (js_commands.rs) replays.
pub const OP_CODES: WireTable = WireTable {
    doc: &["Op-stream op codes (JS `OpWriter` -> Rust `replay_opstream`)."],
    cfg: None,
    hex: true,
    consts: &[
        wc("OP_OPEN", 0x01, "[type: u8]"),
        wc("OP_CLOSE", 0x02, ""),
        wc("OP_REF", 0x03, "[id: u32 LE] — splice an existing node"),
        wc(
            "OP_KEEP_CHILDREN",
            0x04,
            "splice the anchor node's original children",
        ),
        wc("OP_STR", 0x05, "[field: u8][len: u32 LE][utf8]"),
        wc("OP_U8", 0x06, "[field: u8][value: u8]"),
        wc("OP_U32", 0x07, "[field: u8][value: u32 LE]"),
        wc("OP_BOOL", 0x08, "[field: u8][0|1]"),
        wc("OP_DATA", 0x09, "[len: u32 LE][json utf8]"),
        wc(
            "OP_PROP",
            0x0a,
            "[name str][kind: u8][value str] — HAST element property",
        ),
        wc(
            "OP_ALIGN",
            0x0b,
            "[len: u32 LE][ColumnAlign bytes] — table column alignment",
        ),
    ],
};

/// Op-stream field ids (a single namespace across OP_STR/OP_U8/OP_U32/OP_BOOL).
pub const OP_FIELDS: WireTable = WireTable {
    doc: &["Op-stream field ids (single namespace across OP_STR/OP_U8/OP_U32/OP_BOOL)."],
    cfg: None,
    hex: false,
    consts: &[
        wc("OF_VALUE", 0, ""),
        wc("OF_URL", 1, ""),
        wc("OF_TITLE", 2, ""),
        wc("OF_ALT", 3, ""),
        wc("OF_LANG", 4, ""),
        wc("OF_META", 5, ""),
        wc("OF_IDENTIFIER", 6, ""),
        wc("OF_LABEL", 7, ""),
        wc("OF_NAME", 8, "directive / MDX JSX element name"),
        wc("OF_REFERENCE_TYPE", 9, ""),
        wc("OF_DEPTH", 10, ""),
        wc("OF_CHECKED", 11, ""),
        wc("OF_START", 12, ""),
        wc("OF_ORDERED", 13, ""),
        wc("OF_SPREAD", 14, ""),
        wc("OF_TAGNAME", 15, "HAST element tag name"),
        wc("OF_EXPLICIT", 16, "MDX JSX `_mdxExplicitJsx` flag"),
    ],
};

/// Command bytes of the JS `CommandBuffer` -> Rust `apply_*_commands` wire.
pub const COMMANDS: WireTable = WireTable {
    doc: &[
        "Command bytes (0x01–0x0F range). Each is followed by [nodeId: u32 LE];",
        "structural commands then carry [payloadType: u8][payload…].",
    ],
    cfg: None,
    hex: true,
    consts: &[
        wc("CMD_REMOVE", 0x01, ""),
        wc("CMD_INSERT_BEFORE", 0x05, ""),
        wc("CMD_INSERT_AFTER", 0x06, ""),
        wc("CMD_PREPEND_CHILD", 0x07, ""),
        wc("CMD_APPEND_CHILD", 0x08, ""),
        wc("CMD_WRAP", 0x09, ""),
        wc("CMD_REPLACE", 0x0b, ""),
        wc(
            "CMD_SET_PROPERTY",
            0x0c,
            "[valueType: u8][name str][value str], PROP_* value kinds",
        ),
        wc(
            "CMD_SET_CHILDREN",
            0x0d,
            "payload is a Root-wrapped child list",
        ),
    ],
};

/// Structural-command payload types.
pub const PAYLOADS: WireTable = WireTable {
    doc: &["Structural-command payload types (0x10+, a range distinct from commands)."],
    cfg: None,
    hex: true,
    consts: &[
        wc(
            "PAYLOAD_RAW_MARKDOWN",
            0x10,
            "[len: u32 LE][utf8] — re-parsed as markdown",
        ),
        wc(
            "PAYLOAD_RAW_HTML",
            0x11,
            "[len: u32 LE][utf8] — re-parsed as HTML/MDX",
        ),
        wc(
            "PAYLOAD_OPSTREAM",
            0x14,
            "[len: u32 LE][op bytes] — replayed straight into the arena",
        ),
    ],
};

/// Property value kinds, shared by HAST element properties (stored in
/// `type_data`) and SET_PROPERTY commands.
pub const PROP_KINDS: WireTable = WireTable {
    doc: &["Property value kinds (HAST element properties and SET_PROPERTY commands)."],
    cfg: None,
    hex: false,
    consts: &[
        wc("PROP_STRING", 0, "UTF-8 value"),
        wc("PROP_BOOL_TRUE", 1, "no value bytes"),
        wc("PROP_BOOL_FALSE", 2, "no value bytes"),
        wc("PROP_SPACE_SEP", 3, "space-separated list (UTF-8)"),
        wc("PROP_COMMA_SEP", 4, "comma-separated list (UTF-8)"),
        wc("PROP_INT", 5, "decimal string, parsed to i64"),
        wc("PROP_NULL", 6, "no value bytes"),
    ],
};

/// MDX JSX attribute kinds (MDAST and HAST MDX JSX element `type_data`).
pub const MDX_ATTR_KINDS: WireTable = WireTable {
    doc: &["MDX JSX attribute kinds (MDAST and HAST MDX JSX element type_data)."],
    cfg: Some("feature = \"mdx\""),
    hex: false,
    consts: &[
        wc("MDX_ATTR_BOOLEAN_PROP", 0, "name only, no value"),
        wc("MDX_ATTR_LITERAL_PROP", 1, "name=\"literal\""),
        wc("MDX_ATTR_EXPRESSION_PROP", 2, "name={expr}"),
        wc("MDX_ATTR_SPREAD", 3, "{...expr}"),
    ],
};

/// Tables emitted into `satteri-plugin-api/src/generated/wire_constants.rs`.
pub const PLUGIN_WIRE_TABLES: &[&WireTable] = &[&OP_CODES, &OP_FIELDS, &COMMANDS, &PAYLOADS];
/// Tables emitted into `satteri-ast/src/generated/wire_constants.rs`
/// (re-exported by `shared.rs`).
pub const AST_WIRE_TABLES: &[&WireTable] = &[&PROP_KINDS, &MDX_ATTR_KINDS];
/// Tables emitted into `packages/satteri/src/generated/wire-constants.ts`.
pub const TS_WIRE_TABLES: &[&WireTable] = &[
    &OP_CODES,
    &OP_FIELDS,
    &COMMANDS,
    &PAYLOADS,
    &PROP_KINDS,
    &MDX_ATTR_KINDS,
];

/// `ArenaNode` `#[repr(C)]` size; pinned to the real struct by the generated
/// `offset_of!` asserts in satteri-arena.
pub const ARENA_NODE_SIZE: usize = 52;

/// `ArenaNode` field byte offsets (u32 fields except the `node_type` u8),
/// shared by the JS readers' `FIELD` table and the Rust asserts.
pub const ARENA_NODE_FIELDS: &[(&str, usize)] = &[
    ("id", 0),
    ("node_type", 4),
    ("parent", 8),
    ("start_offset", 12),
    ("end_offset", 16),
    ("start_line", 20),
    ("start_column", 24),
    ("end_line", 28),
    ("end_column", 32),
    ("children_start", 36),
    ("children_count", 40),
    ("data_offset", 44),
    ("data_len", 48),
];

/// Raw-buffer header fields in write order; each occupies 4 bytes (u32 LE,
/// `magic` being the 4 magic bytes). `Arena::to_raw_buffer` writes at these
/// offsets and the JS readers' `HEADER` table reads them back.
pub const ARENA_HEADER_FIELDS: &[&str] = &[
    "magic",
    "kind",
    "node_struct_size",
    "node_count",
    "nodes_offset",
    "children_count",
    "children_offset",
    "type_data_len",
    "type_data_offset",
    "string_pool_len",
    "string_pool_offset",
    "node_data_count",
    "node_data_offset",
];

/// `b"MDAR"` read as a little-endian u32 (how the JS readers check it).
pub const ARENA_MAGIC: u32 = u32::from_le_bytes(*b"MDAR");
/// `Arena<K>` kind tags carried in the header's `kind` field.
pub const ARENA_KINDS: &[(&str, u8)] = &[("Mdast", 1), ("Hast", 2)];
