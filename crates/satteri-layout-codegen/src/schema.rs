//! The node registry: the single source of truth for every MDAST and HAST node
//! type. Each [`Node`] declares its tree, tag, Rust enum variant, AST name, and
//! (for fixed-field leaf types) its `type_data` layout.
//!
//! Everything downstream is generated from this one table:
//!   * the `MdastNodeType` / `HastNodeType` enums (`generated/node_types.rs`),
//!   * the TS name maps and visitor keys (`generated/node-types.ts`),
//!   * the walk serializers + layout decoders (`walk_type_data.rs`, `layout.ts`),
//!   * compile-time layout assertions (`assert_layouts.rs`).
//!
//! Add a node here once; every list that used to repeat it is regenerated.

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

const REF_KINDS: &[&str] = &["shortcut", "collapsed", "full"];

const VALUE: &[Field] = &[s32("value", 0)];
const EXPR_VALUE: &[Field] = &[s32p("value", 0)];
const MATH: &[Field] = &[s16n("meta", 0), s32("value", 8)];
const NONE: &[Field] = &[];

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
    }
}
/// A node whose variable-length `type_data` codec stays hand-written.
const fn x(tree: Tree, tag: u8, variant: &'static str, name: &'static str) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields: NONE,
        custom: true,
    }
}

pub const MDAST_NODES: &[Node] = &[
    c(Mdast, 0, "Root", "root"),
    c(Mdast, 1, "Paragraph", "paragraph"),
    n(Mdast, 2, "Heading", "heading", &[num("depth", 0, 1)]),
    c(Mdast, 3, "ThematicBreak", "thematicBreak"),
    c(Mdast, 4, "Blockquote", "blockquote"),
    x(Mdast, 5, "List", "list"),
    x(Mdast, 6, "ListItem", "listItem"),
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
    n(
        Mdast,
        20,
        "FootnoteReference",
        "footnoteReference",
        &[s16("identifier", 0), s16("label", 8), skip8(16)],
    ),
    x(Mdast, 21, "Table", "table"),
    c(Mdast, 22, "TableRow", "tableRow"),
    c(Mdast, 23, "TableCell", "tableCell"),
    c(Mdast, 24, "Delete", "delete"),
    n(Mdast, 25, "Yaml", "yaml", VALUE),
    n(Mdast, 26, "Toml", "toml", VALUE),
    n(Mdast, 27, "Math", "math", MATH),
    // InlineMath shares Math's stored `MathData` (meta@0, value@8) but the mdast
    // spec gives it no `meta`, so only `value` is surfaced.
    n(Mdast, 28, "InlineMath", "inlineMath", &[s32("value", 8)]),
    x(Mdast, 30, "ContainerDirective", "containerDirective"),
    x(Mdast, 31, "LeafDirective", "leafDirective"),
    x(Mdast, 32, "TextDirective", "textDirective"),
    x(Mdast, 100, "MdxJsxFlowElement", "mdxJsxFlowElement"),
    x(Mdast, 101, "MdxJsxTextElement", "mdxJsxTextElement"),
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
    x(Hast, 1, "Element", "element"),
    // text/comment/raw store a single value StringRef (`encode_text_data`);
    // HAST layouts are not generated yet, so only tag/name are read back.
    n(Hast, 2, "Text", "text", VALUE),
    n(Hast, 3, "Comment", "comment", VALUE),
    c(Hast, 4, "Doctype", "doctype"),
    n(Hast, 5, "Raw", "raw", VALUE),
    x(Hast, 10, "MdxJsxElement", "mdxJsxFlowElement"),
    x(Hast, 11, "MdxJsxTextElement", "mdxJsxTextElement"),
    x(Hast, 12, "MdxFlowExpression", "mdxFlowExpression"),
    x(Hast, 13, "MdxEsm", "mdxjsEsm"),
    x(Hast, 14, "MdxTextExpression", "mdxTextExpression"),
];

/// AST names whose op-stream replay falls back to JSON. `finalize_collector`
/// (js_commands.rs) silently encodes no type_data for tags it has no arm for,
/// so a NEW node type must either gain a finalize/generated encode arm or be
/// listed here.
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

pub const MDAST_STRUCTS: &[ArenaStruct] = &[
    ArenaStruct {
        rust: "HeadingData",
        size: 1,
        offsets: &[("depth", 0)],
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
    // Every pinned struct must be reachable from a node, or it drifted loose.
    for s in MDAST_STRUCTS {
        assert!(
            STRUCT_BY_NODE.iter().any(|&(_, st)| st == s.rust),
            "MDAST_STRUCTS entry {} is mapped to no node",
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
        wc("PAYLOAD_SERDE_JSON", 0x12, "[len: u32 LE][JSON node tree]"),
        wc(
            "PAYLOAD_OPSTREAM",
            0x14,
            "[len: u32 LE][op bytes] — replayed straight into the arena, no JsNode hop",
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
    "source_len",
    "source_offset",
    "node_data_count",
    "node_data_offset",
];

/// `b"MDAR"` read as a little-endian u32 (how the JS readers check it).
pub const ARENA_MAGIC: u32 = u32::from_le_bytes(*b"MDAR");
/// `Arena<K>` kind tags carried in the header's `kind` field.
pub const ARENA_KINDS: &[(&str, u8)] = &[("Mdast", 1), ("Hast", 2)];
